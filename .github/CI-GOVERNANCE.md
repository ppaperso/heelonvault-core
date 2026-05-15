# CI/Release Governance Policy

This document defines the **Tier-based CI strategy** for HeelonVault, balancing development velocity, platform coverage, and release quality.

## Overview

The CI pipeline is organized into **5 execution tiers**, each with distinct triggers, scope, and objectives:

```
┌─────────────────────────────────────────────────────────────────┐
│ TRIGGER                 │ TIER 1 │ TIER 2 │ TIER 3 │ TIER 4 │ T5 │
├─────────────────────────────────────────────────────────────────┤
│ Pull Request (PR)       │   ✓    │   ✓    │   ✗    │   ✗    │ ✗  │
│ Push develop            │   ✓    │   ✗    │   ✓    │   ✓    │ ✗  │
│ Push main               │   ✓    │   ✗    │   ✓    │   ✓    │ ✗  │
│ Push tags (v*)          │   ✓    │   ✗    │   ✓    │   ✓    │ ✗  │
│ workflow_dispatch       │   ✓    │   ✗    │   ✓    │   ✓    │ ✓  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tier 1: Code Quality & Validation (Always)

**Duration**: ~2 min | **Always runs** on PR, push, tags, dispatch

### Objective
Fast code quality gates before any build. Fail-fast on obvious issues.

### Jobs
- `validate-source`: Rust formatting (`cargo fmt`), linting (`cargo clippy -D warnings`), desktop entry validation

### Rationale
- Prevents broken builds from reaching expensive build jobs
- Runs on all triggers (minimal resource cost)
- **No platform-specific dependencies** (runs on lightweight ubuntu-24.04)

---

## Tier 2: Dev Platform Build (PR Feedback Only)

**Duration**: ~4 min | **PR only** | Allégé (build validation, no smoke tests)

### Objective
Give developers fast feedback on their target platform (Fedora).

### Jobs
- `build-fedora-dev`: Build release binary on Fedora container, validate desktop file

### Why only Fedora?
- **Your production target** (where HeelonVault is deployed)
- **Faster iteration**: No unnecessary macOS/Windows builds on PR
- **Container-based**: Reproducible, isolated environment
- **Smoke tests skipped**: Adds 2+ min per PR without additional signal during active development

### Rationale for skipping Tier 3 on PR
- Cross-platform validation (macOS, Windows) is expensive (10+ min total)
- Risk of issues is low when Tier 1 + Fedora pass
- **Reserve expensive jobs for push/release** where CI resources are less critical

---

## Tier 3: Build Matrix (Push/Release Validation)

**Duration**: ~12 min (parallel) | **Push/tags only**

### Objective
Comprehensive cross-platform validation before release/integration into main.

### Jobs
- `build-linux-reference`: Ubuntu 24.04 (golden standard Linux platform)
  - Compile tests (`cargo test --no-run`)
  - Full smoke tests (install/uninstall)
  - Distributable artifacts
  
- `build-fedora-production`: Fedora container (production target)
  - Build validation
  - Smoke tests (install/uninstall)

- `build-macos`: macOS 14 (Apple Silicon arm64)
  - Build binary validation
  - Linkage check (`otool -L`)

- `build-windows`: Windows 2022 + MSYS2
  - Build binary validation
  - Binary presence check

### Why separate Linux reference + Fedora production?
- **Ubuntu (reference)**: Canonical Linux distro, validates mainstream glibc/package ecosystem
- **Fedora (production)**: Your actual deployment target, ensures real-world smoke tests pass
- **Both are needed** to catch distro-specific build/runtime issues

### Rationale
- **Parallel execution** keeps wall-time manageable (~12 min)
- **Expensive**: Worth the cost only on integration points (main/release), not every PR
- **Confidence**: All major platforms validated before release

---

## Tier 4: Release Gates (Supply-Chain & SBOM)

**Duration**: ~3 min | **Push/tags only** | **Blocks merge to main**

### Objective
Enforce supply-chain and dependency integrity before code integrates into main.

### Jobs
- `check-sbom`: Verify `sbom.cyclonedx.json` matches fresh dependency tree
  - Fails if dependencies changed but SBOM not regenerated
  - Blocks merge to prevent outdated BOM in main branch

### Rationale
- **Dependency governance**: Enforces explicit review of dependency changes
- **Release readiness**: Ensures SBOM always matches binary artifacts
- **Cost**: Cheap (no rebuild, just verification)
- **Timing**: Only on push/release; no point checking on PR (can't merge anyway)

---

## Tier 5: Manual Force (workflow_dispatch)

**Duration**: Varies | **Manual trigger only**

### Objective
Escape hatch: Force full CI when needed (e.g., GitHub infrastructure issue, manual verification).

### What runs
- All Tiers 1–4

### Rationale
- **Edge cases**: Unblock situations where automated CI is insufficient
- **Not intended for routine use** (that's why it's Tier 5)

---

## Execution Flow by Trigger

### Pull Request
```
1. Tier 1: validate-source (must pass)
   ├─ If fails → PR cannot proceed
   └─ If passes → Continue
2. Tier 2: build-fedora-dev (must pass)
   └─ If passes → PR ready for review + merge
```
**Total time**: ~6 min

### Push main / Push develop
```
1. Tier 1: validate-source (must pass)
2. Tier 3: [build-linux-reference, build-fedora-production, build-macos, build-windows] (parallel, all must pass)
3. Tier 4: check-sbom (must pass, depends on Tier 3)
```
**Total time**: ~15 min (parallel Tier 3)

### Push tags (v*)
```
Same as push main/develop. Tags are treated as release anchors.
```

### Manual dispatch
```
All Tiers 1–5 run sequentially. Used for manual override/verification.
```

---

## Decision Matrix: Should I run X job on Y trigger?

| Question | Answer | Job Move |
|----------|--------|----------|
| Is it cheap and adds safety? | Yes → Run on PR | Tier 1 ✓ |
| Is it dev feedback (not release)? | Yes → Run on PR, Tier 2 | Dev platform only |
| Is it expensive but necessary for release? | Yes → Run only on push | Tier 3–4 |
| Is it a rare override? | Yes → workflow_dispatch | Tier 5 |

---

## Optimization Tips

### To speed up PR feedback
- Keep Tier 2 on PR only
- Skip expensive macOS/Windows validation on PR (do on push)
- Pre-cache dependencies (see `rust-cache@v2` config)

### To ensure release quality
- Tier 3 validates all platforms (required for production release)
- Tier 4 enforces SBOM/supply-chain checks (no exceptions)
- Use branch protection rules: require all Tier 3/4 checks before merge to main

### To handle platform-specific issues
- PR fails on Fedora? Fix, re-push → Tier 2 re-runs
- Merge to main, then macOS fails? Keep commit, revert, fix, re-push → Tier 3/4 re-runs
- Never bypass Tier 3/4 for main branch

---

## Related Files

- **Workflow definition**: [.github/workflows/ci.yml](.github/workflows/ci.yml)
- **Build dependencies**: See per-job `Install *-build-dependencies` steps
- **Supply-chain policy**: [deny.toml](../../deny.toml)
- **SBOM generation**: [scripts/generate-sbom.sh](../../scripts/generate-sbom.sh)

---

## Future Enhancements

- **Tier 3 split**: Separate `build-cross-platform` (release-only) from `build-reference` (push-only)
- **Artifact retention**: Store binaries from main/tags for download
- **Performance metrics**: Track Tier 3 wall-time to optimize further
- **Container update**: Monitor Fedora:latest version changes in CI logs
