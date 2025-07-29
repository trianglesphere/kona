#!/bin/bash
# Script to identify and remove unused crate dependencies
# Run this when network connectivity is restored

set -e

echo "=== Checking for unused crate dependencies ==="
echo "This script will:"
echo "1. Run cargo check to identify unused dependencies"
echo "2. Show which dependencies need to be removed"
echo "3. Provide commands to remove them"
echo ""

# Function to check a single crate
check_crate() {
    local crate_path="$1"
    local crate_name=$(basename "$crate_path")
    
    echo "Checking $crate_name..."
    cd "$crate_path"
    
    # Try to build and capture unused dependency errors
    if ! cargo check 2>&1 | tee /tmp/check_output.txt; then
        if grep -q "unused_crate_dependencies" /tmp/check_output.txt; then
            echo "❌ Found unused dependencies in $crate_name:"
            grep "unused_crate_dependencies" /tmp/check_output.txt | sed 's/^/  /'
            
            # Extract dependency names and suggest removal commands
            grep "unused_crate_dependencies" /tmp/check_output.txt | \
                sed -n 's/.*`\([^`]*\)`.*/\1/p' | \
                while read dep; do
                    echo "  To remove: cargo remove $dep"
                done
        else
            echo "❌ Build failed for other reasons in $crate_name"
        fi
    else
        echo "✅ $crate_name builds successfully"
    fi
    echo ""
}

# Get repo root
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

echo "Starting unused dependency check..."
echo "Repository root: $REPO_ROOT"
echo ""

# Check if we can build at all
echo "Testing basic build capability..."
if ! timeout 60 cargo check --quiet --bin client 2>/dev/null; then
    echo "❌ Cannot build due to network issues or other problems"
    echo "Please ensure:"
    echo "  1. Internet connectivity is available"
    echo "  2. Rust toolchain is properly installed"
    echo "  3. All dependencies can be fetched"
    exit 1
fi

echo "✅ Basic build works, proceeding with unused dependency check"
echo ""

# Find all Cargo.toml files and check each crate
find . -name "Cargo.toml" -not -path "./target/*" | while read toml_file; do
    crate_dir=$(dirname "$toml_file")
    check_crate "$crate_dir"
done

echo "=== Summary ==="
echo "Checked all crates for unused dependencies"
echo "Look for ❌ markers above to identify crates that need dependency removal"
echo ""
echo "To automatically remove unused dependencies, you can run:"
echo "  cargo install cargo-udeps"
echo "  cargo +nightly udeps --all"