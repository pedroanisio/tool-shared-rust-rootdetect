//! # Project Root Detector
//!
//! Detects project root directories from source file paths by searching for
//! project markers (like `.git`, `package.json`, `Cargo.toml`) while respecting
//! exclusion zones (like `node_modules`, `.venv`, `target`).
//!
//! ## Algorithm Summary
//!
//! 1. **Excluded files** → Returns `None`
//! 2. **Marker found** → Returns innermost directory containing a project marker
//! 3. **Orphan cluster** → Returns LCA of dependency-connected files
//! 4. **Isolated orphan** → Returns parent directory
//!
//! ## Example
//!
//! ```no_run
//! use project_root_detector::{find_root, Config};
//! use std::collections::HashSet;
//! use std::path::{Path, PathBuf};
//!
//! let config = Config::default();
//! let source = Path::new("/home/user/my_project/src/main.rs");
//!
//! if let Some(root) = find_root(source, None::<&HashSet<PathBuf>>, &config) {
//!     println!("Project root: {}", root.display());
//! }
//! ```

use std::collections::HashSet;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;

/// Default exclusion directory names (virtual envs, deps, build artifacts, caches)
pub const DEFAULT_EXCLUSIONS: &[&str] = &[
    ".venv",
    "venv",
    "node_modules",
    "__pycache__",
    "site-packages",
    ".tox",
    "dist",
    "build",
    ".egg-info",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "target",
    "vendor",
    ".gradle",
];

/// Default project marker filenames
pub const DEFAULT_MARKERS: &[&str] = &[
    ".git",
    ".hg",
    "pyproject.toml",
    "setup.py",
    "package.json",
    "Cargo.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "CMakeLists.txt",
    "deno.json",
    "composer.json",
    "mix.exs",
];

/// Errors that can occur during root detection
#[derive(Error, Debug)]
pub enum RootDetectionError {
    /// Failed to resolve or canonicalize a path
    #[error("failed to resolve path: {0}")]
    ResolutionFailed(#[from] std::io::Error),

    /// The path has no parent directory (e.g., filesystem root)
    #[error("path has no parent directory")]
    NoParent,
}

/// Configuration for the root detection algorithm
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory names that mark exclusion zones
    pub exclusions: HashSet<String>,
    /// Filenames that mark project roots
    pub markers: HashSet<String>,
    /// Whether to use case-insensitive matching (recommended for Windows/macOS)
    pub case_insensitive: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exclusions: DEFAULT_EXCLUSIONS
                .iter()
                .copied()
                .map(String::from)
                .collect(),
            markers: DEFAULT_MARKERS.iter().copied().map(String::from).collect(),
            case_insensitive: cfg!(any(target_os = "windows", target_os = "macos")),
        }
    }
}

impl Config {
    /// Create a new config with custom exclusions and markers
    #[must_use]
    pub fn new(exclusions: &[&str], markers: &[&str]) -> Self {
        Self {
            exclusions: exclusions.iter().copied().map(String::from).collect(),
            markers: markers.iter().copied().map(String::from).collect(),
            case_insensitive: cfg!(any(target_os = "windows", target_os = "macos")),
        }
    }

    /// Add additional exclusion patterns
    #[must_use]
    pub fn with_exclusions(mut self, exclusions: &[&str]) -> Self {
        self.exclusions
            .extend(exclusions.iter().copied().map(String::from));
        self
    }

    /// Add additional marker patterns
    #[must_use]
    pub fn with_markers(mut self, markers: &[&str]) -> Self {
        self.markers
            .extend(markers.iter().copied().map(String::from));
        self
    }

    fn matches_exclusion(&self, name: &str) -> bool {
        if self.case_insensitive {
            let lower = name.to_lowercase();
            self.exclusions.iter().any(|e| e.to_lowercase() == lower)
        } else {
            self.exclusions.contains(name)
        }
    }

    fn marker_exists_in(&self, dir: &Path) -> bool {
        for marker in &self.markers {
            let marker_path = dir.join(marker);
            if marker_path.exists() {
                return true;
            }
            // Also check case-insensitive on Windows/macOS
            if self.case_insensitive {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    let lower_marker = marker.to_lowercase();
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.to_lowercase() == lower_marker {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

/// Thread-safe cache for exclusion checks
#[derive(Debug, Default)]
pub struct ExclusionCache {
    cache: Mutex<std::collections::HashMap<PathBuf, bool>>,
}

impl ExclusionCache {
    /// Create a new empty exclusion cache
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the cache (useful when filesystem changes)
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    fn get(&self, path: &Path) -> Option<bool> {
        self.cache.lock().ok()?.get(path).copied()
    }

    fn insert(&self, path: PathBuf, excluded: bool) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(path, excluded);
        }
    }
}

/// Check if a path passes through any exclusion boundary.
///
/// This resolves symlinks first, so editable installs (symlinks from
/// `site-packages` into source directories) work correctly.
#[must_use]
pub fn is_excluded(path: &Path, config: &Config, cache: Option<&ExclusionCache>) -> bool {
    // Try to resolve symlinks
    let Ok(resolved) = path.canonicalize() else {
        return true; // Treat unresolvable paths as excluded
    };

    // Check cache
    if let Some(c) = cache {
        if let Some(excluded) = c.get(&resolved) {
            return excluded;
        }
    }

    // Check if any path component is an exclusion boundary
    let excluded = resolved
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .any(|name| config.matches_exclusion(name));

    // Update cache
    if let Some(c) = cache {
        c.insert(resolved, excluded);
    }

    excluded
}

/// Find the innermost marker directory for a source file
fn find_marker_root(source: &Path, config: &Config) -> Option<PathBuf> {
    let mut current = source.parent()?;

    loop {
        // Check if this directory is an exclusion boundary (stop searching)
        if let Some(name) = current.file_name().and_then(|n| n.to_str()) {
            if config.matches_exclusion(name) {
                break;
            }
        }

        // Check for any project marker in this directory
        if config.marker_exists_in(current) {
            return Some(current.to_path_buf());
        }

        // Move to parent
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }

    None
}

/// Compute the Lowest Common Ancestor of a set of paths
fn compute_lca<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Option<PathBuf> {
    let mut common_ancestors: Option<HashSet<PathBuf>> = None;

    for path in paths {
        let Ok(resolved) = path.canonicalize() else {
            continue;
        };

        // Collect all ancestors of this path
        let mut ancestors = HashSet::new();
        let mut current: &Path = &resolved;
        loop {
            ancestors.insert(current.to_path_buf());
            match current.parent() {
                Some(parent) if parent != current => current = parent,
                _ => break,
            }
        }

        // Intersect with existing common ancestors
        common_ancestors = Some(match common_ancestors {
            None => ancestors,
            Some(existing) => existing.intersection(&ancestors).cloned().collect(),
        });
    }

    // Return the deepest common ancestor
    common_ancestors?
        .into_iter()
        .max_by_key(|p| p.components().count())
}

/// Find the project root for a source file.
///
/// # Arguments
///
/// * `source_file` - Path to the source file
/// * `dependency_cluster` - Optional set of files in the same import cluster
///   (requires external static analysis to compute)
/// * `config` - Configuration for exclusions and markers
///
/// # Returns
///
/// * `Some(path)` - The project root directory
/// * `None` - If the file is excluded (in a virtual env, `node_modules`, etc.)
///
/// # Algorithm
///
/// 1. If the file is in an exclusion zone → `None`
/// 2. If a marker directory is found → innermost marker directory
/// 3. If dependency cluster provided → LCA of the cluster
/// 4. Otherwise → parent directory (isolated orphan)
#[must_use]
pub fn find_root<S: BuildHasher>(
    source_file: &Path,
    dependency_cluster: Option<&HashSet<PathBuf, S>>,
    config: &Config,
) -> Option<PathBuf> {
    find_root_with_cache(source_file, dependency_cluster, config, None)
}

/// Find the project root with an optional exclusion cache for better performance.
#[must_use]
pub fn find_root_with_cache<S: BuildHasher>(
    source_file: &Path,
    dependency_cluster: Option<&HashSet<PathBuf, S>>,
    config: &Config,
    cache: Option<&ExclusionCache>,
) -> Option<PathBuf> {
    // Case 1: Check if file is excluded
    if is_excluded(source_file, config, cache) {
        return None;
    }

    // Case 2: Search for marker directories (innermost first)
    if let Some(root) = find_marker_root(source_file, config) {
        return Some(root);
    }

    // Case 3: Orphan with dependency cluster
    if let Some(cluster) = dependency_cluster {
        let valid_files: Vec<&Path> = cluster
            .iter()
            .filter(|f| !is_excluded(f, config, cache))
            .map(PathBuf::as_path)
            .collect();

        if valid_files.len() > 1 {
            if let Some(lca) = compute_lca(valid_files) {
                return Some(lca);
            }
        }
    }

    // Case 4/5: Isolated orphan - fall back to parent directory
    let parent = source_file.parent()?;
    if parent == source_file {
        // Edge case: file at filesystem root
        Some(source_file.to_path_buf())
    } else {
        Some(parent.to_path_buf())
    }
}

/// Batch process multiple source files efficiently using a shared cache.
#[must_use]
pub fn find_roots_batch<'a>(
    source_files: impl IntoIterator<Item = &'a Path>,
    config: &Config,
) -> Vec<(&'a Path, Option<PathBuf>)> {
    let cache = ExclusionCache::new();

    source_files
        .into_iter()
        .map(|path| {
            (
                path,
                find_root_with_cache::<std::collections::hash_map::RandomState>(
                    path,
                    None,
                    config,
                    Some(&cache),
                ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn setup_project(structure: &[(&str, bool)]) -> TempDir {
        let temp = TempDir::new().unwrap();

        for (path, is_dir) in structure {
            let full_path = temp.path().join(path);

            if *is_dir {
                fs::create_dir_all(&full_path).unwrap();
            } else {
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                File::create(&full_path).unwrap();
            }
        }

        temp
    }

    #[test]
    fn test_standard_project_with_git() {
        let temp = setup_project(&[
            (".git", true),
            ("src/main.rs", false),
            ("Cargo.toml", false),
        ]);

        let config = Config::default();
        let source = temp.path().join("src/main.rs");

        let root = find_root(&source, None::<&HashSet<PathBuf>>, &config);
        assert_eq!(root, Some(temp.path().to_path_buf()));
    }

    #[test]
    fn test_monorepo_innermost_wins() {
        let temp = setup_project(&[
            (".git", true),
            ("package.json", false),
            ("packages/api/package.json", false),
            ("packages/api/src/index.ts", false),
        ]);

        let config = Config::default();
        let source = temp.path().join("packages/api/src/index.ts");

        let root = find_root(&source, None::<&HashSet<PathBuf>>, &config);
        assert_eq!(root, Some(temp.path().join("packages/api")));
    }

    #[test]
    fn test_excluded_venv() {
        let temp = setup_project(&[
            (".git", true),
            (".venv/lib/python3.11/site-packages/flask/app.py", false),
        ]);

        let config = Config::default();
        let source = temp
            .path()
            .join(".venv/lib/python3.11/site-packages/flask/app.py");

        let root = find_root(&source, None::<&HashSet<PathBuf>>, &config);
        assert_eq!(root, None);
    }

    #[test]
    fn test_excluded_node_modules() {
        let temp = setup_project(&[
            (".git", true),
            ("node_modules/lodash/index.js", false),
            ("src/app.js", false),
        ]);

        let config = Config::default();

        // File in node_modules should be excluded
        let excluded_source = temp.path().join("node_modules/lodash/index.js");
        assert_eq!(
            find_root(&excluded_source, None::<&HashSet<PathBuf>>, &config),
            None
        );

        // File in src should find the project root
        let valid_source = temp.path().join("src/app.js");
        assert_eq!(
            find_root(&valid_source, None::<&HashSet<PathBuf>>, &config),
            Some(temp.path().to_path_buf())
        );
    }

    #[test]
    fn test_isolated_orphan_fallback() {
        let temp = setup_project(&[("scripts/test.py", false)]);

        let config = Config::default();
        let source = temp.path().join("scripts/test.py");

        let root = find_root(&source, None::<&HashSet<PathBuf>>, &config);
        assert_eq!(root, Some(temp.path().join("scripts")));
    }

    #[test]
    fn test_dependency_cluster_lca() {
        let temp = setup_project(&[
            ("scripts/a.py", false),
            ("scripts/b.py", false),
            ("scripts/utils/c.py", false),
        ]);

        let config = Config::default();
        let source = temp.path().join("scripts/a.py");

        let cluster: HashSet<PathBuf> = [
            temp.path().join("scripts/a.py"),
            temp.path().join("scripts/b.py"),
            temp.path().join("scripts/utils/c.py"),
        ]
        .into_iter()
        .collect();

        let root = find_root(&source, Some(&cluster), &config);
        assert_eq!(root, Some(temp.path().join("scripts")));
    }

    #[test]
    fn test_marker_inside_exclusion_ignored() {
        let temp = setup_project(&[
            (".git", true),
            ("src/main.rs", false),
            ("node_modules/some-pkg/package.json", false),
        ]);

        let config = Config::default();

        // The package.json inside node_modules should not be found
        let source = temp.path().join("node_modules/some-pkg/index.js");
        assert_eq!(find_root(&source, None::<&HashSet<PathBuf>>, &config), None);
    }

    #[test]
    fn test_custom_config() {
        let temp = setup_project(&[
            ("WORKSPACE", false), // Bazel marker
            ("src/BUILD", false),
            ("src/main.cc", false),
        ]);

        let config = Config::default().with_markers(&["WORKSPACE", "BUILD"]);
        let source = temp.path().join("src/main.cc");

        // Should find src/ because BUILD is there (innermost)
        let root = find_root(&source, None::<&HashSet<PathBuf>>, &config);
        assert_eq!(root, Some(temp.path().join("src")));
    }

    #[test]
    fn test_batch_processing() {
        let temp = setup_project(&[
            (".git", true),
            ("src/a.rs", false),
            ("src/b.rs", false),
            ("node_modules/pkg/c.js", false),
        ]);

        let config = Config::default();
        let files: Vec<PathBuf> = vec![
            temp.path().join("src/a.rs"),
            temp.path().join("src/b.rs"),
            temp.path().join("node_modules/pkg/c.js"),
        ];

        let results = find_roots_batch(files.iter().map(PathBuf::as_path), &config);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].1, Some(temp.path().to_path_buf()));
        assert_eq!(results[1].1, Some(temp.path().to_path_buf()));
        assert_eq!(results[2].1, None); // Excluded
    }

    #[test]
    fn test_exclusion_cache() {
        let temp = setup_project(&[
            (".git", true),
            (".venv/lib/pkg/a.py", false),
            (".venv/lib/pkg/b.py", false),
        ]);

        let config = Config::default();
        let cache = ExclusionCache::new();

        let path_a = temp.path().join(".venv/lib/pkg/a.py");
        let path_b = temp.path().join(".venv/lib/pkg/b.py");

        // First call populates cache
        assert!(is_excluded(&path_a, &config, Some(&cache)));

        // Second call should use cache (same resolved prefix)
        assert!(is_excluded(&path_b, &config, Some(&cache)));

        // Verify cache is working
        cache.clear();
    }
}
