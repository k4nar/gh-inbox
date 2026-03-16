# PR Detail View Redesign

**Date:** 2026-03-16
**Status:** Approved for implementation

## Goal

Redesign the PR detail panel to be more compact and scannable. The user should be able to spot the current state of the PR and what has changed since their last visit at a glance, and read new comments in their thread context without scrolling past irrelevant older content.

## Layout

The panel is a vertical flex column, fixed right-hand side, ~500px wide.

### 1. Header (unchanged)

Back button · PR title (truncated) · GitHub external link (↗).

### 2. Status bar

One line. Left to right:

- **State pill** — `Open` (green) / `Draft` (grey) / `Merged` (purple) / `Closed` (red)
- **Author avatar** (18 × 18 px, circular) + **author login**
- `·` separator
- **+{additions}** (green) **−{deletions}** (red) **in {changed_files} files**
- **CI indicator** (right-aligned):
  - All passing → `● CI passing` in green
  - Any failing/pending → `● {n} failing` in red (or `● {n} running` in yellow)
  - On hover: tooltip listing every check run with its dot + name + conclusion

No description section. The PR description is accessible via the GitHub link.

### 3. Timeline body (scrollable)

A single vertically-scrollable list. Content is divided into two zones by named dividers.

#### "Since your last visit" divider (blue)

Shown only when there is at least one new item (commit or comment created after `last_viewed_at`).

New items appear immediately below the divider, in chronological order:

- **New commit row** — commit icon · short SHA · message · time ago. Blue-tinted background, blue border.
- **Fully-new thread** (all comments have `created_at > last_viewed_at`) — expanded in full. Each comment is shown with its avatar, author, timestamp, and body. Every comment row is a clickable `<a target="_blank">` linking to the comment's GitHub URL.
- **Thread with new replies** (some old, some new) — collapsed by default. Shows:
  - Thread header (file path or "Conversation") with a `{n} new` badge
  - First line of the first comment (dimmed, with avatar) as context
  - First line of the last new comment (blue-highlighted, with avatar) as preview
  - Clicking expands the full thread; each comment becomes a clickable link

#### "Earlier" divider (grey)

All activity predating `last_viewed_at`, in reverse chronological order:

- **Old commit row** — same layout as new commit but muted colour, no blue tint.
- **Old thread** — collapsed by default. Shows:
  - Thread header with comment count
  - First line of first comment + first line of last comment (both dimmed, with avatars)
  - Clicking expands the full thread

When there are no new items (first visit or no activity since last visit), the "Since your last visit" divider and "Earlier" divider are both omitted and all items are shown directly.

### Comment rendering (expanded state)

Each comment (whether in a fully-new thread or an expanded collapsed thread):

```
[avatar 18px] author  ·  time ago                              ↗
  comment body (markdown HTML, indented to align under name)
```

- The entire comment row is an `<a href="{comment.html_url}" target="_blank">` element.
- On hover: subtle background lightening + `↗` icon becomes visible.
- New comments have a 3px blue left border and a faint blue background tint.

## Data changes

### Backend

1. **`comments` table** — add `html_url TEXT` column (new migration).
2. **`src/github/pull_requests.rs`** — parse `html_url` from GitHub API responses for both `GithubIssueComment` and `GithubReviewComment` (the field is present on both GitHub REST endpoints).
3. **`src/db/queries.rs`** — add `html_url` to `CommentRow` itself (not to the `CommentResponse` wrapper). `CommentResponse` uses `#[serde(flatten)]` over `CommentRow`, so fields on `CommentRow` are automatically serialized to the top-level JSON object. Update `upsert_comment()` and `query_comments_for_pr()` accordingly.
4. **`last_viewed_at` — previous value must be returned.** The backend currently calls `update_last_viewed_at` (setting it to now) *before* querying comments. This means the `last_viewed_at` in the response equals the current timestamp — every comment appears "old". Fix: read the existing `last_viewed_at` from SQLite first, update it, then include the *old* value in the response as a separate field `previous_viewed_at: Option<String>`. The frontend uses `previous_viewed_at` (not `last_viewed_at`) to determine which comments/commits are new.
5. **`PrDetailResponse`** — add `previous_viewed_at: Option<String>` field alongside the existing `pull_request`, `comments`, `commits`, `check_runs`.

### Frontend

1. **`types.ts`** — add `html_url: string` to `Comment`; add `draft: boolean` and `merged_at: string | null` to `PullRequest`; add `previous_viewed_at: string | null` to `PrDetailResponse`.
2. **`CommentThread.svelte`** — rewrite to implement new expanded layout (avatar, clickable row). `body_html` is already present on `Comment` — no new markdown rendering needed.
3. **`PrDetail.svelte`** — rewrite to implement timeline layout (status bar, dividers, new/old zones, CI tooltip). Use `detail.previous_viewed_at` (not `detail.pull_request.last_viewed_at`) for all new/old comparisons.

## Behaviour details

- **CI tooltip** — rendered as an absolutely-positioned div on hover of the CI indicator in the status bar. Lists all check runs: dot + name + conclusion. No JS library needed.
- **Thread expansion** — collapsed threads toggle open on click (local Svelte state). Expanded threads show all comments as clickable `<a>` rows. The "Conversation" thread (`thread_id = "conversation"`) groups all top-level issue comments into a single unit — expanding it reveals all of them at once. This is acceptable given that long conversation threads are uncommon compared to inline review threads.
- **New vs old detection** — `item.created_at > previous_viewed_at` (or `commit.committed_at > previous_viewed_at`). `previous_viewed_at` is `null` on first visit, in which case nothing is marked new and dividers are omitted.
- **Avatar URLs** — GitHub user avatars follow the pattern `https://github.com/{login}.png?size=40`. No new API call needed; derive from the author login already stored.

## Out of scope

- Description panel (removed; use GitHub link)
- Inline diff/code context for review comments
- Thread resolve/unresolve actions
