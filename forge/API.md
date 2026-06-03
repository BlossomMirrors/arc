# Arc Forge HTTP API

All endpoints are public (no authentication required) and read-only.

Base URL: the root of the Forge instance, e.g. `https://forge.example.com`

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
