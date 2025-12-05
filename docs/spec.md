---
title: "Source Code Project Root Detection"
subtitle: "Formal Specification"
version: 1.0.1
date: 2025-06-05
status: Final
authors:
  - Human (Design & Requirements)
  - Claude (Formalization & Implementation)
abstract: |
  A formal specification for detecting project root directories from source files.
  Handles markers (.git, pyproject.toml, etc.), exclusion zones (node_modules, .venv),
  and orphan grouping via the "orphanage rule." Designed for IDEs, build tools, and
  static analyzers that need reliable project boundary detection.
keywords:
  - project detection
  - source code analysis
  - filesystem traversal
  - monorepo
license: CC-BY-4.0
---

# Formal Specification: Source Code Project Root Detection

## Preliminaries

### Filesystem Model

Let $\mathcal{F} = (N, \trianglelefteq)$ be a rooted tree where:
- $N$ is the set of filesystem nodes (files and directories)
- $a \trianglelefteq b$ denotes "$a$ is an ancestor of $b$" (i.e., $a$ lies on the path from the filesystem root to $b$)
- $a \triangleleft b$ denotes $a \trianglelefteq b \land a \neq b$ (strict ancestor)

We define:
- $\text{name}: N \to \text{String}$ — the basename of a node
- $\text{parent}: N \to N$ — the immediate ancestor (undefined at root)
- $\text{resolve}: N \to N$ — canonical path resolution (follows symlinks)
- $\text{ancestors}(n) = \{a \in N : a \trianglelefteq n\}$
- $\text{descendants}(n) = \{d \in N : n \trianglelefteq d\}$

### Problem Inputs

| Symbol | Type | Description |
|--------|------|-------------|
| $S \subseteq N$ | set | Source files (leaves we care about) |
| $\mathcal{E}$ | set of strings | Exclusion directory names |
| $\mathcal{M}$ | set of strings | Marker filenames |
| $D \subseteq S \times S$ | relation | Import/dependency edges |

Default values (configurable):

$$\mathcal{E} = \{\texttt{.venv}, \texttt{venv}, \texttt{node\_modules}, \texttt{\_\_pycache\_\_}, \texttt{site-packages}, \texttt{.tox}, \texttt{dist}, \texttt{build}, \texttt{.egg-info}, \texttt{.mypy\_cache}, \texttt{.pytest\_cache}, \texttt{.ruff\_cache}, \texttt{target}, \texttt{vendor}, \texttt{.gradle}\}$$

$$\mathcal{M} = \{\texttt{.git}, \texttt{.hg}, \texttt{.svn}, \texttt{pyproject.toml}, \texttt{setup.py}, \texttt{package.json}, \texttt{Cargo.toml}, \texttt{go.mod}, \texttt{pom.xml}, \texttt{build.gradle}, \texttt{CMakeLists.txt}, \texttt{deno.json}, \texttt{composer.json}, \texttt{mix.exs}, \texttt{Gemfile}, \texttt{BUILD}, \texttt{WORKSPACE}\}$$

*Note on markers:* `Makefile` is intentionally excluded from defaults—it commonly appears in subdirectories and would cause false root detection. Add it only if your codebase uses Makefiles exclusively at project roots.

---

## 1. Exclusion Zones

### Definition (Exclusion Boundary)

A node $n$ is an **exclusion boundary** iff its name matches an exclusion pattern:

$$\text{boundary}(n) \iff \text{name}(n) \in \mathcal{E}$$

### Definition (Excluded Set)

The **excluded set** $\mathcal{X}$ contains all nodes at or below any exclusion boundary, computed on resolved paths:

$$\mathcal{X} = \{n \in N : \exists b \in N,\ \text{boundary}(b) \land b \trianglelefteq \text{resolve}(n)\}$$

Equivalently: $n$ is excluded iff any ancestor of its resolved path has an excluded name.

### Lemma 1 (Exclusion Inheritance)

Descendants of excluded nodes are excluded:
$$n \in \mathcal{X} \land n \trianglelefteq m \implies m \in \mathcal{X}$$

*Proof:* Follows directly from the definition of $\mathcal{X}$.

---

## 2. Filtered Sets

### Valid Sources

$$S' = S \setminus \mathcal{X}$$

These are "authored" source files — code written by the developer, not installed dependencies or build artifacts.

### Valid Markers

A marker is valid iff it exists outside exclusion zones:

$$M = \{n \in N : \text{name}(n) \in \mathcal{M} \land n \notin \mathcal{X}\}$$

*Note:* If a directory contains multiple markers (e.g., both `.git` and `pyproject.toml`), all are equally valid. Marker *type* does not affect priority—only *depth* matters (innermost wins).

### Marker Directories

The directories containing valid markers:

$$\text{MarkerDirs} = \{\text{parent}(m) : m \in M\}$$

### Source Directories

The directories directly containing valid source files:

$$\text{SourceDirs} = \{\text{parent}(s) : s \in S'\}$$

---

## 3. Ancestor Constraints

### Definition (Clear Path)

A source file $s$ has a **clear path** to ancestor $a$ iff no exclusion boundary lies strictly between them:

$$\text{clear}(s, a) \iff \neg\exists b \in N : (a \triangleleft b \triangleleft s) \land \text{boundary}(b)$$

### Definition (Reachable Marker Directories)

For source $s \in S'$, the marker directories reachable via clear paths:

$$\text{Roots}(s) = \{d \in \text{MarkerDirs} : d \trianglelefteq s \land \text{clear}(s, d)\}$$

### Lemma 2 (Exclusion Opacity)

Root computation never crosses exclusion boundaries:
$$\forall s \in S',\ \forall d \in \text{Roots}(s) : \text{clear}(s, d)$$

*Proof:* By construction of $\text{Roots}(s)$.

---

## 4. Orphans and Orphanages

Files without markers in their ancestry require special handling to avoid root fragmentation.

### Definition (Orphan)

A source file $s \in S'$ is an **orphan** iff no marker exists in its ancestry:

$$\text{orphan}(s) \iff \text{Roots}(s) = \emptyset$$

### Definition (Orphanage)

The **orphanage** of an orphan source file $s$ is the **outermost ancestor directory that contains source files**:

$$\text{orphanage}(s) = \min_{\trianglelefteq}(\text{SourceDirs} \cap \text{ancestors}(s))$$

This is the topmost directory in $s$'s ancestry that directly contains at least one source file.

*Notation:* $\min_{\trianglelefteq}$ means "minimum by the ancestor relation"—i.e., the **outermost** directory, closest to the filesystem root. Conversely, $\max_{\trianglelefteq}$ means the **innermost** directory, closest to the file.

### The Orphanage Rule

*"Source files mark their parent directory as a potential root. The outermost such directory becomes the orphanage."*

| Scenario | Orphanage |
|----------|-----------|
| Source file exists at ancestor level | Outermost ancestor with source |
| No source files above | $\text{parent}(s)$ (file's immediate parent) |

### Lemma 3 (Orphanage Grouping)

All orphan files sharing a common ancestor with source files get the same orphanage:

If $\text{orphan}(s_1) \land \text{orphan}(s_2) \land \exists a \in \text{SourceDirs} : a \trianglelefteq s_1 \land a \trianglelefteq s_2$, then $\text{orphanage}(s_1) = \text{orphanage}(s_2)$.

*Proof:* Both files compute the minimum over $\text{SourceDirs} \cap \text{ancestors}(s)$, and they share the relevant ancestors.

---

## 5. Dependency Closure

For additional grouping of related orphan files:

### Definition (Dependency Relation)

$D \subseteq S \times S$ where $(a, b) \in D$ means "file $a$ imports file $b$."

*Note:* Construction of $D$ requires static analysis (parsing import statements) and is outside the scope of this specification.

### Definition (Dependency Closure)

$$\text{closure}_D(s) = \{t \in S : s\ D^*\ t \lor t\ D^*\ s\}$$

where $D^*$ is the reflexive-transitive closure of $D$. This captures all files transitively connected by imports in either direction.

### Definition (Lowest Common Ancestor)

For a non-empty set $X \subseteq N$:

$$\text{LCA}(X) = \max_{\trianglelefteq}\{a \in N : \forall x \in X,\ a \trianglelefteq x\}$$

The LCA exists and is unique in a tree.

---

## 6. Root Function

### Definition (Project Root)

The root function $\rho: S \to N \cup \{\bot\}$ is defined:

$$\rho(s) = \begin{cases}
\bot & \text{if } s \in \mathcal{X} \\[8pt]
\max_{\trianglelefteq}(\text{Roots}(s)) & \text{if } \text{Roots}(s) \neq \emptyset \\[8pt]
\text{LCA}(\text{closure}_D(s) \cap S') & \text{if } |\text{closure}_D(s) \cap S'| > 1 \\[8pt]
\text{orphanage}(s) & \text{if } \text{parent}(s) \neq s \\[8pt]
s & \text{otherwise (file at filesystem root)}
\end{cases}$$

Where:
- **Case 1:** $\bot$ indicates "not a project file" (excluded)
- **Case 2:** Innermost marker directory (marker takes precedence)
- **Case 3:** Orphan cluster — use LCA of the dependency-connected component
- **Case 4:** Orphan — use orphanage (outermost SourceDir in ancestry)
- **Case 5:** Edge case for files directly at filesystem root

*Implementation note:* Case 3 requires static import analysis to construct $D$, which may be expensive or unavailable. Implementations may skip directly from Case 2 to Case 4 when dependency information is not provided.

### Theorem (Well-Definedness)

For all $s \in S'$: $\rho(s)$ is defined, $\rho(s) \neq \bot$, and $\rho(s) \trianglelefteq s$.

*Proof sketch:* Case 2's maximum exists since $\text{Roots}(s) \subseteq \text{ancestors}(s)$ which is finite and totally ordered. Case 3's LCA exists by tree properties. Case 4's orphanage is well-defined: $\text{SourceDirs} \cap \text{ancestors}(s)$ is non-empty (contains at least $\text{parent}(s)$) and finite, so the minimum exists. Case 5 handles the degenerate case where $s$ is at filesystem root. ∎

### Theorem (Marker Precedence)

Markers always take precedence over orphanage computation:
$$\text{Roots}(s) \neq \emptyset \implies \rho(s) \in \text{MarkerDirs}$$

*Proof:* Case 2 has priority over Cases 3–4.

---

## 7. Algorithm

```python
from pathlib import Path

EXCLUSIONS = {
    '.venv', 'venv', 'node_modules', '__pycache__', 'site-packages',
    '.tox', 'dist', 'build', '.egg-info', '.mypy_cache', '.pytest_cache',
    '.ruff_cache', 'target', 'vendor', '.gradle'
}

MARKERS = {
    '.git', '.hg', '.svn', 'pyproject.toml', 'setup.py', 'package.json',
    'Cargo.toml', 'go.mod', 'pom.xml', 'build.gradle', 'CMakeLists.txt',
    'deno.json', 'composer.json', 'mix.exs', 'Gemfile', 'BUILD', 'WORKSPACE'
}

_exclusion_cache: dict[Path, bool] = {}
_marker_cache: dict[Path, bool] = {}


def is_excluded(path: Path) -> bool:
    """Check if resolved path passes through any exclusion boundary."""
    try:
        resolved = path.resolve()
    except OSError:
        return True
    
    if resolved in _exclusion_cache:
        return _exclusion_cache[resolved]
    
    result = bool(EXCLUSIONS & set(resolved.parts))
    _exclusion_cache[resolved] = result
    return result


def has_marker(directory: Path) -> bool:
    """Check if directory contains any project marker.
    
    Permission errors are treated as marker-not-present.
    """
    if directory in _marker_cache:
        return _marker_cache[directory]
    
    try:
        result = any((directory / m).exists() for m in MARKERS)
    except OSError:
        result = False
    
    _marker_cache[directory] = result
    return result


def _find_root_single(
    source_file: Path,
    source_dirs: set[Path],
    dependency_cluster: set[Path] | None = None
) -> Path | None:
    """
    Internal: Compute project root for a single source file.
    
    Use compute_all_roots() for batch processing with orphanage support.
    """
    # Case 1: Excluded file
    if is_excluded(source_file):
        return None
    
    # Case 2: Walk up looking for markers (innermost wins)
    for ancestor in source_file.parents:
        if ancestor.name in EXCLUSIONS:
            break
        if has_marker(ancestor):
            return ancestor
        if ancestor.parent == ancestor:
            break
    
    # Case 3: Check dependency cluster for LCA
    if dependency_cluster and len(dependency_cluster) > 1:
        valid_files = {f for f in dependency_cluster if not is_excluded(f)}
        if len(valid_files) > 1:
            common = None
            for f in valid_files:
                try:
                    resolved = f.resolve()
                except OSError:
                    continue
                parents = set(resolved.parents) | {resolved.parent}
                common = parents if common is None else (common & parents)
            if common:
                return max(common, key=lambda p: len(p.parts))
    
    # Case 4: Orphan — find outermost SourceDir in ancestry
    ancestor_source_dirs = [
        d for d in source_file.parents
        if d in source_dirs and d.name not in EXCLUSIONS
    ]
    if ancestor_source_dirs:
        # min by ⊴ (outermost) = last in parents traversal
        return ancestor_source_dirs[-1]
    
    # Case 5: Filesystem root edge case
    parent = source_file.parent
    return parent if parent != source_file else source_file


def compute_all_roots(
    source_files: set[Path],
    dependency_clusters: dict[Path, set[Path]] | None = None
) -> dict[Path, Path | None]:
    """
    Compute roots for all source files.
    
    This is the primary API. It computes SourceDirs upfront to enable
    the orphanage rule for proper grouping of unmarked files.
    
    Args:
        source_files: Set of all source file paths
        dependency_clusters: Optional mapping from file to its import cluster
    
    Returns:
        Mapping from source file to its project root (or None if excluded)
    """
    # Filter excluded files
    valid_files = {f for f in source_files if not is_excluded(f)}
    
    # Compute SourceDirs (enables orphanage detection)
    source_dirs = {f.parent for f in valid_files}
    
    # Compute root for each file
    results = {}
    for f in source_files:
        cluster = dependency_clusters.get(f) if dependency_clusters else None
        results[f] = _find_root_single(f, source_dirs, cluster)
    
    return results


# Complexity: O(n × d + d × |MARKERS|) with caching, where n = file count, d = max depth.
# Without caching: O(n × d × |MARKERS|).
```

### Implementation Considerations

1. **Primary API:** Use `compute_all_roots()` for correct orphanage behavior. The internal `_find_root_single()` requires `source_dirs` to be pre-computed.

2. **Symlink cycles:** Handled by try/except around `Path.resolve()`.

3. **Permission errors:** Treated as marker-not-present (fail open).

4. **Case sensitivity:** On Windows/macOS, consider normalizing with `.lower()`.

5. **Caching:** Both `is_excluded()` and `has_marker()` cache results. Clear caches for long-running processes when filesystem changes.

6. **Configurability:** $\mathcal{M}$ and $\mathcal{E}$ should be configurable via config file or environment.

7. **Platform paths:** This implementation assumes POSIX-style paths. Windows drive letters (e.g., `C:\`) and UNC paths (e.g., `\\server\share`) may require additional handling—see Non-Goals.

---

## 8. Examples

| Scenario | Input | Output | Reason |
|----------|-------|--------|--------|
| File in marked dir | `project/src/main.py` (`.git` at `project/`) | `project/` | Case 2: marker |
| Nested markers | `mono/pkg/app/index.ts` (both have `package.json`) | `mono/pkg/app/` | Case 2: innermost |
| Orphan with root source | `api/app/model/user.py` (`main.py` at `api/`) | `api/` | Case 4: orphanage |
| Deep orphan | `api/a/b/c/file.py` (`main.py` at `api/`) | `api/` | Case 4: same orphanage |
| Isolated orphan | `/tmp/scripts/test.py` (no other sources) | `scripts/` | Case 4: parent fallback |
| Hidden project | `api/lib/hidden/.git` + `api/lib/hidden/src/app.py` | `hidden/` | Case 2: marker wins |
| Excluded | `.venv/lib/flask/app.py` | $\bot$ | Case 1 |

---

## 9. Worked Examples

### Example 1: Orphanage Detection

```
Filesystem:
api-web2text/              ← main.py here (SourceDir)
├── main.py
├── app/
│   └── api/
│       └── model/
│           └── user.py    ← query file
└── services/
    └── auth.py

SourceDirs = {api-web2text/, model/, services/}
```

**Trace for `user.py`:**
```
1. is_excluded? No
2. Walk up for markers:
   - model/        → no marker
   - api/          → no marker
   - app/          → no marker
   - api-web2text/ → no marker
   → No markers found, proceed to Case 4

3. Find outermost SourceDir in ancestry:
   - Ancestors: model/, api/, app/, api-web2text/, ...
   - Ancestors ∩ SourceDirs = {model/, api-web2text/}
   - Outermost (min ⊴) = api-web2text/

Result: api-web2text/
```

All files (`main.py`, `user.py`, `auth.py`) share root `api-web2text/`.

---

### Example 2: Marker Takes Precedence

```
Filesystem:
api-web2text/
├── main.py
├── app/
│   └── model/
│       └── user.py
└── lib/
    └── hidden-project/
        ├── .git           ← MARKER
        └── src/
            └── app.py     ← query file
```

**Trace for `app.py`:**
```
1. is_excluded? No
2. Walk up for markers:
   - src/            → no marker
   - hidden-project/ → HAS .git → RETURN hidden-project/

Result: hidden-project/
```

The marker at `hidden-project/` wins. Case 2 takes precedence.

---

### Example 3: Isolated Script

```
Filesystem:
/tmp/
└── scratch/
    └── test.py    ← only source file

SourceDirs = {scratch/}
```

**Trace for `test.py`:**
```
1. is_excluded? No
2. Walk up for markers: none found
3. No dependency cluster
4. Ancestors ∩ SourceDirs = {scratch/}
   Outermost = scratch/

Result: scratch/
```

---

### Example 4: Nested Orphanages

```
Filesystem:
~/code/
├── project-a/
│   ├── .git         ← marker
│   └── src/
│       └── main.py
└── orphan-stuff/    ← no marker
    ├── utils.py     ← SourceDir = orphan-stuff/
    └── app/
        └── file.py

SourceDirs = {src/, orphan-stuff/, app/}
```

**Trace for `file.py` (in orphan-stuff/app/):**
```
1. is_excluded? No
2. Walk up for markers:
   - app/          → no marker
   - orphan-stuff/ → no marker
   - ~/code/       → no marker
   → No markers found

3. Find outermost SourceDir in ancestry:
   - Ancestors: app/, orphan-stuff/, ~/code/, ...
   - Ancestors ∩ SourceDirs = {app/, orphan-stuff/}
   - Outermost = orphan-stuff/

Result: orphan-stuff/
```

Note: `project-a/` has a marker but is not in `file.py`'s ancestry, so it doesn't affect the result.

---

## 10. Properties

### Theorem (Stability)

Adding files outside exclusion zones does not change roots determined by markers:
$$\text{Roots}(s) \neq \emptyset \implies \rho(s) \text{ is independent of } S$$

*Note:* Orphanage roots may change as $\text{SourceDirs}$ changes with new files.

### Theorem (Exclusion Monotonicity)

Expanding the exclusion set can only exclude more files:
$$\mathcal{E} \subseteq \mathcal{E}' \implies \mathcal{X} \subseteq \mathcal{X}'$$

### Theorem (Marker Precedence)

Markers always override orphanage computation:
$$\text{Roots}(s) \neq \emptyset \implies \rho(s) = \max_{\trianglelefteq}(\text{Roots}(s))$$

### Theorem (Orphanage Grouping)

Orphan files under the same outermost SourceDir share a root:
$$\text{orphan}(s_1) \land \text{orphan}(s_2) \land \text{orphanage}(s_1) = \text{orphanage}(s_2) \implies \rho(s_1) = \rho(s_2)$$

---

## 11. Non-Goals

This specification explicitly does not address:

1. **Language/framework detection** — Determining *which* language or build system a project uses is a separate concern.

2. **Workspace/solution files** — IDE project files (`.sln`, `.xcworkspace`) that span multiple roots require higher-level orchestration.

3. **Non-filesystem structures** — Projects stored in databases, archives, or virtual filesystems are out of scope.

4. **Build graph construction** — While we use dependency relation $D$ for orphan handling, constructing $D$ via static analysis is external to this spec.

5. **Remote/distributed filesystems** — The algorithm assumes local filesystem semantics.

6. **Windows-specific path handling** — Drive letters (`C:\`), UNC paths (`\\server\share`), and junction points may require platform-specific adaptation. The algorithm's logic is portable, but path parsing details vary by OS.

---

## 12. Summary

This specification provides a robust algorithm for project root detection that:

1. **Excludes artifacts** — Virtual environments, `node_modules`, build outputs, and caches are never treated as project sources

2. **Respects markers** — Directories with `.git`, `pyproject.toml`, etc. are recognized as project roots (innermost wins)

3. **Handles symlinks** — Resolution before exclusion checking allows editable installs to work correctly

4. **Groups orphans** — Files without markers share the outermost directory containing source files as their root (orphanage rule)

5. **Maintains precedence** — Markers always take precedence over orphanage computation

---

## Appendix A: Quick Reference

### Case Priority

| Priority | Case | Condition | Result |
|----------|------|-----------|--------|
| 1 | Excluded | $s \in \mathcal{X}$ | $\bot$ |
| 2 | Marked | $\text{Roots}(s) \neq \emptyset$ | Innermost marker directory |
| 3 | Cluster | $|\text{closure}_D(s) \cap S'| > 1$ | LCA of dependency cluster |
| 4 | Orphan | $\text{parent}(s) \neq s$ | Outermost SourceDir in ancestry |
| 5 | Root | Otherwise | $s$ (file at filesystem root) |

### The Orphanage Rule

*"Source files mark their parent as a root candidate. For orphans, the outermost such directory wins."*

$$\text{orphanage}(s) = \min_{\trianglelefteq}(\text{SourceDirs} \cap \text{ancestors}(s))$$

*Notation:* $\min_{\trianglelefteq}$ = outermost (closest to root); $\max_{\trianglelefteq}$ = innermost (closest to file).

### Default Sets

**Exclusions** ($\mathcal{E}$): `.venv`, `venv`, `node_modules`, `__pycache__`, `site-packages`, `.tox`, `dist`, `build`, `.egg-info`, `.mypy_cache`, `.pytest_cache`, `.ruff_cache`, `target`, `vendor`, `.gradle`

**Markers** ($\mathcal{M}$): `.git`, `.hg`, `.svn`, `pyproject.toml`, `setup.py`, `package.json`, `Cargo.toml`, `go.mod`, `pom.xml`, `build.gradle`, `CMakeLists.txt`, `deno.json`, `composer.json`, `mix.exs`, `Gemfile`, `BUILD`, `WORKSPACE`

*Note:* `Makefile` excluded by default—commonly appears in subdirectories.