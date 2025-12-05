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
//! // For single file lookup (without orphanage support)
//! type StdHashSet = HashSet<PathBuf>;
//! if let Some(root) = find_root(source, None::<&StdHashSet>, None::<&StdHashSet>, &config) {
//!     println!("Project root: {}", root.display());
//! }
//!
//! // For batch processing with proper orphanage support, use find_roots_batch
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

/// Find the "orphanage" for an orphan file (no marker found).
///
/// Per spec: `orphanage(s) = min⊴(SourceDirs ∩ ancestors(s))`
///
/// This finds the **outermost** ancestor directory that contains source files.
/// If no such directory exists in ancestry, falls back to the file's parent.
///
/// # Arguments
///
/// * `source` - The orphan source file
/// * `source_dirs` - Pre-computed set of directories containing valid source files
///
/// # Returns
///
/// The outermost `SourceDir` in the file's ancestry, or `parent(source)` as fallback.
fn find_orphanage<S: BuildHasher>(source: &Path, source_dirs: &HashSet<PathBuf, S>) -> PathBuf {
    let parent = source.parent().unwrap_or(source);

    // Collect ancestors that are also SourceDirs
    let mut ancestor_source_dirs: Vec<&Path> = Vec::new();
    let mut current = parent;

    loop {
        // Check if this ancestor is a SourceDir
        if source_dirs.contains(current) {
            ancestor_source_dirs.push(current);
        }

        // Move to parent
        match current.parent() {
            Some(p) if p != current => current = p,
            _ => break,
        }
    }

    if ancestor_source_dirs.is_empty() {
        // No SourceDirs in ancestry - fall back to parent (Case 5 scenario)
        parent.to_path_buf()
    } else {
        // Return the outermost (last in our traversal, closest to root)
        ancestor_source_dirs.last().unwrap().to_path_buf()
    }
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
/// * `source_dirs` - Optional set of directories containing valid source files.
///   Required for correct orphanage behavior when no markers are found.
///   If None, falls back to `parent(source_file)` for orphans.
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
/// 4. Otherwise → orphanage (outermost `SourceDir` in ancestry)
#[must_use]
pub fn find_root<S1: BuildHasher, S2: BuildHasher>(
    source_file: &Path,
    source_dirs: Option<&HashSet<PathBuf, S1>>,
    dependency_cluster: Option<&HashSet<PathBuf, S2>>,
    config: &Config,
) -> Option<PathBuf> {
    find_root_with_cache(source_file, source_dirs, dependency_cluster, config, None)
}

/// Find the project root with an optional exclusion cache for better performance.
#[must_use]
pub fn find_root_with_cache<S1: BuildHasher, S2: BuildHasher>(
    source_file: &Path,
    source_dirs: Option<&HashSet<PathBuf, S1>>,
    dependency_cluster: Option<&HashSet<PathBuf, S2>>,
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

    // Case 4: Orphan - find the orphanage (outermost SourceDir in ancestry)
    source_dirs.map_or_else(
        || Some(source_file.parent().unwrap_or(source_file).to_path_buf()),
        |dirs| Some(find_orphanage(source_file, dirs)),
    )
}

/// Batch process multiple source files efficiently using a shared cache.
///
/// This is the recommended API for processing multiple files, as it computes
/// `SourceDirs` upfront to enable correct orphanage detection.
#[must_use]
pub fn find_roots_batch<'a>(
    source_files: impl IntoIterator<Item = &'a Path>,
    config: &Config,
) -> Vec<(&'a Path, Option<PathBuf>)> {
    let cache = ExclusionCache::new();
    let files: Vec<&'a Path> = source_files.into_iter().collect();

    // Compute SourceDirs: directories containing valid (non-excluded) source files
    let source_dirs: HashSet<PathBuf> = files
        .iter()
        .filter(|f| !is_excluded(f, config, Some(&cache)))
        .filter_map(|f| f.parent().map(Path::to_path_buf))
        .collect();

    files
        .into_iter()
        .map(|path| {
            (
                path,
                find_root_with_cache::<
                    std::collections::hash_map::RandomState,
                    std::collections::hash_map::RandomState,
                >(path, Some(&source_dirs), None, config, Some(&cache)),
            )
        })
        .collect()
}

/// Result of traversing a directory and detecting roots for discovered files
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraversalResult {
    /// The source file that was discovered
    pub file: PathBuf,
    /// The detected project root (None if excluded)
    pub root: Option<PathBuf>,
}

/// Options for filesystem traversal
#[derive(Debug, Clone, Default)]
pub struct TraversalOptions {
    /// File extensions to consider as source files (e.g., `rs`, `py`, `js`)
    /// If empty, all files are considered
    pub extensions: HashSet<String>,
    /// Maximum directory depth to traverse (None for unlimited)
    pub max_depth: Option<usize>,
}

impl TraversalOptions {
    /// Create options with specific file extensions
    #[must_use]
    pub fn with_extensions(mut self, extensions: &[&str]) -> Self {
        self.extensions = extensions.iter().copied().map(String::from).collect();
        self
    }

    /// Set maximum traversal depth
    #[must_use]
    pub const fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    fn matches_extension(&self, path: &Path) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| self.extensions.contains(e))
    }
}

/// Traverse a directory tree, discover source files, and detect their project roots.
///
/// This function walks the filesystem starting from `start_path`, skipping exclusion
/// zones (like `node_modules`, `.venv`, etc.), and returns the project root for each
/// discovered source file.
///
/// The function uses a two-phase approach:
/// 1. First, collect all source files
/// 2. Then, compute `SourceDirs` (directories containing source files)
/// 3. Finally, detect roots with correct orphanage behavior
///
/// # Arguments
///
/// * `start_path` - Directory to start traversal from
/// * `config` - Configuration for exclusions and markers
/// * `options` - Traversal options (extensions filter, max depth)
///
/// # Returns
///
/// Vector of `TraversalResult` containing each discovered file and its root
#[must_use]
pub fn traverse_and_detect(
    start_path: &Path,
    config: &Config,
    options: &TraversalOptions,
) -> Vec<TraversalResult> {
    let cache = ExclusionCache::new();

    // Phase 1: Collect all source files
    let mut files = Vec::new();
    collect_files_recursive(start_path, config, options, 0, &mut files);

    // Phase 2: Compute SourceDirs (directories containing valid source files)
    let source_dirs: HashSet<PathBuf> = files
        .iter()
        .filter(|f| !is_excluded(f, config, Some(&cache)))
        .filter_map(|f| f.parent().map(Path::to_path_buf))
        .collect();

    // Phase 3: Detect roots with proper orphanage support
    files
        .into_iter()
        .map(|file| {
            let root = find_root_with_cache::<
                std::collections::hash_map::RandomState,
                std::collections::hash_map::RandomState,
            >(&file, Some(&source_dirs), None, config, Some(&cache));
            TraversalResult { file, root }
        })
        .collect()
}

fn collect_files_recursive(
    dir: &Path,
    config: &Config,
    options: &TraversalOptions,
    depth: usize,
    files: &mut Vec<PathBuf>,
) {
    // Check max depth
    if let Some(max) = options.max_depth {
        if depth > max {
            return;
        }
    }

    // Check if this directory is an exclusion boundary
    if let Some(name) = dir.file_name().and_then(|n| n.to_str()) {
        if config.matches_exclusion(name) {
            return; // Don't descend into exclusion zones
        }
    }

    // Read directory entries
    let Ok(entries) = std::fs::read_dir(dir) else {
        return; // Skip unreadable directories
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // Recurse into subdirectory
            collect_files_recursive(&path, config, options, depth + 1, files);
        } else if path.is_file() && options.matches_extension(&path) {
            // Found a source file
            files.push(path);
        }
    }
}

/// Traverse and return only the unique project roots discovered
#[must_use]
pub fn discover_roots(
    start_path: &Path,
    config: &Config,
    options: &TraversalOptions,
) -> HashSet<PathBuf> {
    traverse_and_detect(start_path, config, options)
        .into_iter()
        .filter_map(|r| r.root)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    // Type alias for find_root with default hasher
    type StdHashSet = HashSet<PathBuf>;

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

        let root = find_root(&source, None::<&StdHashSet>, None::<&StdHashSet>, &config);
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

        let root = find_root(&source, None::<&StdHashSet>, None::<&StdHashSet>, &config);
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

        let root = find_root(&source, None::<&StdHashSet>, None::<&StdHashSet>, &config);
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
            find_root(
                &excluded_source,
                None::<&StdHashSet>,
                None::<&StdHashSet>,
                &config
            ),
            None
        );

        // File in src should find the project root
        let valid_source = temp.path().join("src/app.js");
        assert_eq!(
            find_root(
                &valid_source,
                None::<&StdHashSet>,
                None::<&StdHashSet>,
                &config
            ),
            Some(temp.path().to_path_buf())
        );
    }

    #[test]
    fn test_isolated_orphan_fallback() {
        // Single orphan file at temp/scripts/test.py
        // Without source_dirs, falls back to parent directory
        let temp = setup_project(&[("scripts/test.py", false)]);

        let config = Config::default();
        let source = temp.path().join("scripts/test.py");

        // Without source_dirs, falls back to parent
        let root = find_root(&source, None::<&StdHashSet>, None::<&StdHashSet>, &config);
        assert_eq!(root, Some(temp.path().join("scripts")));
    }

    #[test]
    fn test_orphanage_bounded_by_marker() {
        // When a marker exists above, orphanage stops below it
        let temp = setup_project(&[
            (".git", true), // Marker at root
            ("orphan-dir/deep/file.py", false),
        ]);

        let config = Config::default();
        let source = temp.path().join("orphan-dir/deep/file.py");

        let root = find_root(&source, None::<&StdHashSet>, None::<&StdHashSet>, &config);
        // File finds .git marker at temp root, returns temp root (Case 2)
        assert_eq!(root, Some(temp.path().to_path_buf()));
    }

    #[test]
    fn test_file_below_project_returns_project_root() {
        // Files below a project with markers return the project root (Case 2)
        // NOT the orphanage - this is correct behavior
        let temp = setup_project(&[
            (".git", true),                        // Root has marker
            ("subdir/orphan/deep/file.py", false), // No markers in subdir tree
        ]);

        let config = Config::default();
        let source = temp.path().join("subdir/orphan/deep/file.py");

        let root = find_root(&source, None::<&StdHashSet>, None::<&StdHashSet>, &config);
        // Should return temp/ because .git marker is found there (Case 2)
        assert_eq!(root, Some(temp.path().to_path_buf()));
    }

    #[test]
    fn test_nested_project_innermost_wins() {
        // Nested project markers - innermost wins (Case 2)
        let temp = setup_project(&[
            (".git", true),                         // Root project
            ("libs/orphan-pkg/src/main.py", false), // No marker - returns root
            ("libs/real-pkg/package.json", false),  // Nested project marker
            ("libs/real-pkg/src/index.js", false),
        ]);

        let config = Config::default();

        // File with no inner marker gets the outermost marker (root)
        let orphan = temp.path().join("libs/orphan-pkg/src/main.py");
        let orphan_root = find_root(&orphan, None::<&StdHashSet>, None::<&StdHashSet>, &config);
        assert_eq!(orphan_root, Some(temp.path().to_path_buf()));

        // Real project file gets real-pkg as root (innermost marker)
        let real = temp.path().join("libs/real-pkg/src/index.js");
        let real_root = find_root(&real, None::<&StdHashSet>, None::<&StdHashSet>, &config);
        assert_eq!(real_root, Some(temp.path().join("libs/real-pkg")));
    }

    #[test]
    fn test_orphanage_with_source_dirs() {
        // TRUE orphanage test using find_roots_batch which computes source_dirs
        // Per spec: orphanage(s) = min⊴(SourceDirs ∩ ancestors(s))
        let temp = setup_project(&[
            ("project/main.py", false),             // SourceDir: project/
            ("project/app/utils/helper.py", false), // SourceDir: project/app/utils/
            ("project/lib/core.py", false),         // SourceDir: project/lib/
        ]);

        let config = Config::default();

        let files = vec![
            temp.path().join("project/main.py"),
            temp.path().join("project/app/utils/helper.py"),
            temp.path().join("project/lib/core.py"),
        ];

        let results = find_roots_batch(files.iter().map(PathBuf::as_path), &config);

        // All files should share the same orphanage: project/ (outermost SourceDir)
        let roots: Vec<_> = results.iter().map(|(_, r)| r.clone()).collect();
        assert_eq!(roots[0], Some(temp.path().join("project")));
        assert_eq!(roots[1], Some(temp.path().join("project")));
        assert_eq!(roots[2], Some(temp.path().join("project")));
    }

    #[test]
    fn test_orphanage_deep_file_finds_outermost() {
        // Test the spec example: api-web2text/app/api/model/user.py
        // with main.py at api-web2text/, should return api-web2text/
        let temp = setup_project(&[
            ("api-web2text/main.py", false), // SourceDir: api-web2text/
            ("api-web2text/app/api/model/user.py", false), // deep file
        ]);

        let config = Config::default();

        let files = vec![
            temp.path().join("api-web2text/main.py"),
            temp.path().join("api-web2text/app/api/model/user.py"),
        ];

        let results = find_roots_batch(files.iter().map(PathBuf::as_path), &config);

        // Both files should get api-web2text/ as root (outermost SourceDir)
        let roots: Vec<_> = results.iter().map(|(_, r)| r.clone()).collect();
        assert_eq!(roots[0], Some(temp.path().join("api-web2text")));
        assert_eq!(roots[1], Some(temp.path().join("api-web2text")));
    }

    #[test]
    fn test_orphanage_isolated_file() {
        // Single orphan file with no sources above it
        // Falls back to parent directory per spec
        let temp = setup_project(&[("scripts/test.py", false)]);

        let config = Config::default();

        let files = vec![temp.path().join("scripts/test.py")];

        let results = find_roots_batch(files.iter().map(PathBuf::as_path), &config);

        // SourceDirs = {scripts/}, ancestors ∩ SourceDirs = {scripts/}
        // Outermost = scripts/
        assert_eq!(results[0].1, Some(temp.path().join("scripts")));
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

        let root = find_root(&source, None::<&StdHashSet>, Some(&cluster), &config);
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
        assert_eq!(
            find_root(&source, None::<&StdHashSet>, None::<&StdHashSet>, &config),
            None
        );
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
        let root = find_root(&source, None::<&StdHashSet>, None::<&StdHashSet>, &config);
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

    // ==================== TRAVERSAL TESTS ====================

    #[test]
    fn test_traverse_simple_project() {
        let temp = setup_project(&[
            (".git", true),
            ("src/main.rs", false),
            ("src/lib.rs", false),
            ("Cargo.toml", false),
        ]);

        let config = Config::default();
        let options = TraversalOptions::default().with_extensions(&["rs"]);

        let results = traverse_and_detect(temp.path(), &config, &options);

        // Should find 2 .rs files
        assert_eq!(results.len(), 2);

        // All should have the same root
        for result in &results {
            assert_eq!(result.root, Some(temp.path().to_path_buf()));
        }
    }

    #[test]
    fn test_traverse_skips_exclusion_zones() {
        let temp = setup_project(&[
            (".git", true),
            ("src/main.rs", false),
            ("node_modules/lodash/index.js", false),
            ("node_modules/lodash/package.json", false),
            (".venv/lib/site-packages/flask/app.py", false),
        ]);

        let config = Config::default();
        let options = TraversalOptions::default(); // All files

        let results = traverse_and_detect(temp.path(), &config, &options);

        // Should only find files outside exclusion zones
        let files: Vec<_> = results.iter().map(|r| &r.file).collect();

        // Should NOT contain any node_modules or .venv files
        for file in &files {
            let path_str = file.to_string_lossy();
            assert!(
                !path_str.contains("node_modules"),
                "Should not traverse node_modules: {path_str}"
            );
            assert!(
                !path_str.contains(".venv"),
                "Should not traverse .venv: {path_str}"
            );
        }

        // Should find src/main.rs
        assert!(files.iter().any(|f| f.ends_with("main.rs")));
    }

    #[test]
    fn test_traverse_monorepo() {
        let temp = setup_project(&[
            (".git", true),
            ("package.json", false),
            ("packages/api/package.json", false),
            ("packages/api/src/index.ts", false),
            ("packages/web/package.json", false),
            ("packages/web/src/app.tsx", false),
        ]);

        let config = Config::default();
        let options = TraversalOptions::default().with_extensions(&["ts", "tsx"]);

        let results = traverse_and_detect(temp.path(), &config, &options);

        assert_eq!(results.len(), 2);

        // api/src/index.ts should have root packages/api
        let api_result = results
            .iter()
            .find(|r| r.file.ends_with("index.ts"))
            .unwrap();
        assert_eq!(api_result.root, Some(temp.path().join("packages/api")));

        // web/src/app.tsx should have root packages/web
        let web_result = results
            .iter()
            .find(|r| r.file.ends_with("app.tsx"))
            .unwrap();
        assert_eq!(web_result.root, Some(temp.path().join("packages/web")));
    }

    #[test]
    fn test_traverse_with_max_depth() {
        let temp = setup_project(&[
            (".git", true),
            ("a.rs", false),
            ("src/b.rs", false),
            ("src/nested/c.rs", false),
            ("src/nested/deep/d.rs", false),
        ]);

        let config = Config::default();

        // Depth 0 = only start directory
        let options = TraversalOptions::default()
            .with_extensions(&["rs"])
            .with_max_depth(0);
        let results = traverse_and_detect(temp.path(), &config, &options);
        assert_eq!(results.len(), 1); // Only a.rs

        // Depth 1 = start + one level
        let options = TraversalOptions::default()
            .with_extensions(&["rs"])
            .with_max_depth(1);
        let results = traverse_and_detect(temp.path(), &config, &options);
        assert_eq!(results.len(), 2); // a.rs + src/b.rs

        // Depth 2
        let options = TraversalOptions::default()
            .with_extensions(&["rs"])
            .with_max_depth(2);
        let results = traverse_and_detect(temp.path(), &config, &options);
        assert_eq!(results.len(), 3); // a.rs + src/b.rs + src/nested/c.rs
    }

    #[test]
    fn test_traverse_extension_filter() {
        let temp = setup_project(&[
            (".git", true),
            ("main.rs", false),
            ("lib.py", false),
            ("app.js", false),
            ("README.md", false),
        ]);

        let config = Config::default();

        // Only .rs files
        let options = TraversalOptions::default().with_extensions(&["rs"]);
        let results = traverse_and_detect(temp.path(), &config, &options);
        assert_eq!(results.len(), 1);
        assert!(results[0].file.ends_with("main.rs"));

        // Multiple extensions
        let options = TraversalOptions::default().with_extensions(&["rs", "py"]);
        let results = traverse_and_detect(temp.path(), &config, &options);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_discover_roots_unique() {
        let temp = setup_project(&[
            (".git", true),
            ("src/a.rs", false),
            ("src/b.rs", false),
            ("src/c.rs", false),
        ]);

        let config = Config::default();
        let options = TraversalOptions::default().with_extensions(&["rs"]);

        let roots = discover_roots(temp.path(), &config, &options);

        // Should return only one unique root
        assert_eq!(roots.len(), 1);
        assert!(roots.contains(&temp.path().to_path_buf()));
    }

    #[test]
    fn test_traverse_nested_exclusions() {
        let temp = setup_project(&[
            (".git", true),
            ("src/main.rs", false),
            (".venv/lib/python/site-packages/pkg/module.py", false),
            ("build/output/generated.rs", false),
            ("target/debug/deps/crate.rs", false),
        ]);

        let config = Config::default();
        let options = TraversalOptions::default();

        let results = traverse_and_detect(temp.path(), &config, &options);

        // Should only find src/main.rs (others are in exclusion zones)
        assert_eq!(results.len(), 1);
        assert!(results[0].file.ends_with("main.rs"));
    }

    #[test]
    fn test_traverse_orphan_files() {
        let temp = setup_project(&[("scripts/util.py", false), ("scripts/helper.py", false)]);

        let config = Config::default();
        let options = TraversalOptions::default().with_extensions(&["py"]);

        let results = traverse_and_detect(temp.path(), &config, &options);

        assert_eq!(results.len(), 2);

        // Orphan files should share the same orphanage
        let first_root = results[0].root.clone();
        assert!(first_root.is_some());
        for result in &results {
            assert_eq!(result.root, first_root);
        }
    }
}
