# API

Base URL: `http://<host>:8678`. JSON API under `/api`, WebDAV under `/dav`, health at `/health`.
UI pages and the setup wizard are HTML, not part of the API.
No endpoint returns file bytes: content goes through WebDAV only.

Source of truth: `crates/fs-ez-web/src/router/routes.rs` and `crates/fs-ez-web/src/api/*`.

## Auth

| Client | Credential |
| --- | --- |
| Browser UI | `fs-ez_session` cookie, created by the `/login` form. |
| `/api/*` | Same session cookie **or** `Authorization: Bearer <api-token>`. |
| `/dav/*` | HTTP Basic: any username, **password = API token**. |

API tokens:

- Created from **Settings → API tokens**, `POST /api/tokens` (browser session only), or `fs-ez token add <name>`.
- SHA-256 hex stored; clear value shown exactly once.
- `expires_at` absent/`null` = permanent; past date = `400`.
- Revoked by id, by expiry, by `user reset`, or by deleting the user.
- A Bearer token cannot list, create or revoke tokens (`403`): token management requires a browser session or the CLI.

No password login over `/api`. The `/login` and `/setup` forms are the only browser auth flow.
Login failures are limited per IP: 5 per 15 min, then `429` with `Retry-After: 900`.

## Conventions

| Topic | Rule |
| --- | --- |
| Response | `{ "schema": 1, ... }` on success; `201` on creation. |
| Error | `{ "schema": 1, "error": "<message>", "code": "<code>" }`. `500` body is `internal server error`. |
| Body | JSON only, `Content-Type: application/json`, non-empty, max 2 MiB (`413`). |
| Paths | Resource IDs are positive integers. Non-UTF8 file paths are returned lossy. |
| Dates | RFC3339 UTC. |
| Pagination | `limit` 1..500 (default 100), `offset` ≥ 0; `next_offset: null` = end. |
| CSRF | Browser `POST/PUT/PATCH/DELETE` with `Sec-Fetch-Site: cross-site` or `same-site` → `403 csrf`. Header absent (curl, Bearer) allowed. |
| Cache | No global cache header; `/api/events` sets `no-cache`. |

Error codes: `bad_request` (`400`), `unauthorized` (`401`), `forbidden`/`csrf` (`403`), `not_found` (`404`), `conflict` (`409`), `payload_too_large` (`413`), `internal`/`db`/`config` (`500`).

## Setup

```bash
API=http://localhost:8678
TOKEN=... # clear value from Settings or: docker exec -it fs-ez fs-ez token add ci
AUTH=(-H "Authorization: Bearer $TOKEN")
JSON=(-H 'Content-Type: application/json')
```

First token: no API can mint it. Use the UI or `fs-ez token add` on the host/container.

## Health

`GET /health` — public, no DB access.

```json
{ "status": "ok", "version": "0.1.0" }
```

```bash
curl "$API/health"
```

## Session

| Method + path | Description |
| --- | --- |
| `GET /api/session` | Current principal. `{ "schema": 1, "username": "user" }`. |
| `POST /logout` | Browser logout; clears the session cookie. HTML redirect, not JSON. |

```bash
curl "${AUTH[@]}" "$API/api/session"
```

## Files

`FileDto`: `id, path, name, dir, size, mtime, kind, fingerprint?, tags[]`.
`tags[]`: `{ "name", "color", "explicit" }`.

`GET /api/files` query parameters:

| Parameter | Description |
| --- | --- |
| `q` | Name search, ≤ 200 characters. |
| `filter` | `untagged` \| `recent` \| `recent-files`. |
| `tag` | One tag name. |
| `query` | TMSU query (`Atlas and Review`, `not Archived`). Exclusive with `filter`, `tag`, `filter_json`. |
| `filter_json` | Serialized filter AST. |
| `sort` | `id\|name\|size\|mtime`, optional `:asc\|:desc`. |
| `offset`, `limit` | Pagination. |

Filter AST: `{ "op": "tag", "tag": "atlas" }`, `only_tag`, `tag_set`, `and`, `or`, `not`.
Example: `{"op":"and","items":[{"op":"tag","tag":"atlas"},{"op":"not","item":{"op":"tag","tag":"archived"}}]}`.

| Method + path | Description |
| --- | --- |
| `GET /api/files` | Page `{ items, next_offset }`. |
| `GET /api/files/{id}` | One file with tags. |
| `POST /api/files/{id}/tags` | `{ "tags": ["atlas"], "action": "add" \| "remove" \| "set" }`. Tags non-empty. Returns the file. |
| `DELETE /api/files/{id}/tags/{tag}` | Remove one tag. Returns the file. |

```bash
curl "${AUTH[@]}" "$API/api/files?q=invoice&sort=mtime:desc&limit=50"
curl "${AUTH[@]}" "$API/api/files?query=Atlas%20and%20not%20Archived"
curl "${AUTH[@]}" "$API/api/files/12"
curl "${AUTH[@]}" -X POST "$API/api/files/12/tags" "${JSON[@]}" \
  -d '{"tags":["atlas","review"],"action":"add"}'
curl "${AUTH[@]}" -X DELETE "$API/api/files/12/tags/atlas"
```

## Tags

Color tokens: `neutral, red, orange, amber, green, teal, cyan, blue, indigo, violet, pink, brown`.

| Method + path | Description |
| --- | --- |
| `GET /api/tags` | All tags with counts. `{ "tags": [...] }`. |
| `POST /api/tags` | `{ "name", "color"?, "group"? }`. Random color if absent. |
| `PATCH /api/tags/{name}` | `{ "name"?, "color"?, "color_custom"? }`; at least one field. |
| `DELETE /api/tags/{name}` | Detach and delete. `{ "ok": true }`. |

```bash
curl "${AUTH[@]}" "$API/api/tags"
curl "${AUTH[@]}" -X POST "$API/api/tags" "${JSON[@]}" \
  -d '{"name":"atlas","color":"blue"}'
curl "${AUTH[@]}" -X PATCH "$API/api/tags/atlas" "${JSON[@]}" \
  -d '{"name":"Atlas","color":"indigo"}'
curl "${AUTH[@]}" -X DELETE "$API/api/tags/Atlas"
```

## Views

A view is a saved filter plus sort, icon and color. `query` is TMSU syntax; `filter_json` is the AST above.
`ViewDto`: `id, name, filter, query, icon?, color?, position, sort, count, created_at, updated_at`.

| Method + path | Description |
| --- | --- |
| `GET /api/views` | `{ "views": [...] }` with counts. |
| `POST /api/views` | `{ "name", "query"? \| "filter_json"?, "icon"?, "color"?, "position"?, "sort"?, "group"? }`. Filter required. |
| `GET /api/views/{id}` | One view with count. |
| `PATCH /api/views/{id}` | Same fields; `icon`/`color` accept `null` to clear. |
| `DELETE /api/views/{id}` | `{ "ok": true }`. |
| `GET /api/views/{id}/files` | Run a view. `{ items, next_offset }`. |

```bash
curl "${AUTH[@]}" "$API/api/views"
curl "${AUTH[@]}" -X POST "$API/api/views" "${JSON[@]}" \
  -d '{"name":"Atlas review","query":"Atlas and Review","color":"amber","sort":"mtime:desc"}'
curl "${AUTH[@]}" "$API/api/views/3/files?limit=100"
curl "${AUTH[@]}" -X PATCH "$API/api/views/3" "${JSON[@]}" -d '{"icon":null,"color":"blue"}'
curl "${AUTH[@]}" -X DELETE "$API/api/views/3"
```

## Tag groups

Every tag belongs to exactly one group. `"action": "remove"` is refused; move the tag or delete it.
`GroupDto`: `id, name, color?, position, collapsed, tags[]`.

| Method + path | Description |
| --- | --- |
| `GET /api/groups` | `{ "groups": [...] }`. |
| `POST /api/groups` | `{ "name", "color"? }`. |
| `PATCH /api/groups/{id}` | `{ "name"?, "color"?, "collapsed"? }`; `color: null` clears. |
| `DELETE /api/groups/{id}` | `{ "ok": true }`. |
| `POST /api/groups/{id}/tags` | `{ "tag", "action": "assign" \| "add" }`. |

```bash
curl "${AUTH[@]}" -X POST "$API/api/groups" "${JSON[@]}" -d '{"name":"Projects","color":"teal"}'
curl "${AUTH[@]}" -X POST "$API/api/groups/2/tags" "${JSON[@]}" \
  -d '{"tag":"atlas","action":"assign"}'
```

## View groups

Every view belongs to exactly one group. Same rule: `"remove"` is refused.
`ViewGroupDto`: `id, name, color?, position, collapsed, views[]`.

| Method + path | Description |
| --- | --- |
| `GET /api/view-groups` | `{ "groups": [...] }`. |
| `POST /api/view-groups` | `{ "name", "color"? }`. |
| `PATCH /api/view-groups/{id}` | `{ "name"?, "color"?, "collapsed"?, "position"? }`. |
| `DELETE /api/view-groups/{id}` | `{ "ok": true }`. |
| `POST /api/view-groups/{id}/views` | `{ "view": 3, "action": "assign" \| "add" }`. |

```bash
curl "${AUTH[@]}" -X POST "$API/api/view-groups" "${JSON[@]}" -d '{"name":"Daily"}'
curl "${AUTH[@]}" -X POST "$API/api/view-groups/1/views" "${JSON[@]}" \
  -d '{"view":3,"action":"assign"}'
```

## Inbox

`POST /api/inbox/scan` — import, fingerprint and deduplicate the inbox. One scan at a time.

```json
{ "schema": 1, "imported": 3, "duplicates": 1, "skipped": 0 }
```

```bash
curl "${AUTH[@]}" -X POST "$API/api/inbox/scan"
```

## Tokens

Session only. Bearer gets `403`; token management requires a browser session or the CLI.
Examples below use the `fs-ez_session` cookie copied from the browser.

| Method + path | Description |
| --- | --- |
| `GET /api/tokens` | `{ "tokens": [...] }`; never the clear value nor the hash. |
| `POST /api/tokens` | `{ "name", "expires_at"? }` → `201` `{ id, name, created_at, expires_at?, last_used_at?, token }`. Clear token returned once. |
| `DELETE /api/tokens/{id}` | `{ "ok": true }`. |

```bash
SESSION=(-H "Cookie: fs-ez_session=$SESSION_COOKIE")

curl "${SESSION[@]}" "$API/api/tokens"
curl "${SESSION[@]}" -X POST "$API/api/tokens" "${JSON[@]}" \
  -d '{"name":"ci","expires_at":"2027-01-01T00:00:00Z"}'
curl "${SESSION[@]}" -X DELETE "$API/api/tokens/4"

# Headless equivalent:
docker exec -it fs-ez fs-ez token add ci
```

## Status & events

| Method + path | Description |
| --- | --- |
| `GET /api/status` | `{ version, root, db_path, db_size, files, untagged, recent, tags, views }`. |
| `GET /api/events` | SSE: `hello` with version, then `data_changed`, `inbox_changed`, `import_finished`, `resync`. |

```bash
curl "${AUTH[@]}" "$API/api/status"
curl -N "${AUTH[@]}" "$API/api/events"
```

## WebDAV

Mount `http://<host>:8678/dav` from any file manager. Basic auth: any username, password = API token.
Upload limit: `MAX_UPLOAD` bytes.

| Collection | Content |
| --- | --- |
| `tags/` | `tags/{tag}/{tag}…/All Files/` = files carrying every selected tag. `All Empty` = tags without files. |
| `views/` | `views/{group}/{view}/`; `PUT` imports with the positive tags required by the view filter. |
| `virtual folders/` | Virtual folder tree; `MKCOL`/`PUT` create folders. |
| `inbox/` | Inbox files; upload imports with tags from the target path. |
| `untagged/`, `recently tagged/`, `recent files/` | System views. |

The namespace is virtual: `PUT` imports or replaces an alias, or imports into a saved view with the positive tags its filter requires (a filter with no required tag refuses the upload). `MOVE` renames/moves a virtual folder or alias, `DELETE` removes a managed file (alias) or an upload temporary. Existing files can be opened, replaced, renamed and deleted through `untagged/`, `recently tagged/` and `recent files/`. Collections and inbox content are protected. `LOCK/UNLOCK`, `Range` and conditional requests are supported.

```bash
curl -u x:"$TOKEN" -X PROPFIND "$API/dav/" -H 'Depth: 1'
curl -u x:"$TOKEN" -T photo.jpg "$API/dav/inbox/photo.jpg"
curl -u x:"$TOKEN" -T photo.jpg "$API/dav/tags/atlas/All Files/atlas.jpg"
curl -u x:"$TOKEN" -X MOVE "$API/dav/virtual folders/todo/a.jpg" \
  -H "Destination: $API/dav/virtual%20folders/done/a.jpg"
```

## CLI parity

Same use cases as the API, no HTTP, no file content:

```bash
fs-ez files [query] --sort <field>[:asc|desc] --limit <n> --offset <n> --untagged --recent --print0
fs-ez tag --tags <t1,t2> <path>...
fs-ez untag --tags <t1,t2> <path>...
fs-ez tag --tags <t> --force <path>... # overwrite explicit tags
fs-ez tags
fs-ez rename <old> <new>
fs-ez merge --into <target> <source>...
fs-ez group list|add|rm|assign|move
fs-ez view list|add|rm|run
fs-ez view-group list|add|rm|assign|move
fs-ez inbox scan
fs-ez token add|list|revoke
fs-ez status|repair|init
```

Global flags: `--root`, `--db`, `--json`, `--quiet`, `--verbose`, `--no-color`, `--yes`.
