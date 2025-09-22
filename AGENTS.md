# Repository Guidelines

## Project Structure & Module Organization
- Language: C FreeSWITCH module inspired by `mod_amqp`, targeting SurrealDB v3 for publishing and receiving commands.
- Source: C files at repo root (e.g., `mod_*/*.c` or `mod_*.c`, `mod_*.h`). Current code mirrors AMQP; SurrealDB integration will live in `mod_surrealdb_*.c`.
- Build artifacts: `Makefile`, `Makefile.in`, `Makefile.am`.
- Config examples: FreeSWITCH autoload configs typically in `/etc/freeswitch/autoload_configs/`.

## Build, Test, and Development Commands
- `make` — builds the module library.
- `make clean` — removes build artifacts.
- `make check` — runs tests if/when added.
Notes:
- Must be buildable without FreeSWITCH source tree. Prefer system headers/packages (e.g., `libfreeswitch-dev`, `pkg-config`) and SurrealDB C/C++ client or HTTP/gRPC bindings.
- Keep external deps optional; gate code with `#ifdef` where reasonable.

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

