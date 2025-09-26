Quickstart: mod_surrealdb

- Prereqs: install `libfreeswitch-dev`, `pkg-config`, `cmake`, and Rust toolchain (for FFI).
- Build FFI (stub by default): `cd surrealdb_ffi && cargo build --release`
- Build module (CMake):
  - `cmake -S . -B build -DCMAKE_BUILD_TYPE=Release`
  - `cmake --build build -j`  (artifacts under `build/`)
  - Install module into FreeSWITCH modules dir:
    - Preferred: `sudo cmake --install build` (installs to detected modules dir)
    - Or: `sudo scripts/install-module.sh` (copies `build/mod_surrealdb.so` to `/usr/local/freeswitch/mod/` or pkg-config path)
  - Optional package: `cmake --build build --target package`
- Or use Docker (Debian Bookworm):
  - `docker build -t mod-surrealdb:bookworm -f Dockerfile.debian .`
  - `docker run --rm -v $PWD:/src -w /src mod-surrealdb:bookworm scripts/build-deb.sh`
  - Legacy (Debian Stretch): `scripts/docker-build-deb-stretch.sh`
- Configure via XML: edit `conf/autoload_configs/surrealdb.conf.xml` with url, namespace, database, username/password or token, and `connect-on-load`.
  - URL format is `host:port` (no scheme), e.g. `127.0.0.1:8000`.
- Load in fs_cli: `load mod_surrealdb` and check logs.

Command test
- Publish a JSON payload from fs_cli: `fs_cli -x "surrealdb.publish my_table {\"msg\":\"hello\"}"`
- Expected: `+OK published` (in stub mode, it always reports success).

Notes
- If FFI lib is not built/found, the module loads in no-op mode and logs a warning.
- Build the real Rust client with: `cargo build --release --no-default-features --features real` inside `surrealdb_ffi/`.

Event sink (FreeSWITCH -> SurrealDB)
- Enable in `surrealdb.conf.xml`:
  - `enable-events=true`
  - `event-table=fs_events`
  - `event-filter=SWITCH_EVENT_ALL` (or a comma list like `SWITCH_EVENT_CHANNEL_CREATE,SWITCH_EVENT_CHANNEL_DESTROY,SWITCH_EVENT_DTMF`)
  - Optional backpressure: `send-queue-size` (default 1000) and `circuit-breaker-ms` (default 10000)
- After loading, generate some events (originate, dtmf, hangup) and query SurrealDB: `SELECT * FROM fs_events`

CDR sink
- Enable in `surrealdb.conf.xml`:
  - `enable-cdr=true`
  - `cdr-table=fs_cdr`
- The module writes one row per `CHANNEL_HANGUP_COMPLETE` with common fields such as `id`, `direction`, `caller_id_number`, `destination_number`, `hangup_cause`, `start_epoch`, `answer_epoch`, `end_epoch`, `duration`, `billsec`, and `sip_call_id`.
- Verify: `SELECT * FROM fs_cdr` after a completed call.
