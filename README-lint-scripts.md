# Unused Crate Dependencies Lint Scripts

This directory contains helper scripts for managing the `#![deny(unused_crate_dependencies)]` lint directive across all crates in the kona workspace.

## Scripts

### `validate_lint_directives.sh`
Validates that all crate root files (lib.rs and main.rs) have the `#![deny(unused_crate_dependencies)]` directive.

**Usage:**
```bash
./validate_lint_directives.sh
```

**Output:**
- ✅ for files that have the directive
- ❌ for files that are missing the directive
- Summary of total files and files with the directive

### `check_unused_deps.sh`
Identifies unused crate dependencies by building each crate and parsing the compiler output.

**Prerequisites:**
- Network connectivity to fetch dependencies
- Rust toolchain properly installed

**Usage:**
```bash
./check_unused_deps.sh
```

**Output:**
- Build status for each crate
- List of unused dependencies that need to be removed
- Suggested `cargo remove` commands

## Manual Unused Dependency Detection

If the scripts don't work due to network issues, you can also use:

```bash
# Install cargo-udeps
cargo install cargo-udeps

# Check for unused dependencies (requires nightly Rust)
cargo +nightly udeps --all
```

## Current Status

As of the last update:
- ✅ All 38 crate root files have the `#![deny(unused_crate_dependencies)]` directive
- ⏳ Unused dependency removal pending network connectivity for builds