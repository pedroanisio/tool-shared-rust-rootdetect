---
title: Changelog
date: 2025-12-05
version: 0.1.0
---

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-12-05

### Added

- Initial release of project-root-detector
- Core library API (`find_root`, `find_root_with_cache`, `find_roots_batch`)
- Filesystem traversal (`traverse_and_detect`, `discover_roots`)
- Configurable exclusion zones (`.venv`, `node_modules`, `target`, etc.)
- Configurable project markers (`.git`, `Cargo.toml`, `package.json`, etc.)
- Thread-safe exclusion cache for batch processing
- Symlink resolution for editable installs
- CLI with two subcommands:
  - `traverse` - Walk directory trees and detect roots
  - `files` - Analyze specific file paths
- CLI options:
  - `--json` for JSON output
  - `--check` for CI validation (exit 1 if files excluded)
  - `-e, --extensions` for filtering by file type
  - `-d, --max-depth` for limiting traversal depth
  - `--roots-only` for unique roots output
  - `--batch` for stdin input
- Comprehensive test suite (23 tests, 93% library coverage)
- Full ADR compliance (ADR-001 through ADR-140)

[Unreleased]: https://github.com/owner/project-root-detector/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/owner/project-root-detector/releases/tag/v0.1.0
