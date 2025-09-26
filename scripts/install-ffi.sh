#!/usr/bin/env bash
set -euo pipefail

# Install libsurrealdb_ffi.so next to the FreeSWITCH module so the loader can resolve symbols.
# Usage: sudo scripts/install-ffi.sh [path-to-libsurrealdb_ffi.so]

SO_SRC=${1:-surrealdb_ffi/target/release/libsurrealdb_ffi.so}
if [[ ! -f "$SO_SRC" ]]; then
  echo "ERROR: FFI library not found: $SO_SRC" >&2
  echo "Build it first: (cd surrealdb_ffi && cargo build --release --no-default-features --features real)" >&2
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

echo "Copying $SO_SRC -> $FS_MOD_DIR/"
install -m 0755 -D "$SO_SRC" "$FS_MOD_DIR/libsurrealdb_ffi.so"
echo "Done. Ensure mod_surrealdb is reloaded (fs_cli: 'reload mod_surrealdb')."

