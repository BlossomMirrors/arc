# Arc Forge HTTP API

All endpoints are public and require no authentication. Most are read-only; `POST /api/installs` is the only write endpoint.

Base URL: the root of the Forge instance, e.g. `https://forge.example.com`

## Localisation

The `/api/pwas` endpoint accepts a `?lang=` query parameter (ISO 639-1 code). When a translation exists for the requested language, the `name`, `summary`, and `description` fields are replaced with the translated values. All other fields remain in their original form. If no translation is available for a field, it falls back to English.

```
GET /api/pwas?lang=de
```

Supported languages currently: `en` (default), `de`. More languages can be added in the admin dashboard.

The discovery endpoints (`/api/new`, `/api/top`, `/api/trending`, `/api/charts`) return only app IDs — the client is responsible for localisation when resolving metadata.

## `GET /api/pwas`

Returns the full list of registered PWA applications.

**Response:** `application/json`

```json
[
  {
    "id": "clx...",
    "appid": "org.example.MyApp",
    "name": "My App",
    "summary": "A short description",
    "description": "Longer description of the app.",
    "icon_url": "https://example.com/icon.png",
    "screenshots": [
      "https://example.com/screenshot1.png"
    ],
    "homepage_url": "https://example.com",
    "content_rating": "All ages",
    "developer_name": "Example Dev",
    "verified": true,
    "url": "https://app.example.com",
    "color": "#3b82f6",
    "css": "",
    "js": "",
    "useragent": "",
    "widevine": false,
    "tray": false
  }
]
```

### Fields

| Field | Type | Description |
|---|---|---|
| `id` | string | Internal record ID (cuid) |
| `appid` | string | Unique reverse-domain app identifier |
| `name` | string | Display name |
| `summary` | string | Short one-line description |
| `description` | string | Full description (may contain HTML) |
| `icon_url` | string | URL to the app icon |
| `screenshots` | string[] | URLs of screenshot images |
| `homepage_url` | string | App website |
| `content_rating` | string | Age rating, e.g. `"All ages"` |
| `developer_name` | string | Publisher name |
| `verified` | boolean | Whether the developer is verified |
| `url` | string | URL used to launch the PWA |
| `color` | string | Hex accent colour for theming |
| `css` | string | Custom CSS injected into the PWA frame |
| `js` | string | Custom JS injected into the PWA frame |
| `useragent` | string | Custom User-Agent override (empty = browser default) |
| `widevine` | boolean | Requires Widevine DRM |
| `tray` | boolean | Should show a system-tray icon |

## `POST /api/installs`

Records an app install event. Used by the client to track installs for ranking purposes.

**Request:** `application/json`

```json
{ "appid": "org.example.MyApp" }
```

**Response:** `application/json`

```json
{ "ok": true }
```

Returns `400` if `appid` is missing or not a string.

---

## `GET /api/new`

Returns recently added app IDs, newest first.

**Sources (merged in order, deduplicated):**
1. Flathub `/collection/recently-added` — ordered by publish date
2. Our PWA apps — ordered by creation date
3. Custom Blossom repo apps

**Query params**

| Param | Default | Max | Description |
|---|---|---|---|
| `limit` | `20` | `100` | Number of results |

**Response:** `application/json` — ordered array of app ID strings.

```json
["nl.jknaapen.fladder", "org.example.MyApp", "com.figma.desktop"]
```

---

## `GET /api/top`

Returns app IDs ordered by popularity (all-time installs).

**Sources (merged in order, deduplicated):**
1. Flathub `/collection/popular` — ordered by installs in the last month
2. Our apps (PWA + custom repo) — ordered by install events recorded via `POST /api/installs`

**Query params**

| Param | Default | Max | Description |
|---|---|---|---|
| `limit` | `20` | `100` | Number of results |

**Response:** `application/json` — ordered array of app ID strings.

---

## `GET /api/trending`

Returns app IDs ordered by install growth in the last 30 days.

**Sources (merged in order, deduplicated):**
1. Flathub `/collection/trending` — ordered by growth trajectory over the last 2 weeks
2. Our apps (PWA + custom repo) — ordered by install events in the last 30 days

**Query params**

| Param | Default | Max | Description |
|---|---|---|---|
| `limit` | `20` | `100` | Number of results |

**Response:** `application/json` — ordered array of app ID strings.

---

## `GET /api/charts`

Returns app IDs with their chart rank. Uses the same ordering as `/api/top`.

**Query params**

| Param | Default | Max | Description |
|---|---|---|---|
| `limit` | `20` | `100` | Number of results |

**Response:** `application/json` — array of rank objects. Rank is 1-based and equals `index + 1`.

```json
[
  { "rank": 1, "id": "com.visualstudio.code" },
  { "rank": 2, "id": "org.example.MyApp" }
]
```

---

### App sources

The discovery endpoints (`/api/new`, `/api/top`, `/api/trending`, `/api/charts`) aggregate app IDs from three sources:

| Source | Description |
|---|---|
| Flathub | `https://flathub.org/api/v2/collection/*` — cached in memory for 1 hour |
| Custom Blossom repo | `https://repo.blossomos.org/flatpak/refs/heads/app/` — cached for 1 hour |
| PWA apps | Apps managed via the Forge dashboard |

The client is responsible for resolving app metadata (name, icon, description) from the appropriate source using the returned IDs.

---

## `GET /api/lutris-whitelist`

Returns the Lutris game whitelist as a plain-text newline-delimited list.

**Response:** `text/plain; charset=utf-8`

```
some-game-id
another-game-id
third-game-id
```

No caching (`Cache-Control: no-store`).

## `GET /api/frontpage`

Returns the store front-page layout as XML.

**Response:** `application/xml; charset=utf-8`

No caching (`Cache-Control: no-store`).

### XML structure

The document begins with an XML declaration followed by a flat sequence of section elements:

```xml
<?xml version="1.0" encoding="UTF-8" ?>
<h1>Welcome to the Store</h1>
<p>Discover great apps.</p>
<carousel breakpoint="5" flathub="false">
    <app id="org.example.MyApp" />
    <story banner="https://example.com/banner.jpg">
        <title lang="en">Featured</title>
        <title lang="de">Empfohlen</title>
        <body>
            Some story body text.
        </body>
    </story>
</carousel>
<top />
<custom>
    <title lang="en">Editor's Picks</title>
    <app id="org.example.AppOne" />
    <app id="org.example.AppTwo" />
</custom>
```

### Text and layout elements

| Element | Description |
|---|---|
| `<h1>text</h1>` | Large heading |
| `<h2>text</h2>` | Medium heading |
| `<h3>text</h3>` | Small heading |
| `<p>text</p>` | Body paragraph |
| `<ul><li>...</li></ul>` | Unordered list |
| `<br />` | Visual divider |

### App store sections

| Element | Attributes | Description |
|---|---|---|
| `<carousel>` | `breakpoint` (int), `flathub` (bool) | Featured app/story slideshow |
| `<top />` | | Highest-rated apps |
| `<new />` | | Recently added apps |
| `<trending />` | | Trending apps |
| `<categories />` | | Full category grid |
| `<category>slug</category>` | | Single category row |
| `<custom>` | | Curated list with title and app entries |
| `<charts>` | `cards` (bool) | App ranking charts; `cards="true"` for card layout |

### `<carousel>` children

| Element | Attributes | Description |
|---|---|---|
| `<app />` | `id` | App entry by appid |
| `<story>` | `banner` (URL) | Editorial story with `<title lang="...">` and `<body>` children |

### `<custom>` children

| Element | Attributes | Description |
|---|---|---|
| `<title>` | `lang` (BCP 47) | Localised section title |
| `<app />` | `id` | App entry by appid |
