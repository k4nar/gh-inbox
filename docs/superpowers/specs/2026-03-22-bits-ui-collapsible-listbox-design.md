# Bits UI: Collapsible & Listbox Adoption

**Date:** 2026-03-22
**Status:** Approved

## Context

Continuing Bits UI adoption. Three remaining hand-rolled interactive patterns in `PrDetail.svelte` and `PrList.svelte`:

1. PR description expand/collapse — `expandedDescription` boolean + `toggleDescription()`
2. Review body expand/collapse — `expandedReviews` Set + `toggleReview()`
3. PR list item selection — `<div role="button">` with manual `onclick`/`onkeydown`

---

## 1. PrDetail — Description Collapsible

### Current structure

```svelte
<!-- script -->
let expandedDescription = $state(true);
$effect(() => {
    // resets expandedDescription based on previousViewedAt when PR changes
});
function toggleDescription(): void {
    expandedDescription = !expandedDescription;
}

<!-- template -->
<button
    type="button"
    class="description-header"
    onclick={toggleDescription}
    aria-expanded={expandedDescription}
>
    <!-- chevron SVG with class:open={expandedDescription} -->
    Description
</button>
{#if expandedDescription}
    <div class="description-body">...</div>
{/if}
```

### Target structure

```svelte
<Collapsible.Root open={expandedDescription} onOpenChange={(v) => (expandedDescription = v)}>
    <Collapsible.Trigger class="description-header" type="button">
        <!-- chevron SVG with class:open={expandedDescription} — unchanged -->
        Description
    </Collapsible.Trigger>
    <Collapsible.Content>
        {#if expandedDescription}
            <div class="description-body">...</div>
        {/if}
    </Collapsible.Content>
</Collapsible.Root>
```

### What changes

- Import `Collapsible` from `"bits-ui"` (alongside existing `Tooltip` import)
- `expandedDescription = $state(true)` kept — controlled pattern, starts expanded
- The existing `$effect` that resets `expandedDescription` on PR navigation **must be kept** — it writes to `expandedDescription` directly and is orthogonal to `onOpenChange`
- `toggleDescription()` function deleted — Bits UI handles toggle internally
- `aria-expanded` removed from trigger button — Bits UI sets it automatically
- `class:open={expandedDescription}` on the chevron SVG kept — still reads from state
- `{#if expandedDescription}` guard kept inside `Collapsible.Content` — avoids `hiddenUntilFound` DOM issue (same pattern as `CommentThread`)
- `.description-header` passed as `class=` prop to `Collapsible.Trigger` → needs `:global(.description-header)` and `:global(.description-header:hover)` in `<style>` block

---

## 2. PrDetail — Reviews Collapsible

### Current structure

```svelte
<!-- script -->
let expandedReviews = $state<Set<number>>(new Set());
function toggleReview(id: number): void {
    const next = new Set(expandedReviews);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedReviews = next;
}

<!-- template, inside {#snippet reviewItem(review, showBadge)} -->
<div class="timeline-item review-item">
    {#if review.body}
        <button
            type="button"
            class="review-thread-header"
            onclick={() => toggleReview(review.id)}
            aria-expanded={expandedReviews.has(review.id)}
        >
            <!-- chevron SVG with class:open={expandedReviews.has(review.id)} -->
            ...
        </button>
        {#if review.body && expandedReviews.has(review.id)}
            <div class="review-body">...</div>
        {/if}
    {:else}
        <a class="review-thread-header review-thread-header--link" href="...">
            <!-- external link — no body to collapse -->
        </a>
    {/if}
</div>
```

### Target structure

Only the `{#if review.body}` branch is migrated. The `{:else}` `<a>` branch stays **unchanged** — wrapping an `<a>` in `Collapsible.Trigger` (which renders a `<button>`) would be invalid HTML.

```svelte
<div class="timeline-item review-item">
    {#if review.body}
        <Collapsible.Root
            open={expandedReviews.has(review.id)}
            onOpenChange={(v) => {
                const next = new Set(expandedReviews);
                if (v) next.add(review.id); else next.delete(review.id);
                expandedReviews = next;
            }}
        >
            <Collapsible.Trigger class="review-thread-header" type="button">
                <!-- chevron SVG with class:open={expandedReviews.has(review.id)} — unchanged -->
                ...
            </Collapsible.Trigger>
            <Collapsible.Content>
                {#if review.body && expandedReviews.has(review.id)}
                    <div class="review-body">...</div>
                {/if}
            </Collapsible.Content>
        </Collapsible.Root>
    {:else}
        <a class="review-thread-header review-thread-header--link" href="...">
            <!-- unchanged -->
        </a>
    {/if}
</div>
```

### What changes

- `Collapsible` already imported from step 1
- `toggleReview()` function deleted — logic inlined into `onOpenChange`
- `expandedReviews` Set kept — state tracks which reviews are open
- `aria-expanded` removed from trigger — Bits UI sets it automatically
- `class:open={expandedReviews.has(review.id)}` on chevron SVG kept
- `{#if review.body && expandedReviews.has(review.id)}` guard kept inside `Collapsible.Content`
- `.review-thread-header` passed as `class=` prop → needs `:global(.review-thread-header)` and `:global(.review-thread-header:hover)` in `<style>` block
- `.review-thread-header--link` on the `<a>` branch is unaffected — no `:global()` change needed for it

---

## 3. PrList — Listbox Item Selection

### Current structure

```svelte
<div class="pr-list" bind:this={listEl}>
    {#each notifications as notif (notif.id)}
        <div
            class="pr-item"
            class:read={!notif.unread}
            class:selected={notif.id === selectedId}
            data-notif-id={notif.id}
            onclick={() => handleSelect(notif)}
            role="button"
            tabindex="0"
            onkeydown={(e) => e.key === 'Enter' && handleSelect(notif)}
        >
            <!-- PR row content -->
        </div>
    {/each}
</div>
```

### Target structure

```svelte
<Listbox.Root
    type="single"
    value={selectedId ?? ""}
    onValueChange={(id) => {
        if (!id) return;
        const notif = notifications.find((n) => n.id === id);
        if (notif) handleSelect(notif);
    }}
>
    <Listbox.Content>
        <div class="pr-list" bind:this={listEl}>
            {#each notifications as notif (notif.id)}
                <Listbox.Item
                    value={notif.id}
                    class="pr-item"
                    class:read={!notif.unread}
                    data-notif-id={notif.id}
                >
                    <!-- PR row content unchanged -->
                </Listbox.Item>
            {/each}
        </div>
    </Listbox.Content>
</Listbox.Root>
```

**`bind:this={listEl}` note:** `listEl` is used by `IntersectionObserver` (calls `listEl.querySelectorAll("[data-notif-id]")`). Keep a plain `<div class="pr-list" bind:this={listEl}>` wrapper inside `Listbox.Content` to guarantee the bind works regardless of how `Listbox.Content` renders internally. The `.pr-list` class stays on the native div (no `:global()` needed — it's a regular scoped div).

**`onValueChange` type note:** Bits UI calls `onValueChange` with `string | undefined` (undefined when deselected). The `if (!id) return;` guard is required for type safety and correct behavior.

### What changes

- Import `Listbox` from `"bits-ui"`
- `<div class="pr-list" bind:this={listEl}>` stays as a native div inside `Listbox.Content`
- `<div class="pr-item" role="button" tabindex="0" onclick onkeydown>` → `Listbox.Item value={notif.id} class="pr-item" class:read={!notif.unread} data-notif-id={notif.id}`
- `onclick` and `onkeydown` removed from each item — `onValueChange` on `Listbox.Root` handles selection
- `class:selected={notif.id === selectedId}` removed — Bits UI sets `data-selected` attribute on the selected item
- Arrow keys (Up/Down) navigate + select (selection-follows-focus behavior — option c)
- `e.stopPropagation()` in `handleArchive`/`handleUnarchive` continues to prevent archive button clicks from bubbling to Listbox selection

### CSS

`.pr-item` is passed as a class prop to `Listbox.Item` (a Bits UI component), so it needs `:global()`:

```css
/* Before */
.pr-item { ... }
.pr-item:hover { ... }
.pr-item.selected { ... }
.pr-item.read .pr-title { ... }
.pr-item:hover :global(.action-btn) { ... }

/* After */
:global(.pr-item) { ... }
:global(.pr-item:hover) { ... }
:global(.pr-item[data-selected]) { ... }
:global(.pr-item.read) .pr-title { ... }   /* .pr-title stays scoped — it's on a plain div */
:global(.pr-item:hover) :global(.action-btn) { ... }
```

`class:read` remains on `Listbox.Item` (Bits UI forwards extra classes), so `.pr-item.read` compound selectors still work — just wrapped in `:global(.pr-item.read)` to reach the Bits UI-rendered element.

`.pr-item.selected` → `:global(.pr-item[data-selected])` (Bits UI uses `data-selected` attribute, not a class).

---

## Files Modified

| File | Change |
|---|---|
| `frontend/src/lib/PrDetail.svelte` | Add `Collapsible` import; replace description toggle; replace review toggle (button branch only) |
| `frontend/src/lib/PrList.svelte` | Add `Listbox` import; wrap pr-list in Listbox; replace pr-item divs with Listbox.Item |

---

## Verification

1. `npm run svelte-check` — no type errors
2. `npm run lint:fix` — no lint errors
3. `npm test` — all tests pass
4. Manual: Description starts expanded; clicking toggle collapses/expands; navigating to a new PR resets state correctly (the `$effect` still fires)
5. Manual: Each review body with content expands/collapses independently; review `<a>` links (no body) are unaffected
6. Manual: Arrow keys navigate + select PR items; Enter also selects
7. Manual: Archive button click does not select the item (stopPropagation still works)
8. Manual: Selected item styling matches previous behavior (via `data-selected`)
9. Manual: `.read` styling still applies to read items
