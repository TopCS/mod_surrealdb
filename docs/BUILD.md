# Building the SurrealDB FFI and Linking from C

## Prereqs
- Rust toolchain (`rustup`, `cargo`) installed.
- Optional: `cbindgen` if you want to regenerate the header.

## Build
- Dynamic library: `cd surrealdb_ffi && cargo build --release`
  - Output: `target/release/libsurrealdb_ffi.so` (Linux)
- Header (optional): `cd surrealdb_ffi && cbindgen --config cbindgen.toml -o ../include/surrealdb_ffi.h`

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
  SurHandle* h = surreal_connect("ws://127.0.0.1:8000", "test", "test", "root", "root");
  if (!h) return 1;
  int rc = surreal_publish(h, "events", "{\"hello\":\"world\"}");
  surreal_close(h);
  return rc;
}
```

Note: The current implementation is a stub. Next step wires real SurrealDB v3 client calls and async runtime.

