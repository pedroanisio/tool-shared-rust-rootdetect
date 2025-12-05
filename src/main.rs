//! CLI tool for detecting project roots
//!
//! Usage:
//!   project-root-detector /path/to/dir          # Traverse directory
//!   project-root-detector --files file1 file2   # Explicit file paths
//!   project-root-detector --batch < files.txt   # Read paths from stdin

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use project_root_detector::{
    discover_roots, find_roots_batch, traverse_and_detect, Config, TraversalOptions,
    TraversalResult,
};
use serde::Serialize;
use std::collections::HashSet;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Detect project root directories from source file paths.
#[derive(Parser, Debug)]
#[command(name = "project-root-detector")]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Output results as JSON
    #[arg(long, global = true)]
    json: bool,

    /// Exit with code 1 if any file is excluded
    #[arg(long, global = true)]
    check: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Traverse a directory tree and detect project roots for all source files
    Traverse {
        /// Directory to traverse
        #[arg(value_name = "DIR")]
        directory: PathBuf,

        /// File extensions to include (e.g., rs, py, js). If not specified, all files are included.
        #[arg(short, long, value_delimiter = ',')]
        extensions: Option<Vec<String>>,

        /// Maximum traversal depth (0 = only the start directory)
        #[arg(short = 'd', long)]
        max_depth: Option<usize>,

        /// Only show unique project roots (not individual files)
        #[arg(long)]
        roots_only: bool,
    },

    /// Detect roots for explicit file paths
    Files {
        /// Source files to analyze
        #[arg(value_name = "FILE")]
        files: Vec<PathBuf>,

        /// Read file paths from stdin (one per line)
        #[arg(long)]
        batch: bool,
    },
}

/// Result for a single file's root detection
#[derive(Serialize)]
struct FileResult {
    file: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    root: Option<PathBuf>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    excluded: bool,
}

/// Result for unique roots discovery
#[derive(Serialize)]
struct RootsResult {
    roots: Vec<PathBuf>,
    count: usize,
}

fn collect_files_from_stdin() -> Result<Vec<PathBuf>> {
    let stdin = io::stdin();
    stdin
        .lock()
        .lines()
        .map(|line| line.context("Failed to read line from stdin"))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| Ok(PathBuf::from(line)))
        .collect()
}

fn run_traverse(
    directory: &Path,
    extensions: Option<&Vec<String>>,
    max_depth: Option<usize>,
    roots_only: bool,
    json: bool,
    check: bool,
) -> Result<bool> {
    let config = Config::default();

    let mut options = TraversalOptions::default();
    if let Some(exts) = extensions {
        options.extensions = exts.iter().cloned().collect();
    }
    if let Some(depth) = max_depth {
        options.max_depth = Some(depth);
    }

    if roots_only {
        let roots: HashSet<PathBuf> = discover_roots(directory, &config, &options);
        let mut roots_vec: Vec<PathBuf> = roots.into_iter().collect();
        roots_vec.sort();

        if json {
            let result = RootsResult {
                count: roots_vec.len(),
                roots: roots_vec,
            };
            let json_str =
                serde_json::to_string_pretty(&result).context("Failed to serialize to JSON")?;
            println!("{json_str}");
        } else {
            for root in &roots_vec {
                println!("{}", root.display());
            }
        }

        Ok(false) // roots_only mode doesn't track exclusions
    } else {
        let results: Vec<TraversalResult> = traverse_and_detect(directory, &config, &options);

        let mut any_excluded = false;
        let file_results: Vec<FileResult> = results
            .into_iter()
            .map(|r| {
                let excluded = r.root.is_none();
                if excluded {
                    any_excluded = true;
                }
                FileResult {
                    file: r.file,
                    root: r.root,
                    excluded,
                }
            })
            .collect();

        output_file_results(&file_results, json)?;

        Ok(check && any_excluded)
    }
}

fn run_files(files: &[PathBuf], batch: bool, json: bool, check: bool) -> Result<bool> {
    let config = Config::default();

    let files: Vec<PathBuf> = if batch || files.is_empty() {
        collect_files_from_stdin()?
    } else {
        files.to_vec()
    };

    if files.is_empty() {
        anyhow::bail!("No files provided");
    }

    let results = find_roots_batch(files.iter().map(PathBuf::as_path), &config);

    let mut any_excluded = false;
    let file_results: Vec<FileResult> = results
        .into_iter()
        .map(|(path, root)| {
            let excluded = root.is_none();
            if excluded {
                any_excluded = true;
            }
            FileResult {
                file: path.to_path_buf(),
                root,
                excluded,
            }
        })
        .collect();

    output_file_results(&file_results, json)?;

    Ok(check && any_excluded)
}

fn output_file_results(results: &[FileResult], json: bool) -> Result<()> {
    if json {
        let json_str =
            serde_json::to_string_pretty(results).context("Failed to serialize to JSON")?;
        println!("{json_str}");
    } else {
        for result in results {
            result.root.as_ref().map_or_else(
                || println!("{} -> (excluded)", result.file.display()),
                |r| println!("{} -> {}", result.file.display(), r.display()),
            );
        }
    }
    Ok(())
}

fn run(args: &Args) -> Result<bool> {
    match &args.command {
        Some(Command::Traverse {
            directory,
            extensions,
            max_depth,
            roots_only,
        }) => run_traverse(
            directory,
            extensions.as_ref(),
            *max_depth,
            *roots_only,
            args.json,
            args.check,
        ),

        Some(Command::Files { files, batch }) => run_files(files, *batch, args.json, args.check),

        // Default: if a single path is provided and it's a directory, traverse it
        // Otherwise, treat arguments as files (backwards compatibility)
        None => {
            // No subcommand - show help
            anyhow::bail!(
                "No command specified. Use 'traverse <DIR>' or 'files <FILE>...'\n\
                 Run with --help for more information."
            )
        }
    }
}

fn main() -> ExitCode {
    let args = Args::parse();

    match run(&args) {
        Ok(should_fail) => {
            if should_fail {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("Error: {e:?}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_result_serialization() {
        let result = FileResult {
            file: PathBuf::from("/test/file.rs"),
            root: Some(PathBuf::from("/test")),
            excluded: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("file"));
        assert!(json.contains("root"));
        assert!(!json.contains("excluded")); // excluded: false should be skipped
    }

    #[test]
    fn test_file_result_excluded_serialization() {
        let result = FileResult {
            file: PathBuf::from("/test/node_modules/pkg/index.js"),
            root: None,
            excluded: true,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("excluded"));
        assert!(!json.contains(r#""root""#)); // None should be skipped
    }

    #[test]
    fn test_roots_result_serialization() {
        let result = RootsResult {
            roots: vec![PathBuf::from("/project1"), PathBuf::from("/project2")],
            count: 2,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("roots"));
        assert!(json.contains("count"));
        assert!(json.contains("/project1"));
    }

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
