# rpow2

> A tribute to the original RPOW by Hal Finney.

A faithful modern recreation of Hal Finney's [Reusable Proofs of Work](https://nakamotoinstitute.org/finney/rpow/) (2004). The upstream project includes a TypeScript server and web client; this repository also contains a standalone Rust CLI client that talks to the existing server API directly.

Core features:

- magic-link authentication
- browser or CLI-driven session bootstrap
- hashcash-style mining
- Ed25519-signed tokens
- email-keyed transfers
- public ledger and personal activity views

## Repository layout

- `apps/server`: Node 22 + Fastify API server
- `apps/web`: React/Vite web client
- `apps/cli`: standalone Rust CLI client
- `packages/shared`: shared protocol types and PoW helpers for the TypeScript apps

## Rust CLI

The Rust CLI is designed to work against an existing RPOW-compatible server without requiring any server changes.

Supported commands:

```bash
rpow login
rpow cookie-login
rpow logout
rpow me
rpow mine
rpow send
rpow activity
rpow ledger
```

### Build

Development build:

```bash
cargo build -p rpow-cli
```

Release build:

```bash
cargo build --release -p rpow-cli
```

Binary locations:

- debug: `./target/debug/rpow`
- release: `./target/release/rpow`

Show help:

```bash
cargo run -p rpow-cli --bin rpow -- --help
```

### Configure server URL

By default the CLI uses:

```bash
http://localhost:8080
```

For a remote server, pass `--base-url` or set `RPOW_BASE_URL`:

```bash
./target/release/rpow --base-url https://api.rpow2.com ledger
```

```bash
export RPOW_BASE_URL=https://api.rpow2.com
./target/release/rpow ledger
```

### Login methods

The CLI stores the server session locally after a successful login.

Magic-link login:

```bash
./target/release/rpow --base-url https://api.rpow2.com login --email you@example.com
```

Flow:

1. CLI calls `POST /auth/request`
2. server emails a magic link
3. you paste the full magic link URL back into the terminal
4. CLI calls `/auth/verify`
5. CLI stores the returned `rpow_session` locally

Cookie login from an existing browser session:

```bash
./target/release/rpow --base-url https://api.rpow2.com cookie-login --cookie 'rpow_session=...'
```

You can also run it without `--cookie` and paste interactively:

```bash
./target/release/rpow --base-url https://api.rpow2.com cookie-login
```

Accepted cookie input formats:

- raw session value
- `rpow_session=...`
- `Cookie: rpow_session=...`
- `Set-Cookie: rpow_session=...; Path=/; HttpOnly`

### Common usage

Check current account:

```bash
./target/release/rpow --base-url https://api.rpow2.com me
```

View public ledger:

```bash
./target/release/rpow --base-url https://api.rpow2.com ledger
```

View activity:

```bash
./target/release/rpow --base-url https://api.rpow2.com activity
```

Mine continuously:

```bash
./target/release/rpow --base-url https://api.rpow2.com mine
```

Mine a single token:

```bash
./target/release/rpow --base-url https://api.rpow2.com mine --once
```

Send tokens:

```bash
./target/release/rpow --base-url https://api.rpow2.com send --to other@example.com --amount 3
```

Logout:

```bash
./target/release/rpow --base-url https://api.rpow2.com logout
```

### Notes

- `cargo run` uses the debug profile and is much slower for mining.
- For realistic mining performance, use the release binary.
- The current miner is single-threaded.
- If HTTPS requests fail during TLS handshake, check local proxy/VPN/DNS behavior before debugging the CLI.

## Local dev for server + web

Requires Node 22 and Docker.

```bash
docker run --rm -d --name rpow-pg -e POSTGRES_PASSWORD=p -p 55432:5432 postgres:16
npm install
npm run build --workspace @rpow/shared
npm test
```

To run the stack with low difficulty for hands-on testing:

```bash
# In one terminal
DATABASE_URL=postgres://postgres:p@localhost:55432/postgres \
RESEND_API_KEY=re_test EMAIL_FROM='rpow2 <no-reply@rpow2.com>' \
SESSION_SECRET=$(openssl rand -hex 32) \
MAGIC_LINK_BASE_URL=http://localhost:8080 WEB_ORIGIN=http://localhost:5173 \
DIFFICULTY_BITS=20 DIFFICULTY_FLOOR=8 \
RPOW_TEST_INBOX=true \
$(node -e 'import("./apps/server/dist/signing.js").then(({generateKeypair})=>{const k=generateKeypair(); console.log("RPOW_SIGNING_PRIVATE_KEY_HEX="+k.privateHex+" RPOW_SIGNING_PUBLIC_KEY_HEX="+k.publicHex);})') \
npm --workspace @rpow/server run dev

# In another terminal
npm --workspace @rpow/web run dev
```

## Deploy

- Server: Fly.io (`api.rpow2.com`)
- Web: Netlify (`rpow2.com`)
- DB: Neon Postgres (serverless)
- Email: Resend
- DNS: GoDaddy (registrar)

See `docs/RUNBOOK.md` for operator instructions.
