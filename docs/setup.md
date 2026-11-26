# Setup & rescue

Environment and `fs-ez.toml` are read once at boot. No external service, no database server: SQLite only.

Priority: CLI flags > environment > `<root>/fs-ez.toml` > database settings > defaults. Database settings (`fs-ez config set`) apply live.

## Quick start

```yaml
# docker/compose.yml
services:
  fs-ez:
    image: ghcr.io/fs-ez/fs-ez:latest
    container_name: fs-ez
    environment:
      TZ: Etc/UTC
      PUID: "1000"
      PGID: "1000"
      MAX_UPLOAD: "17179869184" # 16 GiB
    volumes:
      - ./config:/config # SQLite database + backups
      - ./data:/data     # managed files + inbox
    ports:
      - 8678:8678 # Web UI, API and WebDAV
    restart: unless-stopped
```

Open <http://localhost:8678>, create the user on `/setup` (12+ characters).
Alternatively set `USERNAME` and `PASSWORD` on first boot — the user is created without the wizard.

## Environment

| Variable | Default | Description |
| --- | --- | --- |
| `HOST` | `0.0.0.0` | Bind address. |
| `PORT` | `8678` | Listen port. |
| `MAX_UPLOAD` | 16 GiB | Max bytes per WebDAV upload. Positive integer. |
| `USERNAME` | — | Username of the user created from `PASSWORD`. Required when `PASSWORD` is set. |
| `PASSWORD` | — | First-boot user password (12+ chars). Requires `USERNAME`. Ignored once a user exists. |
| `PUID` / `PGID` | `1000` | Owner of `/data`, `/config` and final process identity. |
| `TZ` | `Etc/UTC` | Local time for backups and displayed dates. |
| `RUST_LOG` | `info` | `trace`, `debug`, `info`, `warn`, `error`. |
| `FS_EZ_CONFIG` | `<root>/config` | State directory: database, backups and upload temporaries. `/config` in the image. |

The root is the process working directory (`/data` in the image). State lives in `FS_EZ_CONFIG` (`/config` in the image); `db`, `files` and `inbox` paths come from `fs-ez.toml`, never from the environment.

CLI flags: `fs-ez --root <path> --db <path>`, `fs-ez serve --host <host> --port <port>`.

## `fs-ez.toml`

Optional, at `<root>/fs-ez.toml`. Relative paths stay under root; `..` is refused.

```toml
host = "0.0.0.0"
port = 8678
max_upload = 17179869184
db = "state/fs-ez.db"
files = "files"
inbox = "inbox"
uid = 1000
gid = 1000
```

## Layout

| Path | Content |
| --- | --- |
| `/config/fs-ez.db` | Single source of truth. WAL, `foreign_keys=ON`, `busy_timeout=5s`. |
| `/config/backup/` | Automatic SQLite copies, `fs-ez-YYYYMMDD-HHMMSS.db`, mode `0600`. |
| `/config/tmp/` | Upload temporaries; purged at boot. |
| `/data/files/` | Managed file copies. |
| `/data/inbox/` | Drop zone. Scan imports, fingerprints, deduplicates. |

Backups run daily at `io.backup_hour` (`0..23`, local `TZ`), keep `io.backup_days` (`1..365`, default 7), and can be triggered from **Settings → Backup**.

## First user

- Web wizard: `GET/POST /setup` creates the single user, then logs in.
- Headless: `USERNAME=alice PASSWORD=change-me-to-12-chars-min` on a database without user. Both are required together. Existing user wins; the variables are never reapplied.
- Reset: `fs-ez user reset` (see below). Deletes every session and every API token.

## Reverse proxy

Terminate TLS in front (Caddy, Traefik, nginx…), forward to `8678/tcp`.
Session cookie is `Secure` unless `Origin`/`Referer` explicitly says `http://`; absent headers default to `Secure`.
HSTS, CSP, `X-Frame-Options` and `nosniff` are emitted by fs-ez.
Raise the proxy body limit to at least `MAX_UPLOAD`.

## Rescue CLI

Offline access, direct SQLite, no HTTP, no file content. Inside the container:

```bash
docker exec -it fs-ez fs-ez <command>
```

| Command | Purpose |
| --- | --- |
| `fs-ez status` | Version, root, DB path/size, file/tag/view counters. |
| `fs-ez repair` | Reconcile database rows with the files directory. |
| `fs-ez user reset [--username <name>]` | Set a new user password. Revokes every session and every API token. |
| `fs-ez token list` | List API tokens (never the clear value nor the hash). |
| `fs-ez token add <name> [--expires <rfc3339>]` | Create a token; clear value printed **once**. |
| `fs-ez token revoke <id>` | Revoke one token. |
| `fs-ez config list` / `get <key>` / `set <key> <value>` | Inspect or change database settings. |
| `fs-ez init` | Create the layout and missing settings. Idempotent. |
| `fs-ez files [query]` | Query files; `--untagged`, `--recent`, `--sort`, `--limit`, `--offset`. |

Global flags: `--root`, `--db`, `--json`, `--quiet`, `--verbose`, `--no-color`, `--yes`.

`user reset` reads the password from stdin. Interactive TTY prompts; non-interactive pipes work:

```bash
echo 'new-password-12+' | docker exec -i fs-ez fs-ez user reset
docker exec -it fs-ez fs-ez token list
docker exec -it fs-ez fs-ez config get io.backup_days
docker exec -it fs-ez fs-ez files --untagged --limit 20
```

`user reset` is the full lockout recovery: one user, new password, all old credentials dead.
Any automation still using a `Bearer` token must mint a new one afterwards.

## Restore a backup

Stop the container first. Replace the database, drop stale WAL files, start (paths relative to the compose file):

```bash
docker compose stop fs-ez
cp config/backup/fs-ez-<stamp>.db config/fs-ez.db
rm -f config/fs-ez.db-wal config/fs-ez.db-shm
docker compose start fs-ez
```

Backups contain every row but no file content — `/data/files` stays in place.
