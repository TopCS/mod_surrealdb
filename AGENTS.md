# Repository Guidelines

## Project Structure & Module Organization
- Language: C FreeSWITCH module, with Rust FFI for SurrealDB v3 I/O.
- C sources: repo root (e.g., `mod_*.c`, `mod_*.h`). New code should use `mod_surrealdb_*.c`.
- Rust FFI: `surrealdb_ffi/` produces `libsurrealdb_ffi.so`; C header in `include/`.
- Build artifacts: `Makefile`, `Makefile.in`, `Makefile.am`.
- Config examples: FreeSWITCH autoload configs typically in `/etc/freeswitch/autoload_configs/`.

## Build, Test, and Development Commands
- `make` — builds the module library.
- `make clean` — removes build artifacts.
- `make check` — runs tests if/when added.
- Rust FFI: `cd surrealdb_ffi && cargo build --release` (stub by default).
- Enable real client: `cargo build --release --no-default-features --features real`.
Notes:
- Must be buildable without FreeSWITCH source tree. Prefer system headers/packages (e.g., `libfreeswitch-dev`, `pkg-config`).
- Keep external deps optional; gate code with `#ifdef` or feature flags where reasonable.

## Coding Style & Naming Conventions
- Indentation: tabs preferred, width 4 (matches existing hints).
- C style: K&R braces, 120-col soft limit, no trailing whitespace.
- Names: files `mod_surrealdb_*.c`; public symbols prefixed `mod_surrealdb_`.
- Logging: use FreeSWITCH logging macros consistently for levels and context.

## Testing Guidelines
- Unit tests: place under `tests/` named `test_*.c`; target via `make check`.
- Runtime checks: verify load/unload with `fs_cli` (`load mod_surrealdb`, `unload mod_surrealdb`) and confirm SurrealDB I/O.
- Aim for coverage of parsing, connection lifecycle, backpressure, and reconnection logic.

## Commit & Pull Request Guidelines
- Commits: concise imperative subject, e.g., `feat(io): add SurrealDB producer`; group related changes.
- PRs: include summary, rationale, linked issues, build output, and config snippets; add `fs_cli` logs and example SurrealDB payloads when applicable.

## Agent-Specific Instructions
- Ask, document, behave: propose changes, capture decisions in code comments/docs.
- Commit every change: keep incremental, reviewable commits. If the repo is not yet git-initialized, confirm before running `git init && git add -A && git commit`.
- Security & Config: do not hardcode credentials; support env vars (e.g., `SURREALDB_URL`, `SURREALDB_NS`, `SURREALDB_DB`, `SURREALDB_AUTH`).
