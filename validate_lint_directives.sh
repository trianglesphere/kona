#!/bin/bash
# Script to validate that all crates have the unused_crate_dependencies lint directive

set -e

echo "=== Validating unused_crate_dependencies lint directive ==="
echo ""

REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

# Find all lib.rs and main.rs files
echo "Checking all crate root files for #![deny(unused_crate_dependencies)]..."
echo ""

missing_count=0
total_count=0

find . -name "lib.rs" -o -name "main.rs" | sort | while read file; do
    total_count=$((total_count + 1))
    
    if grep -q "#!\[deny(unused_crate_dependencies)\]" "$file"; then
        echo "✅ $file"
    else
        echo "❌ $file - MISSING lint directive"
        missing_count=$((missing_count + 1))
    fi
done

# Check the final counts
echo ""
echo "=== Summary ==="

total_files=$(find . -name "lib.rs" -o -name "main.rs" | wc -l)
files_with_directive=$(find . -name "lib.rs" -o -name "main.rs" | xargs grep -l "#!\[deny(unused_crate_dependencies)\]" | wc -l)

echo "Total crate root files: $total_files"
echo "Files with lint directive: $files_with_directive"

if [ "$total_files" -eq "$files_with_directive" ]; then
    echo "✅ SUCCESS: All crate root files have the lint directive"
    exit 0
else
    missing=$((total_files - files_with_directive))
    echo "❌ FAILURE: $missing files are missing the lint directive"
    exit 1
fi