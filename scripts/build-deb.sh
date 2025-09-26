#!/usr/bin/env bash
set -euo pipefail

# Build and package mod_surrealdb as a .deb using CMake + CPack.
# Usage: scripts/build-deb.sh [build-dir]

BUILD_DIR=${1:-build}
JOBS=${JOBS:-2}

# Optional: build the Rust FFI first if present
if [[ -d surrealdb_ffi && -x "$(command -v cargo || true)" ]]; then
  echo "Building Rust FFI (release)..."
  (cd surrealdb_ffi && cargo build --release)
else
  echo "Skipping Rust FFI build (not found or cargo unavailable)"
fi

echo "Configuring CMake..."
cmake -S . -B "${BUILD_DIR}" -DCMAKE_BUILD_TYPE=Release -DFS_MOD_DIR=/usr/lib/freeswitch/mod ${CMAKE_ARGS:-}

echo "Building..."
cmake --build "${BUILD_DIR}" -j"${JOBS}"

echo "Installing (component) if available..."
cmake --install "${BUILD_DIR}" --component mod_surrealdb || true

echo "Packaging .deb ..."
cmake --build "${BUILD_DIR}" --target package -j"${JOBS}"

echo -e "\nArtifacts in ${BUILD_DIR}:"
ls -1 "${BUILD_DIR}"/*.deb || true
