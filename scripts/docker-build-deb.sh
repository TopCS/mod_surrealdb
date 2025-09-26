#!/usr/bin/env bash
set -euo pipefail

# Build the Debian packaging image and run a container to produce the .deb.

IMAGE_NAME=${IMAGE_NAME:-mod-surrealdb-deb:bookworm}
BUILD_DIR=${BUILD_DIR:-build}
FS_PREFIX=${FS_PREFIX:-/usr/local/freeswitch}

echo "Building image $IMAGE_NAME (Debian Bookworm) ..."
docker build -t "$IMAGE_NAME" -f Dockerfile.debian .

echo "Running packaging inside container ..."
RUN_ARGS=(--rm -v "$(pwd)":/src -w /src)
if [[ -d "$FS_PREFIX" ]]; then
  echo "Mounting FreeSWITCH prefix: $FS_PREFIX"
  RUN_ARGS+=(
    -v "$FS_PREFIX":/usr/local/freeswitch:ro
    -e PKG_CONFIG_PATH=/usr/local/freeswitch/lib/pkgconfig:${PKG_CONFIG_PATH:-}
  )
fi

docker run "${RUN_ARGS[@]}" "$IMAGE_NAME" bash -lc "scripts/build-deb.sh $BUILD_DIR"

echo -e "\nDone. Artifacts in $BUILD_DIR/ :"
ls -1 "$BUILD_DIR"/*.deb || true

