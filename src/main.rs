//! CLI tool for detecting project roots
//!
//! Usage:
//!   project-root-detector <file>...
//!   project-root-detector --batch < files.txt

use anyhow::{Context, Result};
use clap::Parser;
use project_root_detector::{find_roots_batch, Config};
use serde::Serialize;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process::ExitCode;

/// Detect project root directories from source file paths.
#[derive(Parser, Debug)]
#[command(name = "project-root-detector")]
#[command(version, about, long_about = None)]
struct Args {
    /// Read file paths from stdin (one per line)
    #[arg(long)]
    batch: bool,

    /// Exit with code 1 if any file is excluded
    #[arg(long)]
    check: bool,

    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Source files to analyze
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
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

fn collect_files(args: &Args) -> Result<Vec<PathBuf>> {
    if args.batch || args.files.is_empty() {
        let stdin = io::stdin();
        let files: Vec<PathBuf> = stdin
            .lock()
            .lines()
            .map(|line| line.context("Failed to read line from stdin"))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .map(PathBuf::from)
            .collect();
        Ok(files)
    } else {
        Ok(args.files.clone())
    }
}

fn run(args: Args) -> Result<bool> {
    let config = Config::default();

    let files = collect_files(&args)?;

    if files.is_empty() {
        anyhow::bail!("No files provided");
    }

    let results = find_roots_batch(files.iter().map(|p| p.as_path()), &config);

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

    if args.json {
        let json = serde_json::to_string_pretty(&file_results)
            .context("Failed to serialize results to JSON")?;
        println!("{json}");
    } else {
        for result in file_results {
            match result.root {
                Some(r) => println!("{} -> {}", result.file.display(), r.display()),
                None => println!("{} -> (excluded)", result.file.display()),
            }
        }
    }

    Ok(any_excluded)
}

fn main() -> ExitCode {
    let args = Args::parse();
    let check = args.check;

    match run(args) {
        Ok(any_excluded) => {
            if check && any_excluded {
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
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
