# Inbox Filters Design

**Date:** 2026-04-13

## Overview

Add filtering to the inbox: quick single-select filters for repository and team in the left sidebar, and an advanced filter popover (repo, org, team, author, state) accessible from the existing Filter button in the PR list header. Filtering is server-side to work correctly with pagination.

---

## Filter State

A single `ActiveFilters` object lives in `App.svelte` and is passed down to both `Sidebar` and `PrList`.

```ts
interface ActiveFilters {
  repo?: string;    // "owner/repo"
  org?: string;     // "owner"
  team?: string;    // "owner/team-slug"
  author?: string;
  state?: string;   // "open" | "draft" | "merged" | "closed"
}
```

**Rules:**
- `repo` and `org` are mutually exclusive — setting one clears the other.
- All other filters stack as AND conditions.
- Sidebar and filter popover share the same `ActiveFilters` object — they are not independent states.
- A single `onFiltersChange(f: ActiveFilters) => void` callback in `App.svelte` handles all mutations.

Changing filters resets pagination to page 1.

---

## Backend

### New endpoint: `GET /api/inbox/options`

Returns distinct filter options currently in the database. All queries are capped at `LIMIT 100`.

```json
{
  "repos": ["owner/repo1", "owner/repo2"],
  "orgs": ["owner1", "owner2"],
  "teams": ["owner/team-slug"],
  "authors": ["alice", "bob"]
}
```

- `repos`: `SELECT DISTINCT repository FROM notifications LIMIT 100`
- `orgs`: derived from repos by splitting on `/` and taking the owner segment
- `teams`: from `user_teams` table (`SELECT slug FROM user_teams LIMIT 100`)
- `authors`: `SELECT DISTINCT author FROM pull_requests WHERE id IN (SELECT pr_id FROM notifications WHERE pr_id IS NOT NULL) LIMIT 100`

No `status` (inbox/archived) filter on the options endpoint — show all available values across both views.

### Extended `GET /api/inbox` query params

Add to `InboxQuery`:

```rust
pub repo: Option<String>,
pub org: Option<String>,
pub team: Option<String>,
pub author: Option<String>,
pub state: Option<String>,
```

These are passed into `query_enriched_paginated` via a `FilterParams` struct. SQL `WHERE` clause additions (all ANDed):

| Param | SQL condition |
|---|---|
| `repo` | `AND n.repository = ?` |
| `org` | `AND n.repository LIKE ?` (value: `"owner/%"`) |
| `team` | `AND EXISTS (SELECT 1 FROM json_each(pr.teams) WHERE value = ?)` |
| `author` | `AND pr.author = ?` |
| `state=open` | `AND pr.merged_at IS NULL AND pr.state != 'closed' AND pr.draft = 0` |
| `state=draft` | `AND pr.draft = 1` |
| `state=merged` | `AND pr.merged_at IS NOT NULL` |
| `state=closed` | `AND pr.state = 'closed' AND pr.merged_at IS NULL` |

The COUNT query used for pagination totals receives the same WHERE additions.

---

## Frontend

### Filter options loading

`App.svelte` fetches `GET /api/inbox/options` on mount and re-fetches on `notifications:new` SSE events. The result is stored as `filterOptions` state and passed to `Sidebar`.

### Sidebar

The two placeholder sections ("Repositories" and "Codeowner Teams") are populated from `filterOptions`.

Props added to `Sidebar`:
- `options: { repos: string[], teams: string[] }`
- `activeFilters: ActiveFilters`
- `onFiltersChange: (f: ActiveFilters) => void`

Behaviour:
- Each repo/team is a button with the same `.sidebar-item` style as Inbox/Archived.
- Active item shows the existing active style (left accent bar + bold).
- Clicking an active item deselects it (toggle off).
- Clicking a different item replaces the current selection.
- Repo items display just the `repo` part of `owner/repo` as the label; full string in `title` attribute for disambiguation.
- Team items display just the team slug part of `owner/team-slug`.
- If `options.repos` is empty, the "Repositories" section is hidden entirely. Same for teams.

### Filter Popover

The existing `<button class="filter-btn">` in `PrList.svelte` becomes a `Popover.Root` (Bits UI). The popover opens below the button.

Props added to `PrList`:
- `filterOptions: FilterOptions`
- `activeFilters: ActiveFilters`
- `onFiltersChange: (f: ActiveFilters) => void`

The popover contains 5 filter rows, each using a Bits UI `Select`:

| Field | Options source |
|---|---|
| Repository | `filterOptions.repos` |
| Org | `filterOptions.orgs` |
| Team | `filterOptions.teams` |
| Author | `filterOptions.authors` |
| State | hardcoded: open, draft, merged, closed |

- Each field shows a "—" placeholder when unset.
- Selecting a value updates `activeFilters` immediately (no Apply button).
- Selecting "—" clears that filter.
- A "Clear all" link at the bottom resets `activeFilters` to `{}`.
- The Filter button shows an accent-colored dot when any filter is active.

`PrList` appends active filters to the fetch URL:
```
/api/inbox?status=inbox&page=1&per_page=20&repo=owner%2Frepo&author=alice
```

---

## Testing

### Backend
- Unit tests for `query_enriched_paginated` with each filter param (repo, org, team, author, each state value) — assert correct rows returned and correct total count
- Unit test for `get_inbox_options` — assert LIMIT 100, correct distinct values, correct org derivation
- Integration test for `GET /api/inbox/options` — assert correct JSON shape
- Integration tests for `GET /api/inbox` with each filter param — assert filtered response shape and count

### Frontend
- `Sidebar.test.ts`: repo/team items render from options; active item gets active style; clicking toggles filter; empty section hidden when no options
- `PrList.test.ts`: filter button shows active indicator when filters set; popover contains correct fields; fetch URL includes active filter params; "Clear all" resets filters
- `App.test.ts`: `onFiltersChange` updates `activeFilters`; filter options fetched on mount; options re-fetched on SSE `notifications:new`
