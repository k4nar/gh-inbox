# Theming Design

## Overview

Introduce a light and dark theme to the frontend, matching GitHub's Primer design system. Users select a theme via a dropdown in the topbar. The preference is stored in a new backend `user_preferences` table and injected into the HTML at serve time to prevent flash.

## CSS Layer

All color tokens in `app.css` are reorganized into four blocks. Non-color layout tokens (`--sidebar-w`, `--pr-list-min-w`, `--pr-detail-min-w`) remain in `:root` unconditionally, before the theme blocks.

1. **`:root`** — layout tokens (unchanged) + light theme color values (default; covers "system" on light OS and old browsers)
2. **`@media (prefers-color-scheme: dark) { :root:not([data-theme]) }`** — dark values when system is dark and no explicit theme is set
3. **`html[data-theme="light"]`** — explicit light override
4. **`html[data-theme="dark"]`** — explicit dark override

Blocks 3 and 4 use `html[data-theme]` (element + attribute) rather than `[data-theme]` alone to ensure higher specificity than the media-query block in block 2. This makes the override unambiguous regardless of source order.

### GitHub Primer Light palette

| Token | Value |
|---|---|
| `--canvas-default` | `#ffffff` |
| `--canvas-subtle` | `#f6f8fa` |
| `--canvas-inset` | `#f0f6fc` |
| `--border-default` | `#d0d7de` |
| `--border-muted` | `#d8dee4` |
| `--fg-default` | `#1f2328` |
| `--fg-muted` | `#636c76` |
| `--fg-subtle` | `#6e7781` |
| `--fg-on-emphasis` | `#ffffff` |
| `--accent-fg` | `#0969da` |
| `--accent-subtle` | `#ddf4ff` |
| `--accent-emphasis` | `#0969da` |
| `--success-fg` | `#1a7f37` |
| `--danger-fg` | `#d1242f` |
| `--attention-fg` | `#9a6700` |
| `--done-fg` | `#8250df` |

Note: `--accent-fg` and `--accent-emphasis` share the same value in the GitHub Primer light theme intentionally.

### GitHub Primer Dark palette (existing, verified correct)

| Token | Value |
|---|---|
| `--canvas-default` | `#0d1117` |
| `--canvas-subtle` | `#161b22` |
| `--canvas-inset` | `#010409` |
| `--border-default` | `#30363d` |
| `--border-muted` | `#21262d` |
| `--fg-default` | `#e6edf3` |
| `--fg-muted` | `#7d8590` |
| `--fg-subtle` | `#6e7681` |
| `--fg-on-emphasis` | `#ffffff` |
| `--accent-fg` | `#2f81f7` |
| `--accent-subtle` | `#121d2f` |
| `--accent-emphasis` | `#1f6feb` |
| `--success-fg` | `#3fb950` |
| `--danger-fg` | `#f85149` |
| `--attention-fg` | `#d29922` |
| `--done-fg` | `#a371f7` |

## Backend

### Migration

New migration `user_preferences` table:

```sql
CREATE TABLE IF NOT EXISTS user_preferences (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
```

Designed to be extended with future preference keys beyond `theme`.

### API endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/preferences` | Returns a JSON object containing all known preference keys with their current or default values, e.g. `{"theme": "system"}`. Only known keys are included; the response does not surface unknown/future keys stored in the table. |
| `PATCH` | `/api/preferences` | Body is a partial preferences object, e.g. `{"theme": "light"}`. For the `theme` key, valid values are `"system"`, `"light"`, and `"dark"`; any other value returns 400. Unknown keys are stored as-is without validation. Malformed JSON returns 400. Returns 204 on success. |

Known keys and defaults:

| Key | Default | Valid values |
|---|---|---|
| `theme` | `"system"` | `"system"`, `"light"`, `"dark"` |

Both endpoints require the `X-Session-Token` header (standard for all `/api/*` routes). The session token is read synchronously from the injected `<meta name="session-token">` tag in the DOM and is available before any fetch is made, so no special treatment is needed.

### SPA handler injection

When serving `index.html` (release mode only), the handler reads the `theme` preference from SQLite and performs a single injection pass that handles both the existing CSRF session-token meta tag and the new `data-theme` attribute:

- `"system"` → only inject the CSRF meta tag (no `data-theme` attribute; CSS media query handles it)
- `"light"` or `"dark"` → inject `data-theme="light"` (or `"dark"`) onto the `<html>` tag, and inject the CSRF meta tag into `<head>`

The two injections compose in a single pass: first replace `<html` with `<html data-theme="..."` (when not system), then replace `</head>` with the CSRF meta tag + `</head>`. This keeps the handler's HTML mutation in one place. This assumes `index.html` is Vite build output and contains exactly one `<html` opening tag and one `</head>` closing tag — a safe assumption for controlled build output.

To perform the async SQLite read, `serve_embedded` must receive a `SqlitePool` reference and become async. The theme is fetched inside the handler before building the HTML response. If the SQLite read fails, the handler falls back to "system" (omits `data-theme` injection) and logs the error. The frontend startup `GET /api/preferences` will recover the correct state after load.

Dev mode (Vite dev server) serves `index.html` directly — no injection runs.

## Frontend

### Theme values

| Value | Behaviour |
|---|---|
| `"system"` | No `data-theme` attr on `<html>`; CSS media query resolves light or dark |
| `"light"` | `document.documentElement.dataset.theme = "light"` |
| `"dark"` | `document.documentElement.dataset.theme = "dark"` |

### Topbar dropdown

A `<select>` element added to the right side of the topbar with three options: **System**, **Light**, **Dark**.

On change:
1. If `"system"`: `delete document.documentElement.dataset.theme`
2. Otherwise: `document.documentElement.dataset.theme = value`
3. `PATCH /api/preferences` with `{ "theme": value }`

### Startup

On app mount, `GET /api/preferences` is called to populate the dropdown's selected value and to apply the theme to the DOM:

- In release mode: the backend has already injected `data-theme` (or left it absent for "system"), so the DOM is correct. Only the dropdown state needs updating.
- In dev mode: the backend injection does not run. After fetching preferences, if the saved value is `"light"` or `"dark"`, apply it to `document.documentElement.dataset.theme`. If `"system"`, remove the attribute (the CSS media query already handles it).

This ensures that an explicit theme preference is honoured in dev mode too, rather than silently reverting to the system default on every page load. A brief flash of the system theme is possible in dev mode before `GET /api/preferences` resolves — this is acceptable as a dev-only limitation.

## Out of scope

- Per-user preferences (single-user local app)
- Custom/user-defined themes
- Theme transition animations
