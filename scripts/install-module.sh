#!/usr/bin/env bash
set -euo pipefail

# Install the built mod_surrealdb.so into the FreeSWITCH modules directory.
# Usage: scripts/install-module.sh [path-to-mod_surrealdb.so]

SO_SRC=${1:-build/mod_surrealdb.so}
if [[ ! -f "$SO_SRC" ]]; then
  echo "ERROR: Module not found: $SO_SRC" >&2
  echo "Build first: cmake -S . -B build && cmake --build build" >&2
  exit 2
fi

FS_MOD_DIR=${FS_MOD_DIR:-}
if [[ -z "${FS_MOD_DIR}" ]]; then
  FS_MOD_DIR=$(pkg-config --variable=modulesdir freeswitch 2>/dev/null || true)
fi
if [[ -z "${FS_MOD_DIR}" ]]; then
  # Fallbacks
  if [[ -d /usr/lib/freeswitch/mod ]]; then
    FS_MOD_DIR=/usr/lib/freeswitch/mod
  else
    FS_MOD_DIR=/usr/local/freeswitch/mod
  fi
fi

echo "Installing $SO_SRC -> $FS_MOD_DIR/"
mkdir -p "$FS_MOD_DIR"
cp -f "$SO_SRC" "$FS_MOD_DIR/"
echo "Done. Restart or reload the module in fs_cli (e.g., 'reload mod_surrealdb')."

