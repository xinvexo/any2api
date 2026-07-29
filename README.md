# any2api

any2api is a personal, self-hosted AI API aggregation proxy. One process combines Codex, Claude, and Grok API keys and OAuth accounts behind OpenAI Responses, OpenAI Chat Completions, OpenAI Images, and Anthropic Messages endpoints.

The project is intentionally single-node. It does not provide registration, tenants, billing, subscriptions, API key sales, or distributed scheduling.

## Build And Run

The repository pins Rust 1.90.0. The management Web application is embedded in the binary, so a normal Rust build does not require Node.js.

```sh
cargo build --locked --release -p any2api
ANY2API_DATA_DIR=/var/lib/any2api ./target/release/any2api
```

The default listener is `127.0.0.1:3210`. Open `http://127.0.0.1:3210` after startup. On a new data directory, the process prints a one-time administrator setup token. Enter that token in the local Web UI, or set `ANY2API_ADMIN_PASSWORD` only for first-run password initialization.

The service health endpoint is `GET /api/health`.

## GitHub Releases

GitHub Releases currently provide one prebuilt target: Linux AMD64 (`x86_64-unknown-linux-gnu`), built on Ubuntu
22.04. The host must provide a compatible glibc runtime and system CA certificates. For example, to install `v0.1.0`:

```sh
VERSION=v0.1.0
ASSET="any2api-${VERSION}-linux-amd64.tar.gz"
curl -fLO "https://github.com/xinvexo/any2api/releases/download/${VERSION}/${ASSET}"
curl -fLO "https://github.com/xinvexo/any2api/releases/download/${VERSION}/${ASSET}.sha256"
sha256sum -c "${ASSET}.sha256"
tar -xzf "$ASSET"
ANY2API_DATA_DIR=/var/lib/any2api ./any2api
```

Run the `Release` workflow manually from GitHub Actions and enter the release version without the `v` prefix (for
example, `0.0.2`). The workflow uses that stable SemVer for the binary's reported version, the matching `v<version>`
tag, and the release asset names; it does not require the Cargo package version to match. It builds Linux AMD64 and
publishes the archive and checksum.

An authenticated administrator can open **Settings → About** to view the running version and repository, explicitly
check the latest official release, and install it on a supported Linux AMD64 GNU release build. The installer downloads
the fixed archive and checksum, verifies SHA-256, atomically replaces the executable, completes the existing bounded
graceful shutdown, and restarts the same executable with its original arguments. Development builds, other platforms,
and `ANY2API_WEB_DIR` mode can check releases but cannot install them in place. Docker deployments should update the
image instead; an in-container replacement only affects that container's writable layer.

Before upgrading, take an offline copy of the data directory and master key.

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

## Database Compatibility

Database migrations are immutable forward-only scripts. `migrations/0001_initial.sql` and every committed migration
must not be edited; schema changes append the next numbered SQL script and existing data directories upgrade through
the complete migration chain on startup.

Do not run two any2api processes against one data directory. The process holds an
exclusive instance lock and rejects a second owner.

## Remote Management

Remote management is disabled by default even if the listener is exposed. Enable `admin.remote_enabled` from a local management session before remote use.

Plain HTTP is supported, but it exposes the administrator password, session cookie, OAuth callback/code, and device user code to anyone able to observe the network. Prefer Caddy, Nginx, or another TLS-terminating reverse proxy. When forwarded client identity is required, set `ANY2API_TRUSTED_PROXY_CIDRS` only to the actual proxy networks. Headers from other peers are ignored; requests from a trusted proxy fail closed unless they contain exactly one valid `X-Forwarded-For` and `X-Forwarded-Proto` header.

For a same-host Nginx proxy, trust only the loopback address actually used by Nginx and send one normalized client address:

```nginx
location / {
    proxy_pass http://127.0.0.1:3210;
    proxy_http_version 1.1;
    proxy_buffering off;
    proxy_read_timeout 1200s;
    proxy_send_timeout 1200s;
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

The 1200-second proxy window is required for unary Responses Compact requests and also covers Codex
`remote_compaction_v2` streams whose first event can take much longer than a
normal generation. Configure every outer CDN/load balancer with an equal or longer timeout, or
bypass that layer for `/v1`; an HTML 502/504 near a fixed elapsed time usually means the outer proxy
expired before any2api or the model upstream did.

If Cloudflare is in front of Nginx, configure Nginx's real-IP module with Cloudflare's current official CIDRs and `CF-Connecting-IP` first, then pass the normalized `$remote_addr` as above. Keep the origin restricted to Cloudflare. any2api deliberately does not trust `CF-Connecting-IP` directly. RequestLog stores only the resulting canonical IP, never the raw forwarding chain.

## Public API

Clients authenticate with a Gateway API Key using `Authorization: Bearer <key>` or `x-api-key: <key>`.

- `GET /v1/models`
- `POST /v1/responses`
- `POST /v1/responses/compact`
- `POST /v1/chat/completions`
- `POST /v1/images/generations`
- `POST /v1/images/edits`
- `POST /v1/messages`
- `POST /v1/messages/count_tokens`

Images generation accepts JSON. Images edits accept JSON references or `multipart/form-data` uploads, and both endpoints support JSON or SSE responses when the configured OpenAI-compatible upstream supports them.

Provider API keys and OAuth accounts remain separate management records, but eligible credentials for the same model and protocol share one runtime routing pool.

## Development

Node.js 22.12+ and pnpm 10.17 are required only when changing the Web application.

```sh
cd web
pnpm install --frozen-lockfile
pnpm dev
```

Before submitting changes, run the relevant checks from [AGENTS.md](AGENTS.md). Architecture decisions live in [ARCHITECTURE.md](ARCHITECTURE.md) and [docs/adr](docs/adr).
