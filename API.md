# Arc Daemon HTTP API

The daemon exposes a read-only HTTP API on **`http://127.0.0.1:1312`**.  
It is bound to localhost only and intended for local tooling and web projects running on the same machine.

All endpoints support **CORS** (`Access-Control-Allow-Origin: *`).

---

## Language

Every endpoint accepts an optional `lang` query parameter. Pass a BCP-47 or POSIX locale tag to receive translated metadata. Omit it (or pass `en`) to get the English AppStream default.

| Value | Resolved candidates |
|---|---|
| *(absent)* | English (AppStream default) |
| `en` | English (AppStream default) |
| `de` | `de` → English fallback |
| `de_DE` | `de_DE` → `de` → English fallback |
| `zh-CN` | `zh_CN` → `zh` → English fallback |

---

## Endpoints

### `GET /api/v1/search`

Full-text search across app ID, name, and summary.

**Query parameters**

| Parameter | Required | Description |
|---|---|---|
| `q` | yes | Search query string |
| `lang` | no | Language tag (see above) |

**Response** `200 OK` — JSON array of [App objects](#app-object).

```
GET /api/v1/search?q=browser&lang=de
```

---

### `GET /api/v1/home`

Returns curated popular and recently-added app lists.

**Query parameters**

| Parameter | Default | Description |
|---|---|---|
| `popular` | `12` | Number of popular apps to return |
| `recent` | `24` | Number of recently-added apps to return |
| `lang` | — | Language tag |

**Response** `200 OK`

```json
{
  "popular": [ /* App objects */ ],
  "recent":  [ /* App objects */ ]
}
```

---

### `GET /api/v1/category/{name}`

Returns all apps in a given AppStream category.

**Path parameters**

| Parameter | Description |
|---|---|
| `name` | Category name (case-insensitive). See values below. |

**Common category names**

`AudioVideo`, `Development`, `Education`, `Graphics`, `Network`, `Office`, `Science`, `System`, `Utility`

**Query parameters**

| Parameter | Description |
|---|---|
| `lang` | Language tag |

**Response** `200 OK` — JSON array of [App objects](#app-object).

```
GET /api/v1/category/Graphics?lang=de
```

---

### `GET /api/v1/apps/{id}`

Returns metadata for a single app by its Flatpak application ID.

For installed apps that are absent from any AppStream catalog the daemon falls back to the app's own exported metainfo file.

**Path parameters**

| Parameter | Description |
|---|---|
| `id` | Flatpak application ID, e.g. `org.gnome.Gedit` |

**Query parameters**

| Parameter | Description |
|---|---|
| `lang` | Language tag |

**Response**

- `200 OK` — [App object](#app-object)
- `404 Not Found` — app not found in any catalog or metainfo file

```
GET /api/v1/apps/io.github.flattool.Warehouse?lang=de
```

---

### `GET /api/v1/apps/{id}/icon`

Fetches and proxies the app icon image. The response preserves the upstream `Content-Type` (typically `image/png` or `image/svg+xml`).

Only apps whose icon is served from a remote `https://` URL are supported. Apps with locally-cached icons return `404`.

**Path parameters**

| Parameter | Description |
|---|---|
| `id` | Flatpak application ID |

**Response**

- `200 OK` — image bytes with upstream `Content-Type`
- `404 Not Found` — no remote icon available
- `502 Bad Gateway` — upstream fetch failed

```
GET /api/v1/apps/org.gnome.Gedit/icon
```

---

### `GET /api/v1/image`

Proxies an arbitrary remote image URL. Intended for screenshot URLs returned in [App objects](#app-object). The response preserves the upstream `Content-Type`.

Only `http://` and `https://` URLs are accepted.

**Query parameters**

| Parameter | Required | Description |
|---|---|---|
| `url` | yes | URL-encoded image URL |

**Response**

- `200 OK` — image bytes with upstream `Content-Type`
- `400 Bad Request` — non-http(s) URL
- `502 Bad Gateway` — upstream fetch failed

```
GET /api/v1/image?url=https%3A%2F%2Fdl.flathub.org%2Fmedia%2F...%2Fscreenshot.png
```

---

## App object

All list and detail endpoints return objects with the following shape.

```json
{
  "id":             "io.github.flattool.Warehouse",
  "name":           "Warehouse",
  "summary":        "Manage all things Flatpak",
  "description":    "<p>HTML description...</p>",
  "icon_url":       "https://dl.flathub.org/media/.../icon.png",
  "remote":         "flathub",
  "screenshots":    ["https://dl.flathub.org/media/.../screenshot.png"],
  "license":        "GPL-3.0-only",
  "eula_url":       null,
  "homepage_url":   "https://github.com/flattool/warehouse",
  "content_rating": "All ages",
  "developer_name": "Heliguy"
}
```

| Field | Type | Notes |
|---|---|---|
| `id` | string | Canonical Flatpak application ID |
| `name` | string | Localized display name |
| `summary` | string | Short one-line description (localized) |
| `description` | string | Full HTML description (localized) |
| `icon_url` | string \| null | Remote `https://` URL or `local:` prefix (not directly fetchable); use `/apps/{id}/icon` to retrieve |
| `remote` | string \| null | Flatpak remote name (`"flathub"`, `"blossomos"`, …) |
| `screenshots` | string[] | Remote screenshot URLs; proxy via `/api/v1/image?url=` |
| `license` | string \| null | SPDX expression or `"Proprietary"` |
| `eula_url` | string \| null | EULA URL when the license is proprietary with a linked agreement |
| `homepage_url` | string \| null | Project homepage |
| `content_rating` | string | `"All ages"`, `"7+"`, `"12+"`, or `"18+"` |
| `developer_name` | string \| null | Developer or publisher name (localized) |
