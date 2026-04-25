#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# Atupa Workspace — Crates.io Publishing Script
#
# This script publishes all crates in the workspace in the required order.
# Usage: ./publish.sh [--dry-run]
# ─────────────────────────────────────────────────────────────────────────────

set -e

DRY_RUN=""
if [ "$1" == "--dry-run" ]; then
    DRY_RUN="--dry-run"
    echo "🔍 Performing DRY RUN..."
fi

# Robust publish function
publish_crate() {
    local crate=$1
    local delay=$2
    echo "📦 Publishing $crate..."
    
    # Run publish and capture output/exit status
    set +e
    output=$(cargo publish -p "$crate" $DRY_RUN 2>&1)
    status=$?
    set -e
    
    if [ $status -eq 0 ]; then
        echo "✅ Success: $crate"
    elif echo "$output" | grep -q "already exists"; then
        echo "⚠️  Already published: $crate"
    else
        echo "❌ FAILED: $crate"
        echo "$output"
        exit 1
    fi

    if [ -n "$delay" ] && [ "$DRY_RUN" == "" ]; then
        echo "⏳ Waiting ${delay}s for crates.io index..."
        sleep "$delay"
    fi
}

# 1. Foundation
publish_crate "atupa-core" 10

# 2. Level 1 - Independent / Base modules
publish_crate "atupa-rpc" 10
publish_crate "atupa-adapters" 10

# 3. Level 2 - Core Parsing & Visuals
publish_crate "atupa-parser" 10
publish_crate "atupa-output" 15

# 4. Level 3 - Protocol Adapters
publish_crate "atupa-aave" 10
publish_crate "atupa-lido" 10
publish_crate "atupa-nitro" 20

# 5. Facade SDK (Depends on adapters)
publish_crate "atupa-sdk" 30

# 6. Final Binary (Depends on everything)
publish_crate "atupa"

echo "✅ All crates processed successfully!"
