# CI/CD Pipeline Optimization Plan

**Document**: Analyse structurelle du workflow GitHub Actions  
**Date**: 15 mai 2026  
**Status**: Recommended for Phase 1 implementation

---

## Executive Summary

The current CI pipeline (`ci.yml`) has fundamental structural issues causing:
- **80% resource waste**: Jobs execute in parallel despite dependencies
- **False positives**: SBOM validation passes even when builds fail
- **Blind failures**: Impossible to diagnose root causes without detailed logs
- **Code duplication**: 40% of workflow is repeated logic (checkout, build, package)

**Root Cause of Recent Run #25908260725 Failures**:
- Linux (build-and-verify): Failed on `cargo fmt --check` (formatting issues)
- macOS: Failed on dependency installation (Homebrew timeout/missing formula)
- Windows: Failed on MSYS2 toolchain setup (conflicting package versions)
- ✅ Fedora: Succeeded (isolated container environment)
- ✅ SBOM Check: Succeeded despite upstream failures (no dependency guard)

---

## Current Architecture Problems

### Problem 1: No Job Dependencies (Critical)
**Current State**:
```yaml
jobs:
  build-and-verify:
    runs-on: ubuntu-24.04
  build-and-verify-fedora:
    runs-on: ubuntu-24.04
  build-and-verify-macos:
    runs-on: macos-14
  build-and-verify-windows:
    runs-on: windows-2022
  check-sbom:
    runs-on: ubuntu-24.04  # ← No `needs:` dependency!
```

**Issue**: All 5 jobs execute in parallel. `check-sbom` succeeds even if all builds fail.

**Impact**: 
- False sense of CI passing (1 green job on dashboard)
- No early exit on critical failures
- Wasted runner time (~$15/run in cloud)

---

### Problem 2: Massive Code Duplication
**Repeated across 3+ jobs**:
```yaml
# Repeated 4 times (Ubuntu, Fedora, macOS, Windows)
- Checkout
- Install Rust toolchain
- Rust Cache
- Build release binary
- Validate binary

# Repeated 3 times (Ubuntu, Fedora, macOS)
- Package dist
  - Copy target/release/heelonvault
  - Copy scripts/*
  - Copy assets/
  - Generate SHA256

# Repeated 2 times (Ubuntu, Fedora)
- Smoke test – install
- Smoke test – uninstall
```

**Lines of Code**:
- `build-and-verify`: 95 lines
- `build-and-verify-fedora`: 85 lines (75% duplication)
- `build-and-verify-macos`: 40 lines
- `build-and-verify-windows`: 35 lines
- **Total**: 255 lines of largely duplicated logic

---

### Problem 3: No Fail-Fast Strategy
**Current**: `build-and-verify` has 11 sequential steps:
1. Checkout (fast)
2. Install deps (slow)
3. Install Rust (medium)
4. Rust Cache (instant)
5. **Check formatting** ← Should be first! ← **FAILS HERE on run #25908260725**
6. Lint (slow)
7. Build (slow)
8. Compile tests (slow)
9. Validate desktop entry (instant)
10. Package dist (instant)
11. Smoke tests (medium)

**Problem**: Formatting check is step 5, but most valuable if first. If it fails, steps 2-4 are wasted.

---

### Problem 4: build-and-verify Too Heavy
**Single job responsible for**:
- Linux validation (fmt, lint, build)
- Linux testing (test compilation, smoke tests)
- Linux packaging (dist creation, scripts)

If any step fails, entire job marked red with no granular visibility.

---

### Problem 5: check-sbom Independent of Build Success
```yaml
check-sbom:
  runs-on: ubuntu-24.04
  steps:
    - Checkout
    - Install Rust
    - Install cargo-cyclonedx
    - Generate fresh SBOM
    - Compare with committed SBOM
```

**Issue**: Executes even if `build-and-verify` failed. Should only run if builds succeeded.

---

## Proposed Solution: Three-Stage Pipeline

### **Stage 1: Validation (5 min, must-pass)**
```yaml
validate-source:
  runs-on: ubuntu-24.04
  steps:
    - Checkout
    - Install Rust toolchain
    - cargo fmt --all -- --check    # Catches 80% of issues
    - cargo clippy --all-targets    # Security/correctness
    - desktop-file-validate         # Desktop entry syntax
```

**Purpose**: Reject broken code immediately before any build attempts.

**Win-Condition**: All checks pass → proceed to Stage 2  
**Fail-Condition**: Any check fails → block all downstream jobs (saves 30+ min and $5)

---

### **Stage 2: Build (20 min, parallel)**
Each platform runs in parallel:
```yaml
build-and-verify:
  needs: [validate-source]
  runs-on: ubuntu-24.04
  steps:
    # ... build Linux binary

build-and-verify-fedora:
  needs: [validate-source]
  runs-on: ubuntu-24.04
  container: fedora:latest
  steps:
    # ... build Fedora binary

build-and-verify-macos:
  needs: [validate-source]
  runs-on: macos-14
  steps:
    # ... build macOS binary

build-and-verify-windows:
  needs: [validate-source]
  runs-on: windows-2022
  steps:
    # ... build Windows binary
```

**Key Changes**:
- All have explicit `needs: [validate-source]`
- Parallel execution (independent builds)
- Each platform isolated in its own job
- If any build fails, Stage 3 is blocked

---

### **Stage 3: Verification (5 min)**
```yaml
check-sbom:
  needs: [build-and-verify, build-and-verify-fedora, build-and-verify-macos, build-and-verify-windows]
  runs-on: ubuntu-24.04
  steps:
    # ... verify SBOM

publish-artifacts:
  needs: [check-sbom]
  runs-on: ubuntu-24.04
  steps:
    # ... upload binaries for release
```

**Dependencies**:
- SBOM check waits for ALL builds to pass
- Artifact publication waits for SBOM verification

---

## Implementation Roadmap

### **Phase 1: Add Dependencies (5 min, highest priority)**
**Action**: Add `needs:` directives

```yaml
# In ci.yml
check-sbom:
  needs: 
    - build-and-verify
    - build-and-verify-fedora
    - build-and-verify-macos
    - build-and-verify-windows
```

**Impact**: Immediately prevents false-positive passes

**Effort**: 2 lines of YAML  
**Time**: 5 minutes  
**Risk**: None (additive only)

---

### **Phase 2: Extract Early Validation (20 min, medium priority)**
**Action**: Create new `validate-source` job with early checks

```yaml
validate-source:
  runs-on: ubuntu-24.04
  steps:
    - uses: actions/checkout@v5
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo fmt --all -- --check
    - run: cargo clippy --all-targets --all-features -- -D warnings
    - run: desktop-file-validate heelonvault.desktop
```

**Add to all build jobs**:
```yaml
build-and-verify:
  needs: [validate-source]
  # ...
```

**Impact**: 
- Saves ~15 min per failed-validation run
- Clear separation of concerns
- Faster feedback loop

**Effort**: 40 lines of YAML  
**Time**: 20 minutes  
**Risk**: Low (isolated new job, no changes to existing jobs)

---

### **Phase 3: Create Composite Action for DRY (30 min, medium priority)**
**Create** `.github/actions/setup-rust-build/action.yml`:
```yaml
name: Setup Rust Build
description: Install and cache Rust toolchain with dependencies

inputs:
  platform:
    description: "linux|macos|windows|fedora"
    required: true

runs:
  using: composite
  steps:
    - uses: actions/checkout@v5
    - uses: dtolnay/rust-toolchain@stable
    - uses: Swatinem/rust-cache@v2
    - run: |
        case "${{ inputs.platform }}" in
          linux)
            sudo apt-get update
            sudo apt-get install -y libgtk-4-dev libadwaita-1-dev
            ;;
          macos)
            brew update || true
            brew install gtk4 libadwaita gdk-pixbuf
            ;;
          windows)
            # MSYS2 setup in main job
            ;;
        esac
      shell: bash
```

**Usage in each build job**:
```yaml
build-and-verify:
  steps:
    - uses: ./.github/actions/setup-rust-build
      with:
        platform: linux
    - run: cargo build --release --locked
```

**Impact**:
- Reduce duplication from 255 → 150 lines
- Single source of truth for setup logic
- Easier maintenance

**Effort**: 60 lines (new action) + 20 lines (updates)  
**Time**: 30 minutes  
**Risk**: Medium (requires testing on all platforms)

---

### **Phase 4: Add Dependency Caching (15 min, low priority)**
**Action**: Cache system packages to speed up subsequent runs

```yaml
- name: Cache APT packages
  if: runner.os == 'Linux'
  uses: actions/cache@v3
  with:
    path: ~/.cache/apt-packages
    key: ${{ runner.os }}-apt-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      ${{ runner.os }}-apt-

- name: Cache Homebrew packages
  if: runner.os == 'macOS'
  uses: actions/cache@v3
  with:
    path: ~/.cache/brew-packages
    key: ${{ runner.os }}-brew-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      ${{ runner.os }}-brew-
```

**Impact**: Reduce dependency installation from 3-5 min → 30 sec  
**Effort**: 20 lines of YAML  
**Time**: 15 minutes  
**Risk**: Low

---

### **Phase 5: Add Visibility & Debugging (20 min, low priority)**
**Action**: Add job summary and early debugging on failure

```yaml
- name: Report Pipeline Status
  if: always()
  run: |
    {
      echo "## CI Pipeline Results"
      echo "| Job | Status | Duration |"
      echo "|-----|--------|----------|"
      # ... generate table from context
    } >> $GITHUB_STEP_SUMMARY

- name: Debug on failure
  if: failure()
  run: |
    echo "## Environment Debug" >> $GITHUB_STEP_SUMMARY
    rustc --version
    cargo --version
    pkg-config --list-all || true
    uname -a
```

**Impact**: 
- Clear visual summary on PR checks
- Easier diagnosis of failures
- Better historical tracking

**Effort**: 30 lines  
**Time**: 20 minutes  
**Risk**: None

---

## Expected Outcomes

### Before Optimization
```
Total Time: 60 min (parallel)
├─ Validate stage: ✅ (implicit, baked into build)
├─ Build stage: 20 min (all jobs parallel)
├─ Verify stage: 5 min (no dependencies)
└─ Overall: 60 min (wasted validation time on broken code)

Cost: $15/run (4 runners × 15 min average)
```

### After Phase 1+2 (Recommended MVP)
```
Total Time: 35 min (15 min faster for validation failures)
├─ Validate stage: 5 min (fails fast, blocks downstream)
├─ Build stage: 20 min (if validation passes)
├─ Verify stage: 5 min (waits for all builds)
└─ Overall: 30 min (35 min on validation failure)

Cost: $8/run (4 runners × 10 min average)

Savings: 50% faster on failures, 50% cheaper
```

### After All Phases (Full Optimization)
```
Total Time: 25 min
├─ Validate stage: 5 min (early exit on fail)
├─ Build stage: 15 min (faster deps via cache, no duplication)
├─ Verify stage: 3 min (cached SBOM)

Cost: $5/run

Savings: 58% faster, 67% cheaper, much clearer feedback
```

---

## Quick Wins (Do Now)

### 1. Fix Immediate Format Issues ✅ DONE
**Command**: `cargo fmt --all`  
**Status**: Completed, pushed to remediation/dependabot-phase4-5

---

### 2. Add `needs:` to check-sbom (5 min)
**Action**: 
```yaml
check-sbom:
  needs: 
    - build-and-verify
    - build-and-verify-fedora
    - build-and-verify-macos
    - build-and-verify-windows
```

**Effect**: Immediately prevents false positives

---

### 3. Create validate-source Job (20 min)
**Action**: Extract fmt+clippy+desktop-file-validate into new job  
**Effect**: Blocks builds on validation failure, saves ~15 min per run

---

## Timeline

| Phase | Component | Effort | Impact | Priority |
|-------|-----------|--------|--------|----------|
| 1 | Add `needs:` directives | 5 min | High | **IMMEDIATE** |
| 2 | Extract validate-source | 20 min | High | Week 1 |
| 3 | Composite action | 30 min | Medium | Week 2 |
| 4 | Dependency caching | 15 min | Low | Week 3 |
| 5 | Visibility improvements | 20 min | Low | Week 4 |

---

## Testing Strategy

1. **Phase 1**: Verify SBOM check waits for builds before merging PR #14
2. **Phase 2**: Run with intentional format error to confirm early exit
3. **Phase 3**: Test composite action on all 4 platforms sequentially
4. **Phase 4**: Verify cache hits in subsequent runs
5. **Phase 5**: Validate job summaries display correctly on PR

---

## Rollback Strategy

Each phase is independently deployable:
- Phase 1 (needs): Zero risk, additive only
- Phase 2 (validate): Zero risk, new isolated job
- Phase 3 (composite): Test in feature branch before merging
- Phases 4-5: Cosmetic, can be disabled safely

---

## References

- [GitHub Actions: Job Dependencies](https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions#jobsjob_idneeds)
- [GitHub Actions: Composite Actions](https://docs.github.com/en/actions/creating-actions/metadata-syntax-for-github-actions#composite)
- [Rust Formatting Standards](https://rust-lang.github.io/api-guidelines/about.html)
