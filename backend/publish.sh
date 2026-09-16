#!/usr/bin/env bash
# publish.sh
# -----------------------------------------------------------------------------
# Architectural Note: This script assumes a local SpacetimeDB instance 
# is running on port 3000. It compiles the Rust backend and publishes it 
# as 'hybrid-backend'.
# -----------------------------------------------------------------------------

set -e

echo "Compiling and publishing SpacetimeDB backend module..."

# Ensure we are logged into the local spacetime instance (if required)
spacetime server add local http://localhost:3000 --default || true

# Publish the database module
spacetime publish --project-path . hybrid-backend

echo "Deployment complete. Backend is active."