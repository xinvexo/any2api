# any2api

any2api is a personal, self-hosted AI API aggregation proxy. One process combines Codex, Claude, and Grok API keys and OAuth accounts behind OpenAI Responses, OpenAI Chat Completions, and Anthropic Messages endpoints.

The project is intentionally single-node. It does not provide registration, tenants, billing, subscriptions, API key sales, or distributed scheduling.

## Build And Run

The repository pins Rust 1.90.0. The management Web application is embedded in the binary, so a normal Rust build does not require Node.js.

```sh
cargo build --locked --release -p any2api
ANY2API_DATA_DIR=/var/lib/any2api ./target/release/any2api
```

The default listener is `127.0.0.1:3210`. Open `http://127.0.0.1:3210` after startup. On a new data directory, the process prints a one-time administrator setup token. Enter that token in the local Web UI, or set `ANY2API_ADMIN_PASSWORD` only for first-run password initialization.

The service health endpoint is `GET /api/health`.

## Startup Environment

| Variable | Default | Purpose |
|---|---|---|
| `ANY2API_BIND` | `127.0.0.1:3210` | HTTP listen address |
| `ANY2API_DATA_DIR` | `./data` | SQLite database, instance lock, default master key, and logs |
| `ANY2API_MASTER_KEY_FILE` | `<data-dir>/master-key.json` | External master-key file path |
| `ANY2API_ADMIN_PASSWORD` | unset | Initialize the first administrator password; it does not rotate an existing password |
| `ANY2API_TRUSTED_PROXY_CIDRS` | unset | Comma-separated trusted reverse-proxy networks allowed to supply forwarded client/protocol headers |
| `ANY2API_WEB_DIR` | unset | Explicit external Web assets for development; production normally uses embedded assets |
| `RUST_LOG` | process default | Console log filter; file-log level is managed in the Web settings |

## Data Protection

The data directory contains configuration, credentials, request history, and local logs. Provider API keys and proxy passwords are encrypted with the master key. OAuth Provider JSON is deliberately stored as plaintext in SQLite and is never exposed by a read/download/export endpoint.

Protect both of these files together:

- `<data-dir>/any2api.sqlite3`
- `<data-dir>/master-key.json`, or the file selected by `ANY2API_MASTER_KEY_FILE`

Losing the master key makes encrypted credentials unrecoverable. Copying only the master key without the database is also insufficient. On Unix the key file must not be readable by group or other users. On Windows, restrict the data directory and key file to the service account with the host ACL.

There is no built-in backup or restore workflow. For an offline filesystem backup, stop any2api first and copy the database, master key, and any SQLite sidecar files as one consistent set.

## Upgrades

Database migrations are forward-only and run before the server starts accepting requests.

1. Stop the existing process cleanly.
2. Take an offline copy of the data directory and external master key.
3. Replace the binary.
4. Start it against the same data directory and verify `/api/health` and the management UI.

Do not run two any2api processes against one data directory. The process holds an exclusive instance lock and rejects a second owner.

## Remote Management

Remote management is disabled by default even if the listener is exposed. Enable `admin.remote_enabled` from a local management session before remote use.

Plain HTTP is supported, but it exposes the administrator password, session cookie, OAuth callback/code, and device user code to anyone able to observe the network. Prefer Caddy, Nginx, or another TLS-terminating reverse proxy. When forwarded client identity is required, set `ANY2API_TRUSTED_PROXY_CIDRS` only to the actual proxy networks; untrusted forwarded headers are rejected.

## Public API

Clients authenticate with a Gateway API Key using `Authorization: Bearer <key>` or `x-api-key: <key>`.

- `GET /v1/models`
- `POST /v1/responses`
- `POST /v1/responses/compact`
- `POST /v1/chat/completions`
- `POST /v1/messages`
- `POST /v1/messages/count_tokens`

Provider API keys and OAuth accounts remain separate management records, but eligible credentials for the same model and protocol share one runtime routing pool.

## Development

Node.js 22.12+ and pnpm 10.17 are required only when changing the Web application.

```sh
cd web
pnpm install --frozen-lockfile
pnpm dev
```

Before submitting changes, run the relevant checks from [AGENTS.md](AGENTS.md). Architecture decisions live in [ARCHITECTURE.md](ARCHITECTURE.md) and [docs/adr](docs/adr).
