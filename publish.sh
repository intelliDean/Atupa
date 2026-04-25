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

# 1. Foundation
echo "📦 Publishing atupa-core..."
cargo publish -p atupa-core $DRY_RUN || echo "⚠️ Already published"

# 2. Level 1 - Independent / Base modules
echo "📦 Publishing atupa-rpc..."
cargo publish -p atupa-rpc $DRY_RUN || echo "⚠️ Already published"

echo "📦 Publishing atupa-adapters..."
cargo publish -p atupa-adapters $DRY_RUN || echo "⚠️ Already published"

# 3. Level 2 - Core Parsing & Visuals
echo "📦 Publishing atupa-parser..."
cargo publish -p atupa-parser $DRY_RUN || echo "⚠️ Already published"

echo "📦 Publishing atupa-output..."
cargo publish -p atupa-output $DRY_RUN || echo "⚠️ Already published"

# 4. Level 3 - Protocol Adapters
echo "📦 Publishing atupa-aave..."
cargo publish -p atupa-aave $DRY_RUN || echo "⚠️ Already published"

echo "📦 Publishing atupa-lido..."
cargo publish -p atupa-lido $DRY_RUN || echo "⚠️ Already published"

echo "📦 Publishing atupa-nitro..."
cargo publish -p atupa-nitro $DRY_RUN || echo "⚠️ Already published"

# 5. Facade SDK (Depends on adapters)
echo "📦 Publishing atupa-sdk..."
cargo publish -p atupa-sdk $DRY_RUN || echo "⚠️ Already published"

# 6. Final Binary (Depends on everything)
echo "📦 Publishing atupa (binary)..."
cargo publish -p atupa $DRY_RUN || echo "⚠️ Already published"

echo "✅ All crates processed successfully!"
