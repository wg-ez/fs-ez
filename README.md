<p align="center">
  <img src="static/img/logo.svg" width="96" alt="fs-ez logo" />
</p>

<h1 align="center">fs-ez</h1>

<p align="center">
  A self-hosted file library packaged into a single static Rust binary, backed by SQLite.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green?style=flat-square" alt="MIT" /></a>
  <img src="https://img.shields.io/badge/rust-1.99+-orange?style=flat-square&logo=rust&logoColor=white" alt="Rust" />
  <a href="https://ghcr.io/fs-ez/fs-ez"><img src="https://img.shields.io/badge/docker-ghcr.io-blue?style=flat-square&logo=docker&logoColor=white" alt="Docker" /></a>
  <img src="https://img.shields.io/badge/SQLite-003B57?style=flat-square&logo=sqlite&logoColor=white" alt="SQLite" />
</p>

<p align="center">
  <img src="docs/screenshot.png" width="100%" alt="fs-ez file library" />
</p>

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
    volumes:
      - ./config:/config # SQLite database + backups
      - ./data:/data     # managed files + inbox
    ports:
      - 8678:8678 # Web UI, API and WebDAV
    restart: unless-stopped
```

Open <http://localhost:8678> and create the user on `/setup`.

## Docs

- [Setup & rescue](docs/setup.md) — volumes, environment, backups, CLI recovery
- [API](docs/api.md) — Bearer tokens, REST routes, WebDAV
- [Development](docs/development.md) — gates, dev server, audit, layout

## License

MIT — see [LICENSE](LICENSE).
