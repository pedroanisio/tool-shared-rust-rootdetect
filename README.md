---
title: Project Root Detector
date: 2025-12-05
version: 0.1.0
---

# Project Root Detector

[![CI](https://github.com/pedroanisio/tool-shared-rust-rootdetect/actions/workflows/ci.yml/badge.svg)](https://github.com/pedroanisio/tool-shared-rust-rootdetect/actions/workflows/ci.yml)

Detect project root directories from source file paths, supporting monorepos, virtual environments, and a wide range of project types.

## Features

- **Exclusion zones**: Skips virtual environments, `node_modules`, build artifacts, and caches
- **Marker detection**: Finds project roots via `.git`, `Cargo.toml`, `package.json`, and more
- **Innermost wins**: Returns the closest marker to the source file (monorepo support)
- **Symlink-aware**: Resolves symlinks before checking exclusions (editable installs work)
- **Directory traversal**: Walk filesystem trees with configurable depth and extension filters
- **Thread-safe caching**: Efficient batch processing with shared exclusion cache
- **Cross-platform**: Case-insensitive matching on Windows/macOS
- **CLI and library**: Use as a command-line tool or as a Rust crate

## Installation

```bash
cargo install project-root-detector
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
project-root-detector = "0.1"
```

## CLI Usage

### Traverse a Directory

```bash
# Find all project roots under a directory
project-root-detector traverse /path/to/code

# Only Rust files
project-root-detector traverse /path/to/code -e rs

# Multiple extensions
project-root-detector traverse /path/to/code -e rs,py,js

# Limit depth
project-root-detector traverse /path/to/code -d 3

# Show only unique roots (not per-file)
project-root-detector traverse /path/to/code --roots-only

# JSON output
project-root-detector traverse /path/to/code --json
```

### Analyze Specific Files

```bash
# Explicit files
project-root-detector files src/main.rs src/lib.rs

# Batch mode (read from stdin)
find . -name '*.rs' | project-root-detector files --batch

# JSON output
project-root-detector files --json src/main.rs

# Exit with error if any file is excluded
project-root-detector files --check node_modules/pkg/index.js
```

### Global Options

- `--json` — Output results as JSON
- `--check` — Exit with code 1 if any file is excluded

## Library Usage

```rust
use project_root_detector::{find_root, Config};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

let config = Config::default();
let source = Path::new("/home/user/my_project/src/main.rs");

// For single file lookup (without orphanage support)
type StdHashSet = HashSet<PathBuf>;
if let Some(root) = find_root(source, None::<&StdHashSet>, None::<&StdHashSet>, &config) {
    println!("Project root: {}", root.display());
}

// For batch processing with proper orphanage support, use find_roots_batch
```

### Batch Processing

```rust
use project_root_detector::{find_roots_batch, Config};
use std::path::Path;

let config = Config::default();
let files = vec![
    Path::new("src/main.rs"),
    Path::new("src/lib.rs"),
    Path::new("tests/integration.rs"),
];

let results = find_roots_batch(files.into_iter(), &config);
for (file, root) in results {
    println!("{} -> {:?}", file.display(), root);
}
```

### Directory Traversal

```rust
use project_root_detector::{traverse_and_detect, discover_roots, Config, TraversalOptions};
use std::path::Path;

let config = Config::default();
let options = TraversalOptions::default();

// Get per-file results
let results = traverse_and_detect(Path::new("/path/to/code"), &config, &options);

// Or just unique roots
let roots = discover_roots(Path::new("/path/to/code"), &config, &options);
```

### Custom Configuration

```rust
use project_root_detector::Config;

// Custom configuration
let config = Config::new(
    &[".venv", "node_modules"],  // exclusions
    &[".git", "Cargo.toml"],     // markers
);

// Or extend defaults
let config = Config::default()
    .with_exclusions(&[".myenv", "deps"])
    .with_markers(&["WORKSPACE", "BUILD.bazel"]);
```

## Algorithm

The algorithm follows these cases in order:

| Case | Condition | Result |
|------|-----------|--------|
| 1 | File in exclusion zone | `None` (excluded) |
| 2 | Marker directory found | Innermost marker directory |
| 3 | Dependency cluster provided | LCA of the cluster |
| 4 | Orphan (no markers) | Outermost SourceDir in ancestry |

### The Orphanage Rule

For files without markers in their ancestry, the algorithm finds the **outermost** ancestor directory that contains source files. This groups related orphan files under a common root.

```
api-web2text/
├── main.py              ← SourceDir: api-web2text/
└── app/api/model/
    └── user.py          → api-web2text/ (outermost SourceDir)
```

This requires batch processing via `find_roots_batch` or `traverse_and_detect` to compute SourceDirs upfront.

### Default Exclusions

```
.venv, venv, node_modules, __pycache__, site-packages,
.tox, dist, build, .egg-info, .mypy_cache, .pytest_cache,
.ruff_cache, target, vendor, .gradle
```

### Default Markers

```
.git, .hg, pyproject.toml, setup.py, package.json,
Cargo.toml, go.mod, pom.xml, build.gradle, CMakeLists.txt,
deno.json, composer.json, mix.exs
```

## Examples

### Standard Project

```
my_project/
├── .git/
├── Cargo.toml
└── src/
    └── main.rs  →  my_project/
```

### Monorepo (Innermost Wins)

```
mono/
├── .git/
├── package.json
└── packages/
    └── api/
        ├── package.json  ← innermost marker
        └── src/
            └── index.ts  →  mono/packages/api/
```

### Excluded Virtual Environment

```
project/
├── .git/
└── .venv/
    └── lib/
        └── flask/
            └── app.py  →  None (excluded)
```

### Symlink from site-packages (Editable Install)

```
project/
├── .git/
├── src/
│   └── mylib/
│       └── core.py
└── .venv/
    └── site-packages/
        └── mylib → ../../../src/mylib

# .venv/site-packages/mylib/core.py resolves to project/src/mylib/core.py
# which is NOT excluded → returns project/
```

## License

MIT
