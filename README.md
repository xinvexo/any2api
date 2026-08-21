# any2api

any2api is a personal, self-hosted AI API aggregation proxy. One process combines Codex, Claude, Grok, and Kimi API keys, plus OAuth accounts for supported providers, behind OpenAI Responses, OpenAI Chat Completions, OpenAI Images, and Anthropic Messages endpoints.

The project is intentionally single-node. It does not provide registration, tenants, billing, subscriptions, API key sales, or distributed scheduling.

## Build, Package, And Run

The repository pins Rust 1.97.1. A complete application build requires Node.js 24.19+, pnpm 11.22.0, and Rust. Install the workspace dependencies and build the production Web application together with the Rust executable from the repository root:

```sh
pnpm install --frozen-lockfile
pnpm build
# pnpm build prints the exact target/<triple>/release executable path.
ANY2API_DATA_DIR=/var/lib/any2api /path/to/any2api
/path/to/any2api --version
```

`pnpm build` produces the standalone production executable. `pnpm package` builds the same application through the shared build lifecycle, then writes a predictably named distribution archive and SHA-256 checksum to `dist/`.

Cargo commands remain Rust-only: `cargo check`, `cargo test`, and `cargo build --release` never start Node, pnpm, or Vite and never create distribution archives. They embed the explicit Rust-only notice page. The root `pnpm build` command passes its temporary Vite asset directory to Cargo when the executable must contain the current production frontend.

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
example, `0.0.2`). That workflow input is the release's sole product-version source: it determines the binary's reported
version, the matching `v<version>` tag, and the release asset names. The Cargo package version is Rust package metadata
and does not need to match. Before publishing, the workflow runs
`pnpm package --target x86_64-unknown-linux-gnu`. That public package lifecycle builds the current frontend and
backend, requires the built binary's exact `--version` output to match the input, and generates the Linux AMD64
archive and checksum. The workflow only publishes the resulting `dist/` artifacts.

An authenticated administrator can open **Settings → About** to view the running version and repository, explicitly
check the latest official release, and install it on a supported Linux AMD64 GNU release build. The installer downloads
the fixed archive and checksum, verifies SHA-256, executes a bounded `--version` smoke check, retains the current binary
as a sibling `.previous` file, atomically replaces the executable, completes the existing bounded graceful shutdown,
and restarts the same executable with its original arguments. The new process removes `.previous` only after storage,
configuration, listener, required workers, and shutdown signal handling are ready; an earlier observable startup failure
restores the old executable. This is binary recovery only: it does not reverse SQLite migrations or replace a systemd/
Docker supervisor. Development builds, other platforms, and `ANY2API_WEB_DIR` mode can check releases but cannot install
them in place. Docker deployments should update the image instead; an in-container replacement only affects that
container's writable layer.

Before upgrading, take an offline copy of the data directory.

## Startup Environment

| Variable | Default | Purpose |
|---|---|---|
| `ANY2API_BIND` | `127.0.0.1:3210` | HTTP listen address |
| `ANY2API_DATA_DIR` | `./data` | SQLite database, instance lock, and logs |
| `ANY2API_ADMIN_PASSWORD` | unset | Initialize the first administrator password; it does not rotate an existing password |
| `ANY2API_WEB_DIR` | unset | Explicit external Web assets for development; production normally uses embedded assets |
| `ANY2API_WORKER_THREADS` | Tokio default | Override the Tokio worker thread count with a positive integer |
| `RUST_LOG` | `info` | Console log filter; file-log level is managed in the Web settings |

## Data Protection

The data directory contains configuration, credentials, request history, and local logs. Provider API keys, proxy passwords, Gateway API keys, and OAuth Provider JSON are stored as plaintext in SQLite by product decision. Provider and OAuth secrets are not exposed by ordinary read/download/export endpoints; Gateway API keys remain visible to an authenticated administrator.

The data directory is the local protection boundary. On Unix any2api enforces `0700` on the data and log directories and `0600` on the SQLite database, WAL/SHM sidecars, instance lock, and application log files. On Windows, restrict the data directory to the service account with the host ACL.

There is no built-in backup or restore workflow. For an offline filesystem backup, stop any2api first and copy the data directory as one consistent set.

## Database Compatibility

Database migrations are immutable forward-only scripts. `migrations/0001_initial.sql` and every numbered migration
and checksum are frozen once added to the repository; schema changes append the next numbered SQL script. Existing
data directories and fresh databases both reach the current schema by running the complete migration chain on startup.

Do not run two any2api processes against one data directory. The process holds an
exclusive instance lock and rejects a second owner.

## Remote Management

Remote management is enabled by default, but it does not expose a new socket: the listener still defaults to `127.0.0.1:3210` and is controlled by `ANY2API_BIND`. On a new remote deployment, initialize the administrator password with `ANY2API_ADMIN_PASSWORD` because the one-time Setup API remains loopback-only.

Plain HTTP is supported, but it exposes the administrator password, session cookie, OAuth callback/code, and device user code to anyone able to observe the network. Prefer Caddy, Nginx, or another TLS-terminating reverse proxy. Configure **Settings → Basic → Trusted reverse proxy addresses** only with the IP addresses or CIDRs that can connect directly to any2api; leave it empty when no reverse proxy is used. The setting takes effect immediately. Headers from other peers are ignored. For a trusted proxy, a missing `X-Forwarded-For` falls back to the normalized TCP peer and a missing `X-Forwarded-Proto` is treated as insecure HTTP; present but malformed or ambiguous values fail closed. A request that traversed a trusted proxy never gains direct-loopback privileges from either fallback.

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
- `GET /v1/responses` (returns HTTP 426 `websocket_unavailable` as an HTTP fallback signal; it is not a WebSocket endpoint)
- `POST /v1/responses`
- `POST /v1/responses/compact`
- `POST /v1/alpha/search`
- `POST /v1/chat/completions`
- `POST /v1/images/generations`
- `POST /v1/images/edits`
- `POST /v1/messages`
- `POST /v1/messages/count_tokens`

Images generation accepts JSON. Images edits accept JSON references or `multipart/form-data` uploads, and both endpoints support JSON or SSE responses when the configured OpenAI-compatible upstream supports them.

Provider API keys and OAuth accounts remain separate management records, but eligible credentials for the same model and protocol share one runtime routing pool.

## Development

Node.js 24.19+ and pnpm 11.22.0 are required when changing the Web application or producing a complete package.

```sh
pnpm install --frozen-lockfile
pnpm dev
```

`pnpm dev` supervises the Vite development server and the automatically rebuilding Rust backend as one session. Use
`pnpm --dir web dev` only when intentionally running the frontend by itself.

Before submitting changes, run the relevant checks from [AGENTS.md](AGENTS.md). Current architecture lives in
[ARCHITECTURE.md](ARCHITECTURE.md); rationale and discarded directions live in the single register indexed by
[docs/adr/README.md](docs/adr/README.md).
