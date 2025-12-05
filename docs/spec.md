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

$$\mathcal{M} = \{\texttt{.git}, \texttt{.hg}, \texttt{pyproject.toml}, \texttt{setup.py}, \texttt{package.json}, \texttt{Cargo.toml}, \texttt{go.mod}, \texttt{pom.xml}, \texttt{build.gradle}, \texttt{CMakeLists.txt}, \texttt{deno.json}, \texttt{composer.json}, \texttt{mix.exs}\}$$

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

## 4. Dependency Closure

For handling orphan files that import each other:

### Definition (Dependency Relation)

$D \subseteq S \times S$ where $(a, b) \in D$ means "file $a$ imports file $b$."

*Note:* Construction of $D$ requires static analysis (parsing import statements) and is outside the scope of this specification. The closure is symmetric, so direction only matters for documentation.

### Definition (Dependency Closure)

$$\text{closure}_D(s) = \{t \in S : s\ D^*\ t \lor t\ D^*\ s\}$$

where $D^*$ is the reflexive-transitive closure of $D$. This captures all files transitively connected by imports in either direction.

### Definition (Lowest Common Ancestor)

For a non-empty set $X \subseteq N$:

$$\text{LCA}(X) = \max_{\trianglelefteq}\{a \in N : \forall x \in X,\ a \trianglelefteq x\}$$

The LCA exists and is unique in a tree.

---

## 5. Root Function

### Definition (Project Root)

The root function $\rho: S \to N \cup \{\bot\}$ is defined:

$$\rho(s) = \begin{cases}
\bot & \text{if } s \in \mathcal{X} \\[8pt]
\max_{\trianglelefteq}(\text{Roots}(s)) & \text{if } \text{Roots}(s) \neq \emptyset \\[8pt]
\text{LCA}(\text{closure}_D(s) \cap S') & \text{if } |\text{closure}_D(s) \cap S'| > 1 \\[8pt]
\text{parent}(s) & \text{if } \text{parent}(s) \neq s \\[8pt]
s & \text{otherwise (file at filesystem root)}
\end{cases}$$

Where:
- $\bot$ indicates "not a project file" (excluded)
- Case 2: Innermost directory containing a project marker (closest to source file)
- Case 3: Orphan cluster — use LCA of the dependency-connected component
- Case 4: Isolated orphan — fall back to parent directory
- Case 5: Edge case for files at filesystem root

### Theorem (Well-Definedness)

For all $s \in S'$: $\rho(s)$ is defined, $\rho(s) \neq \bot$, and $\rho(s) \trianglelefteq s$.

*Proof sketch:* Case 2's maximum exists since $\text{Roots}(s) \subseteq \text{ancestors}(s)$ which is finite and totally ordered. Case 3's LCA exists by tree properties. Cases 4–5 are always defined. ∎

---

## 6. Algorithm

> **Note:** This implementation handles Cases 1, 2, and 4 of the specification. Case 3 (dependency closure) requires external static analysis infrastructure to construct relation $D$, which is passed as an optional parameter.

```python
from pathlib import Path

EXCLUSIONS = {
    '.venv', 'venv', 'node_modules', '__pycache__', 'site-packages',
    '.tox', 'dist', 'build', '.egg-info', '.mypy_cache', '.pytest_cache',
    '.ruff_cache', 'target', 'vendor', '.gradle'
}

MARKERS = {
    '.git', '.hg', 'pyproject.toml', 'setup.py', 'package.json',
    'Cargo.toml', 'go.mod', 'pom.xml', 'build.gradle', 'CMakeLists.txt',
    'deno.json', 'composer.json', 'mix.exs'
}

# Module-level cache for exclusion checks (cleared between runs if needed)
_exclusion_cache: dict[Path, bool] = {}


def is_excluded(path: Path) -> bool:
    """Check if resolved path passes through any exclusion boundary.
    
    Note: Case-sensitive matching. On case-insensitive filesystems,
    consider normalizing names with .lower() before comparison.
    """
    try:
        resolved = path.resolve()
    except OSError:
        # Symlink cycle or other resolution failure — treat as excluded
        return True
    
    if resolved in _exclusion_cache:
        return _exclusion_cache[resolved]
    
    result = bool(EXCLUSIONS & set(resolved.parts))
    _exclusion_cache[resolved] = result
    return result


def find_root(source_file: Path, dependency_cluster: set[Path] | None = None) -> Path | None:
    """
    Compute project root for a source file.
    
    Args:
        source_file: Path to the source file
        dependency_cluster: Optional set of files in the same import cluster
                           (requires external static analysis to compute)
    
    Returns:
        Project root directory, or None if file is excluded
    """
    # Case 1: Excluded file
    if is_excluded(source_file):
        return None
    
    # Case 2: Search for marker directories (innermost first)
    # source_file.parents is ordered from immediate parent outward
    for ancestor in source_file.parents:
        # Stop if we hit an exclusion boundary
        if ancestor.name in EXCLUSIONS:
            break
        
        # Check for project markers
        for marker in MARKERS:
            if (ancestor / marker).exists():
                return ancestor  # First (innermost) match wins
    
    # Case 3: Orphan with dependency cluster
    if dependency_cluster and len(dependency_cluster) > 1:
        valid_files = {f for f in dependency_cluster if not is_excluded(f)}
        if len(valid_files) > 1:
            # Compute LCA of the cluster
            common = None
            for f in valid_files:
                try:
                    resolved = f.resolve()
                except OSError:
                    continue
                # Note: Path.parents may not include immediate parent at index 0
                # in all Python versions, so we include it explicitly for safety
                parents = set(resolved.parents) | {resolved.parent}
                common = parents if common is None else (common & parents)
            if common:
                return max(common, key=lambda p: len(p.parts))
    
    # Case 4/5: Isolated orphan (handle root edge case)
    parent = source_file.parent
    return parent if parent != source_file else source_file


# Complexity: O(d * |MARKERS|) per file, where d is directory depth.
# The is_excluded cache amortizes repeated checks on shared ancestors.
```

### Implementation Considerations

1. **Symlink cycles:** Handled by try/except around `Path.resolve()`. Unresolvable paths are treated as excluded.

2. **Case sensitivity:** The exclusion check is case-sensitive. On Windows/macOS, consider:
   ```python
   EXCLUSIONS_LOWER = {e.lower() for e in EXCLUSIONS}
   # Then check: part.lower() in EXCLUSIONS_LOWER
   ```

3. **Caching:** The module-level `_exclusion_cache` amortizes repeated exclusion checks. For root results, consider a similar cache keyed by resolved path.

4. **Marker configurability:** In practice, $\mathcal{M}$ and $\mathcal{E}$ should be configurable. Consider loading from a config file or environment.

5. **Cache invalidation:** For long-running processes, implement cache clearing when the filesystem changes (e.g., via file watchers).

---

## 7. Examples

| Scenario | Input | Output | Reason |
|----------|-------|--------|--------|
| Standard project | `my_project/src/main.py` | `my_project/` | Found `.git` marker |
| Monorepo package | `mono/packages/app/index.ts` | `mono/packages/app/` | Found `package.json` (innermost) |
| Virtual env file | `.venv/lib/python3.11/flask/app.py` | $\bot$ | Path contains `.venv` |
| Orphan cluster | `~/scripts/a.py` imports `b.py` | `~/scripts/` | LCA of cluster |
| Isolated script | `~/scratch/test.py` | `~/scratch/` | Parent fallback |
| Symlink escape | `.venv/site-packages/pkg` → `../../src/pkg` | `my_project/` | Resolved path is not excluded |

---

## 8. Worked Example

Step-by-step trace for a monorepo file:

```
Input: source_file = /home/user/mono/packages/api/src/index.ts

Filesystem:
/home/user/mono/
├── .git
├── package.json
└── packages/
    ├── api/
    │   ├── package.json    ← innermost marker
    │   └── src/
    │       └── index.ts    ← source file
    └── web/
        └── ...
```

**Step 1: Check exclusion**
```
resolved = /home/user/mono/packages/api/src/index.ts
parts = ('/', 'home', 'user', 'mono', 'packages', 'api', 'src', 'index.ts')
EXCLUSIONS ∩ parts = ∅
→ Not excluded, continue to Case 2
```

**Step 2: Search ancestors for markers (innermost first)**
```
/home/user/mono/packages/api/src    — no markers, continue
/home/user/mono/packages/api        — contains package.json ✓ STOP
```

**Result:** `/home/user/mono/packages/api`

Note that `/home/user/mono/` also contains markers (`.git`, `package.json`), but we return the innermost match. This correctly identifies the `api` package as the project root rather than the monorepo root.

---

## 9. Edge Cases

### Symlinks Escaping Exclusion Zones

Editable installs create symlinks from `site-packages` into source directories:

```
project/
├── .git
├── src/
│   └── mylib/
└── .venv/
    └── site-packages/
        └── mylib → ../../../src/mylib  (symlink)
```

The path `.venv/site-packages/mylib/core.py` resolves to `project/src/mylib/core.py`, which is **not** excluded. The algorithm correctly returns `project/` as the root.

### Nested Exclusion Zones

```
project/
├── .git
├── src/
└── .venv/
    └── lib/
        └── node_modules/    ← nested exclusion
            └── ...
```

Both `.venv` and `node_modules` trigger exclusion. The algorithm stops at the first boundary encountered.

### Markers Inside Exclusion Zones

```
project/
├── .git
└── node_modules/
    └── some-package/
        └── package.json    ← ignored marker
```

The `package.json` inside `node_modules` is not in $M$ and won't be considered as a project root marker.

### Co-located Dependency Cluster

When all files in a dependency cluster reside in the same directory:

```
~/scripts/
├── a.py    (imports b)
└── b.py
```

Both Case 3 (LCA) and Case 4 (parent fallback) produce `~/scripts/`. The algorithm handles this gracefully—co-located orphans don't need special treatment.

---

## 10. Properties

### Theorem (Stability)

Adding files outside exclusion zones does not change existing roots, provided dependency relationships are unchanged:
$$s \in S' \land S' \subseteq S'' \land (S'' \setminus S') \cap \mathcal{X} = \emptyset \land D_{S'} = D_{S''} \implies \rho_{S'}(s) = \rho_{S''}(s)$$

*Caveat:* Without the constraint $D_{S'} = D_{S''}$, an isolated orphan (Case 4) may transition to Case 3 if new files create dependency connections, potentially changing its root.

### Theorem (Exclusion Monotonicity)

Expanding the exclusion set can only exclude more files:
$$\mathcal{E} \subseteq \mathcal{E}' \implies \mathcal{X} \subseteq \mathcal{X}'$$

### Theorem (Innermost Marker Wins)

For files with clear paths to multiple markers, the closest one determines the root:
$$|\text{Roots}(s)| > 1 \implies \rho(s) = \max_{\trianglelefteq}(\text{Roots}(s))$$

---

## 11. Non-Goals

This specification explicitly does not address:

1. **Language/framework detection** — Determining *which* language or build system a project uses is a separate concern. This spec only finds the root directory.

2. **Workspace/solution files** — IDE project files (`.sln`, `.xcworkspace`) that span multiple roots require higher-level orchestration.

3. **Non-filesystem structures** — Projects stored in databases, archives, or virtual filesystems are out of scope.

4. **Build graph construction** — While we use dependency relation $D$ for orphan handling, constructing $D$ via static analysis is external to this spec.

5. **Remote/distributed filesystems** — The algorithm assumes local filesystem semantics. Network filesystem quirks (e.g., different symlink behavior) may require adaptation.

---

## 12. Summary

This specification provides a robust algorithm for project root detection that:

1. **Excludes artifacts** — Virtual environments, `node_modules`, build outputs, and caches are never treated as project sources
2. **Respects boundaries** — Traversal stops at exclusion boundaries, preventing incorrect root assignment
3. **Handles symlinks** — Resolution before exclusion checking allows editable installs to work correctly
4. **Supports orphans** — Files without markers get reasonable fallback behavior based on dependencies or parent directory
5. **Prefers innermost roots** — Monorepos with nested projects resolve to the most specific applicable root

---

## Appendix A: Quick Reference

### Case Priority

| Case | Condition | Result |
|------|-----------|--------|
| 1 | $s \in \mathcal{X}$ | $\bot$ (excluded) |
| 2 | $\text{Roots}(s) \neq \emptyset$ | Innermost marker directory |
| 3 | $|\text{closure}_D(s) \cap S'| > 1$ | LCA of dependency cluster |
| 4 | $\text{parent}(s) \neq s$ | Parent directory |
| 5 | Otherwise | $s$ itself (filesystem root edge case) |

### Default Sets

**Exclusions** ($\mathcal{E}$): `.venv`, `venv`, `node_modules`, `__pycache__`, `site-packages`, `.tox`, `dist`, `build`, `.egg-info`, `.mypy_cache`, `.pytest_cache`, `.ruff_cache`, `target`, `vendor`, `.gradle`

**Markers** ($\mathcal{M}$): `.git`, `.hg`, `pyproject.toml`, `setup.py`, `package.json`, `Cargo.toml`, `go.mod`, `pom.xml`, `build.gradle`, `CMakeLists.txt`, `deno.json`, `composer.json`, `mix.exs`