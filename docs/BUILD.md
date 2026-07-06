# Building the SurrealDB FFI and Linking from C

## Prereqs
- Rust toolchain (`rustup`, `cargo`) installed.
  - The real client uses the SurrealDB Rust SDK `3.2.0`, which requires Rust `1.94` or newer.
- Optional: `cbindgen` if you want to regenerate the header.

## Build
- Dynamic library: `cd surrealdb_ffi && cargo build --release`
  - Output: `target/release/libsurrealdb_ffi.so` (Linux)
- Header (optional): `cd surrealdb_ffi && cbindgen --config cbindgen.toml -o ../include/surrealdb_ffi.h`

### Features
- Default build uses a stub (no network deps).
- Enable real client wiring:
  - `cd surrealdb_ffi && cargo build --release --no-default-features --features real`
  - Requires Rust deps and network access to fetch crates.

## Link from C
- Include: `#include "include/surrealdb_ffi.h"`
- Link flags example:
  - `-L$(PWD)/surrealdb_ffi/target/release -lsurrealdb_ffi -ldl -lpthread`
- Runtime search path (example): `-Wl,-rpath,$(PWD)/surrealdb_ffi/target/release`

## Minimal usage
```c
#include "include/surrealdb_ffi.h"

int main() {
  surreal_init_runtime();
  // Password auth
  // URL format is host:port (no scheme)
  SurHandle* h = surreal_connect("127.0.0.1:8000", "test", "test", "root", "root");
  if (!h) return 1;
  int rc = surreal_publish(h, "events", "{\"hello\":\"world\"}");
  surreal_close(h);
  return rc;
}
```

Install or refresh `libsurrealdb_ffi.so` after rebuilding it:

```sh
sudo scripts/install-ffi.sh
```

FreeSWITCH loads the module and FFI library together. If `mod_surrealdb.so` was rebuilt but an older `libsurrealdb_ffi.so` is still installed, module loading can fail with an undefined symbol error.

## Receiving commands (callback API)
- Register a callback for a topic:
  - `int32_t surreal_subscribe(SurHandle*, const char* topic, surreal_command_cb cb, void* user_data);`
  - `int32_t surreal_unsubscribe(SurHandle*, const char* topic);`
- Stub testing helper (no network):
  - `int32_t surreal_debug_emit(SurHandle*, const char* topic, const char* json);`

Example:
```c
static void on_cmd(const char* topic, const char* json, void* ud) {
  (void)ud; printf("%s %s\n", topic, json);
}
...
surreal_subscribe(h, "commands", on_cmd, NULL);
surreal_debug_emit(h, "commands", "{\"do\":\"ping\"}");
surreal_unsubscribe(h, "commands");
```

## Token auth
```c
// URL format is host:port (no scheme)
SurHandle* h = surreal_connect_with_token("db.example.com:8000", "ns", "db", "<bearer-token>");
```
