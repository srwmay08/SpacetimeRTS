#!/usr/bin/env bash
# publish.sh
# -----------------------------------------------------------------------------
# Architectural Note: This script assumes a local SpacetimeDB instance 
# is running on port 3000. It compiles the Rust backend and publishes it 
# as 'hybrid-backend'.
# -----------------------------------------------------------------------------

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Compiling and publishing SpacetimeDB backend module..."

# Publish the database module to the local SpacetimeDB server
spacetime publish -s local -p "$SCRIPT_DIR/spacetimedb" -y hybrid-backend

echo "Deployment complete. Backend is active."