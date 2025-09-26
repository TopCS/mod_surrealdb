SurrealDB Commands Control Plane

Table schema (suggested)
- Table: `fs_commands`
- Fields:
  - `id` (record id; e.g., `fs_commands:<uuid>`)
  - `action` (string): one of `api`, `originate`, `hangup`, `bridge`, `playback`
  - `status` (string, optional): `new` | `processing` | `done` | `failed`
  - `created_at` (datetime, optional)
  - `processed_at` (datetime, set by module)
  - `result` (string, set by module): result or error text
  - Per-action fields:
    - `api`: `cmd` (string), `args` (string, optional)
    - `originate`: `args` (string) — full originate argument string
    - `hangup`: `uuid` (string), `cause` (string, optional)
    - `bridge`: `uuid_a` (string), `uuid_b` (string)
    - `playback`: `uuid` (string), `file` (string), `legs` (string, optional: aleg|bleg|both)

Examples (SQL)
- Create an API command:
  CREATE fs_commands CONTENT {
    action: 'api',
    cmd: 'status',
    status: 'new'
  };

- Originate:
  CREATE fs_commands CONTENT {
    action: 'originate',
    args: 'sofia/gateway/gw/1000 &park()',
    status: 'new'
  };

- Hangup:
  CREATE fs_commands CONTENT {
    action: 'hangup',
    uuid: 'b8d6e4e8-....',
    status: 'new'
  };

- Bridge:
  CREATE fs_commands CONTENT {
    action: 'bridge',
    uuid_a: 'uuid-a',
    uuid_b: 'uuid-b',
    status: 'new'
  };

- Playback:
  CREATE fs_commands CONTENT {
    action: 'playback',
    uuid: 'uuid-a',
    file: '/tmp/hello.wav',
    legs: 'both',
    status: 'new'
  };

Module behavior
- On subscribe, the module starts a live stream and receives change notifications.
- For each command, it runs the action and updates the row:
  - `status`: `done` or `failed`
  - `processed_at`: unix timestamp
  - `result`: API output or error

Notes
- Ensure mod_surrealdb is built with the `real` feature in `surrealdb_ffi` (SDK-backed).
- Configure `enable-commands=true` and `command-table=fs_commands` in `surrealdb.conf.xml`.
- Use idempotency and validation in your producer when needed.
surrealdb.publish
- Usage: `surrealdb.publish <table_or_topic> <json>`
- Inserts a JSON document into the specified table.
- Returns `+OK published` on success.

surrealdb.select
- Usage: `surrealdb.select <table> [limit]`
- Reads rows from a table and prints a JSON array.
- Default limit is 100; implementations may cap the maximum.

surrealdb.get
- Usage: `surrealdb.get <table> <id>`
- Reads a single record by id and prints a JSON object (or `null` if not found).

surrealdb.update
- Usage: `surrealdb.update <table> <id> <json>`
- Merges the provided JSON object into the record id in table. Returns `+OK updated` on success.
