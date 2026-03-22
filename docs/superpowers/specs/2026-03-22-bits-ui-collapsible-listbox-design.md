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
function toggleDescription(): void {
    expandedDescription = !expandedDescription;
}

<!-- template -->
<button
    type="button"
    class="description-toggle"
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
    <Collapsible.Trigger class="description-toggle" type="button">
        <!-- chevron SVG with class:open={expandedDescription} -->
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

- Import `Collapsible` from `"bits-ui"` (add to existing import if `Tooltip` is already imported)
- `expandedDescription = $state(true)` kept — controlled pattern, starts expanded
- `toggleDescription()` function deleted — Bits UI handles toggle internally
- `aria-expanded` removed from trigger — Bits UI sets it automatically
- `class:open={expandedDescription}` on the chevron SVG kept — still reads from state
- `{#if expandedDescription}` guard kept inside `Collapsible.Content` — avoids `hiddenUntilFound` DOM issue (same pattern as `CommentThread`)
- `.description-toggle` passed as `class=` prop to `Collapsible.Trigger` → needs `:global(.description-toggle)` in `<style>` block

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

<!-- template, inside {#each reviews as review} -->
<button
    type="button"
    class="review-header"
    onclick={() => toggleReview(review.id)}
    aria-expanded={expandedReviews.has(review.id)}
>
    <!-- chevron SVG with class:open={expandedReviews.has(review.id)} -->
    ...
</button>
{#if review.body && expandedReviews.has(review.id)}
    <div class="review-body">...</div>
{/if}
```

### Target structure

```svelte
<!-- script -->
let expandedReviews = $state<Set<number>>(new Set());
<!-- toggleReview deleted -->

<!-- template, inside {#each reviews as review} -->
<Collapsible.Root
    open={expandedReviews.has(review.id)}
    onOpenChange={(v) => {
        const next = new Set(expandedReviews);
        if (v) next.add(review.id); else next.delete(review.id);
        expandedReviews = next;
    }}
>
    <Collapsible.Trigger class="review-header" type="button">
        <!-- chevron SVG with class:open={expandedReviews.has(review.id)} -->
        ...
    </Collapsible.Trigger>
    <Collapsible.Content>
        {#if review.body && expandedReviews.has(review.id)}
            <div class="review-body">...</div>
        {/if}
    </Collapsible.Content>
</Collapsible.Root>
```

### What changes

- `Collapsible` already imported from step 1
- `toggleReview()` function deleted — logic inlined into `onOpenChange`
- `expandedReviews` Set kept — state tracks which reviews are open
- `aria-expanded` removed from trigger — Bits UI sets it automatically
- `class:open={expandedReviews.has(review.id)}` on chevron SVG kept
- `{#if review.body && expandedReviews.has(review.id)}` guard kept inside `Collapsible.Content`
- `.review-header` passed as `class=` prop → needs `:global(.review-header)` in `<style>` block

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
        const notif = notifications.find((n) => n.id === id);
        if (notif) handleSelect(notif);
    }}
>
    <Listbox.Content class="pr-list" bind:this={listEl}>
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
    </Listbox.Content>
</Listbox.Root>
```

**Note on `bind:this={listEl}`:** `listEl` is used for `IntersectionObserver` (infinite scroll). It must be bound to the scrollable `.pr-list` container. Bind it to `Listbox.Content` if that renders the container element, or keep a wrapper div if needed.

**Note on `class:selected`:** Bits UI Listbox sets `data-selected` on the active item rather than a CSS class. Replace `.pr-item.selected { ... }` CSS with `:global(.pr-item[data-selected]) { ... }` (or use `data-highlighted` for keyboard focus state). Remove `class:selected={notif.id === selectedId}` from the template — Bits UI manages this via `data-selected`.

### What changes

- Import `Listbox` from `"bits-ui"`
- `<div class="pr-list">` → `Listbox.Content class="pr-list"` wrapped in `Listbox.Root`
- `<div class="pr-item" role="button" tabindex="0" onclick onkeydown>` → `Listbox.Item value={notif.id} class="pr-item"`
- `onclick` and `onkeydown` removed from each item — `onValueChange` on `Listbox.Root` handles selection
- `class:selected` removed — replaced by `data-selected` attribute set by Bits UI
- Arrow keys (Up/Down) navigate + select (selection-follows-focus behavior)
- `e.stopPropagation()` in `handleArchive`/`handleUnarchive` continues to prevent archive button clicks from triggering Listbox selection
- CSS: `.pr-list`, `.pr-item` need `:global()` wrappers; `.pr-item.selected` → `:global(.pr-item[data-selected])`; `.pr-item:hover` → `:global(.pr-item:hover)`; `.pr-item.read` → `:global(.pr-item[data-value].read)` or keep `class:read` if Listbox.Item supports it

### CSS note on compound selectors

Compound selectors that combine `.pr-item` with a scoped parent (e.g. `.pr-list .pr-item`) must be restructured. Scoped parents (`.pr-list`, `.pr-list-header`) stay scoped; `.pr-item` references inside them need `:global()`:

```css
/* Before */
.pr-item.selected { ... }
.pr-item:hover { ... }
.pr-item.read .pr-title { ... }
.pr-item:hover :global(.action-btn) { ... }

/* After */
:global(.pr-item[data-selected]) { ... }
:global(.pr-item:hover) { ... }
:global(.pr-item.read) .pr-title-class { ... }
:global(.pr-item:hover) :global(.action-btn) { ... }
```

---

## Files Modified

| File | Change |
|---|---|
| `frontend/src/lib/PrDetail.svelte` | Add `Collapsible` import; replace description toggle + reviews toggle |
| `frontend/src/lib/PrList.svelte` | Add `Listbox` import; replace pr-list/pr-item with Listbox |

---

## Verification

1. `npm run svelte-check` — no type errors
2. `npm run lint:fix` — no lint errors
3. `npm test` — all tests pass
4. Manual: Description starts expanded; clicking toggle collapses/expands
5. Manual: Each review body expands/collapses independently
6. Manual: Arrow keys navigate + select PR items; Enter also selects
7. Manual: Archive button click does not select the item (stopPropagation still works)
8. Manual: Selected item styling matches previous behavior
