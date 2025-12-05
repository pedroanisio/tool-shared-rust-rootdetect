# Project Root Detector

A Rust implementation of the [Project Root Detection Specification](https://example.com/spec) for identifying project root directories from source file paths.

## Features

- **Exclusion zones**: Automatically excludes virtual environments, `node_modules`, build artifacts, and caches
- **Marker detection**: Finds project roots via `.git`, `Cargo.toml`, `package.json`, and other project markers
- **Innermost wins**: Correctly handles monorepos by returning the closest marker to the source file
- **Symlink-aware**: Resolves symlinks before checking exclusions (editable installs work correctly)
- **Dependency clustering**: Supports LCA computation for orphan file clusters
- **Thread-safe caching**: Amortizes repeated exclusion checks across batch operations
- **Cross-platform**: Automatic case-insensitive matching on Windows/macOS

## Installation

```bash
cargo add project-root-detector
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
project-root-detector = "0.1"
```

## Quick Start

```rust
use project_root_detector::{find_root, Config};
use std::path::Path;

fn main() {
    let config = Config::default();
    let source = Path::new("/home/user/my_project/src/main.rs");
    
    match find_root(source, None, &config) {
        Some(root) => println!("Project root: {}", root.display()),
        None => println!("File is excluded (in venv, node_modules, etc.)"),
    }
}
```

## CLI Usage

```bash
# Single file
project-root-detector src/main.rs

# Multiple files
project-root-detector src/main.rs src/lib.rs tests/test.rs

# Batch mode (read from stdin)
find . -name '*.rs' | project-root-detector --batch

# JSON output
project-root-detector --json src/main.rs

# Exit with error if any file is excluded
project-root-detector --check node_modules/pkg/index.js
```

## Algorithm

The algorithm follows these cases in order:

| Case | Condition | Result |
|------|-----------|--------|
| 1 | File in exclusion zone | `None` (excluded) |
| 2 | Marker directory found | Innermost marker directory |
| 3 | Dependency cluster provided | LCA of the cluster |
| 4 | Isolated orphan | Parent directory |

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

## Configuration

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

## Batch Processing with Caching

For processing many files, use the batch API which shares an exclusion cache:

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
