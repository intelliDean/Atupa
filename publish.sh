#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# Atupa Workspace — Crates.io Publishing Script
#
# This script publishes all crates in the workspace in topological dependency order.
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
    local delay=${2:-10}
    echo "📦 Publishing $crate..."
    
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

# Robust publish function with extra flags (e.g. --allow-dirty for embedded assets)
publish_crate_with_flags() {
    local crate=$1
    local delay=${2:-10}
    local flags=$3
    echo "📦 Publishing $crate with flags [$flags]..."
    
    set +e
    output=$(cargo publish -p "$crate" $DRY_RUN $flags 2>&1)
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

# 2. Base Networking & Registry Traits
publish_crate "atupa-rpc" 10
publish_crate "atupa-adapters" 10

# 3. Core Parsing & Visual Generators
publish_crate "atupa-parser" 10
publish_crate "atupa-output" 15

# 4. Specialized Protocol Tracers & Nitro VM
publish_crate "atupa-aave" 10
publish_crate "atupa-lido" 10
publish_crate "atupa-nitro" 15

# 5. Non-EVM VM Adapters
publish_crate "atupa-starknet" 10
publish_crate "atupa-solana" 10
publish_crate "atupa-stellar" 10

# 6. High-level SDK Facade
publish_crate "atupa-sdk" 20

# 7. Final CLI Binary (Embeds Studio bundle)
echo "📦 Preparing studio assets for atupa binary..."
if [ -d "studio/dist" ]; then
    rm -rf bin/atupa/dist
    cp -r studio/dist bin/atupa/dist
else
    echo "❌ Error: studio/dist not found. Run 'cd studio && npm run build' first."
    exit 1
fi

publish_crate_with_flags "atupa" 0 "--allow-dirty"

# Cleanup temporary build copy
rm -rf bin/atupa/dist

echo "🎉 All crates processed and published successfully!"
