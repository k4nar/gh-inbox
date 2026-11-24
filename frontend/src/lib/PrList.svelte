<script lang="ts">
import { Pagination } from "bits-ui";
import { tick, untrack } from "svelte";
import { apiFetch } from "./api.ts";
import { countActiveFilters } from "./filters.ts";
import { focusIsFree, rowSelector } from "./focus.ts";
import PrListItem from "./PrListItem.svelte";
import { registerShortcuts, type Shortcut } from "./shortcuts.ts";
import { onPrInfoUpdated } from "./sse.svelte.ts";
import { showError } from "./toast.svelte.ts";
import type { ActiveFilters } from "./types.ts";
import {
    DEFAULT_PER_PAGE,
    type InboxItem,
    type PaginatedInbox,
} from "./types.ts";

let {
    currentView = "inbox",
    onSelect = (_notification: InboxItem) => {},
    onSelectionChange = (_notification: InboxItem | null) => {},
    selectedId = null,
    refreshKey = 0,
    activeFilters = {} as ActiveFilters,
    onClearFilters = () => {},
}: {
    currentView?: string;
    onSelect?: (notification: InboxItem) => void;
    onSelectionChange?: (notification: InboxItem | null) => void;
    selectedId?: string | null;
    refreshKey?: number;
    activeFilters?: ActiveFilters;
    onClearFilters?: () => void;
} = $props();

let notifications: InboxItem[] = $state([]);
let listEl: HTMLElement | undefined = $state(undefined);
// Incremented only when the full list is re-fetched (not on per-item SSE updates).
// The IntersectionObserver effect depends on this so it doesn't restart on every mutation.
let listVersion = $state(0);
// Tracks which notification IDs have already been sent to the prefetch endpoint.
// Cleared when the list is re-fetched.
const prefetchedIds = new Set<string>();
let currentPage = $state(1);
let totalCount = $state(0);
const PER_PAGE = DEFAULT_PER_PAGE;

// Deliberately non-reactive: only compared inside the refetch effect, and
// making them $state would re-trigger the effect it just ran in.
let lastFiltersKey = "";

function buildInboxUrl(view: string, page: number): string {
    const params = new URLSearchParams({
        status: view,
        page: String(page),
        per_page: String(PER_PAGE),
    });
    if (activeFilters.repo) params.set("repo", activeFilters.repo);
    if (activeFilters.team) params.set("team", activeFilters.team);
    if (activeFilters.author) params.set("author", activeFilters.author);
    const states = activeFilters.states ?? {};
    const include = Object.keys(states).filter((s) => states[s] === "include");
    const exclude = Object.keys(states).filter((s) => states[s] === "exclude");
    if (include.length) params.set("state_include", include.join(","));
    if (exclude.length) params.set("state_exclude", exclude.join(","));
    return `/api/inbox?${params.toString()}`;
}

// Subscribe inside the effect so setup and teardown share a lifecycle — a
// module-level subscription with effect-only cleanup leaks the listener if
// the component is destroyed before its first effect flush.
$effect(() => {
    return onPrInfoUpdated((data) => {
        const item = notifications.find(
            (n) => n.pr_id === data.pr_id && n.repository === data.repository,
        );
        if (item) {
            item.author = data.author;
            item.pr_status = data.pr_status;
            item.ci_status = data.ci_status;
            // Assign unconditionally: null is meaningful here (first visit —
            // the "New pull request" state), only `undefined` means "not
            // enriched yet".
            item.new_commits = data.new_commits;
            item.new_comments = data.new_comments;
            item.new_reviews = data.new_reviews;
            if (data.teams !== null) item.teams = data.teams;
            notifications = [...notifications];
        }
    });
});

// IntersectionObserver: prefetch PR data for visible inbox rows.
// Depends on listVersion (incremented only on full re-fetch) so SSE-triggered
// mutations to individual items do not restart the observer unnecessarily.
$effect(() => {
    void listVersion;
    if (!listEl) return;

    const pendingIds = new Set<string>();
    let timer: ReturnType<typeof setTimeout> | null = null;

    function schedulePrefetch() {
        if (timer) clearTimeout(timer);
        timer = setTimeout(() => {
            const items = notifications.filter(
                (n) => pendingIds.has(n.id) && n.pr_id !== null,
            );
            pendingIds.clear();
            if (items.length === 0) return;
            void apiFetch("/api/inbox/prefetch", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    items: items.map((n) => ({
                        repository: n.repository,
                        pr_number: n.pr_id,
                    })),
                }),
            });
        }, 200);
    }

    const observer = new IntersectionObserver(
        (entries) => {
            let changed = false;
            for (const entry of entries) {
                if (entry.isIntersecting) {
                    const id = (entry.target as HTMLElement).dataset.notifId;
                    if (id && !prefetchedIds.has(id)) {
                        pendingIds.add(id);
                        prefetchedIds.add(id);
                        changed = true;
                    }
                }
            }
            if (changed) schedulePrefetch();
        },
        { rootMargin: "200px" },
    );

    listEl.querySelectorAll(rowSelector()).forEach((el) => {
        observer.observe(el);
    });

    return () => {
        observer.disconnect();
        if (timer) clearTimeout(timer);
    };
});

// Monotonic id per fetch: rapid page/filter changes can resolve out of order,
// and an older response must not overwrite a newer one.
let fetchSeq = 0;

async function fetchNotifications(
    view: string,
    page: number = currentPage,
): Promise<PaginatedInbox | null> {
    const seq = ++fetchSeq;
    try {
        const result = await apiFetch<PaginatedInbox>(
            buildInboxUrl(view, page),
        );
        if (seq !== fetchSeq) return null; // superseded by a newer fetch
        notifications = result.items;
        totalCount = result.total;
        currentPage = result.page;
        prefetchedIds.clear();
        listVersion++;
        return result;
    } catch (err) {
        console.error("Failed to fetch notifications:", err);
        if (seq === fetchSeq) showError("Failed to load notifications");
        return null;
    }
}

// Single effect: reset to page 1 when view or filters change, otherwise refetch
// current page. Only refreshKey, currentView and activeFilters are tracked;
// everything else (the change bookkeeping, currentPage, the fetch's own state
// writes) runs untracked so the effect never re-triggers itself into a
// duplicate fetch.
let lastView = "inbox";
$effect(() => {
    void refreshKey;
    void currentView;
    const filtersKey = JSON.stringify(activeFilters);
    untrack(() => {
        const changed =
            currentView !== lastView || filtersKey !== lastFiltersKey;
        lastView = currentView;
        lastFiltersKey = filtersKey;
        if (changed) currentPage = 1;
        fetchNotifications(currentView, changed ? 1 : currentPage);
    });
});

let count = $derived(notifications.length);
let unreadCount = $derived(notifications.filter((n) => n.unread).length);
let viewTitle = $derived(currentView === "archived" ? "Archived" : "Inbox");
let totalPages = $derived(Math.max(1, Math.ceil(totalCount / PER_PAGE)));
let hasActiveFilters = $derived(countActiveFilters(activeFilters) > 0);
let emptyMessage = $derived(
    currentView === "archived"
        ? "No archived notifications."
        : "All caught up!",
);

async function markRead(notif: InboxItem): Promise<void> {
    if (!notif.unread) return;
    notif.unread = false;
    notifications = [...notifications];
    try {
        await apiFetch(`/api/inbox/${notif.id}/read`, { method: "POST" });
    } catch (err) {
        console.error("Failed to mark read:", err);
        showError("Failed to mark notification as read");
    }
}

function handleSelect(notif: InboxItem): void {
    cursorId = notif.id;
    // markRead flips the flag synchronously; the panel need not wait for
    // the POST.
    void markRead(notif);
    onSelect(notif);
}

// Open without toggling: the parent closes the panel when the already-open
// row is selected again, which a keyboard "open" must never do.
function open(notif: InboxItem): void {
    if (notif.id !== selectedId) handleSelect(notif);
}

// o / Enter: open in the panel, or on GitHub when it is already open.
function activate(notif: InboxItem): void {
    if (notif.id === selectedId) openOnGitHub(notif);
    else open(notif);
}

function openOnGitHub(notif: InboxItem): void {
    if (notif.pr_id === null) return;
    window.open(
        `https://github.com/${notif.repository}/pull/${notif.pr_id}`,
        "_blank",
        "noopener",
    );
}

// The row to land on after `removedId` leaves the list: its nearest
// neighbour (below first, then above) that is still in `latest`, so a row
// removed meanwhile (rapid archives) is never picked.
function nextAfterRemoval(
    previousItems: InboxItem[],
    removedId: string,
    latest: InboxItem[],
): InboxItem | null {
    const index = previousItems.findIndex((item) => item.id === removedId);
    const present = (id: string) => latest.find((item) => item.id === id);
    if (index !== -1) {
        for (let i = index + 1; i < previousItems.length; i++) {
            const hit = present(previousItems[i].id);
            if (hit) return hit;
        }
        for (let i = index - 1; i >= 0; i--) {
            const hit = present(previousItems[i].id);
            if (hit) return hit;
        }
    }
    return latest[0] ?? null;
}

// Re-insert `row` into `current` where it sat in `previous`: before the
// first later row that is still present.
function restoreRow(
    current: InboxItem[],
    previous: InboxItem[],
    row: InboxItem,
): InboxItem[] {
    if (current.some((n) => n.id === row.id)) return current;
    const after = previous.slice(
        previous.findIndex((n) => n.id === row.id) + 1,
    );
    const anchor = after.find((n) => current.some((c) => c.id === n.id));
    const at = anchor
        ? current.findIndex((c) => c.id === anchor.id)
        : current.length;
    return [...current.slice(0, at), row, ...current.slice(at)];
}

// Archive and unarchive both remove the row from the current view, then move
// the selection and the keyboard cursor to its neighbour.
async function moveOut(
    notif: InboxItem,
    action: "archive" | "unarchive",
): Promise<void> {
    const prevNotifications = [...notifications];
    const wasSelected = selectedId === notif.id;
    const viewAtStart = currentView;
    const pageAtStart = currentPage;
    notifications = notifications.filter((n) => n.id !== notif.id);
    // Move the cursor at once; the refetch below may refine it.
    const provisionalCursor =
        cursorId === notif.id
            ? (nextAfterRemoval(prevNotifications, notif.id, notifications)
                  ?.id ?? null)
            : undefined;
    if (provisionalCursor !== undefined) cursorId = provisionalCursor;
    try {
        await apiFetch(`/api/inbox/${notif.id}/${action}`, { method: "POST" });
        // Refetch; if page is now empty and not page 1, go back
        const page =
            notifications.length === 0 && currentPage > 1
                ? currentPage - 1
                : currentPage;
        const result = await fetchNotifications(currentView, page);
        // A superseded refetch returns null: fall back to the local list,
        // which already reflects any later archive.
        const next = nextAfterRemoval(
            prevNotifications,
            notif.id,
            result?.items ?? notifications,
        );
        // Only move what nobody has moved since (another archive, j/k).
        if (provisionalCursor !== undefined && cursorId === provisionalCursor) {
            cursorId = next?.id ?? null;
        }
        if (wasSelected && selectedId === notif.id) onSelectionChange(next);
    } catch (err) {
        console.error(`Failed to ${action}:`, err);
        showError(`Failed to ${action} notification`);
        // Put back only this row (another archive may have succeeded since),
        // and only into the page it came from: after j/k crossed pages or the
        // view changed, it is not on screen and the next refetch shows it.
        if (currentView === viewAtStart && currentPage === pageAtStart) {
            notifications = restoreRow(notifications, prevNotifications, notif);
        }
        if (provisionalCursor !== undefined && cursorId === provisionalCursor) {
            cursorId = notif.id;
        }
        // The selection only moves on success, so it needs no restoring.
    }
}

function goToPage(page: number): void {
    fetchNotifications(currentView, page);
}

// --- Keyboard navigation ---

// The keyboard cursor (j/k) is separate from the open PR, so moving through
// the list does not mark every row read or fetch every PR.
let cursorId: string | null = $state(null);

// Follow selection changes made elsewhere (click, select-next after archive).
$effect(() => {
    if (selectedId) cursorId = selectedId;
});

function cursorRow(): InboxItem | undefined {
    return notifications.find((n) => n.id === cursorId);
}

// Keep the cursor row visible. With no PR open, also focus it (when focus
// is not on another control: a sidebar button, a pagination button), so
// screen readers announce it and Enter on the focused row is the cursor row.
// With a PR open, PrDetail focuses its timeline instead, so Space and Page
// Down scroll the PR. Tracking selectedId hands focus back to the row when
// the panel closes.
$effect(() => {
    const id = cursorId;
    const panelOpen = selectedId != null;
    if (!id) return;
    void tick().then(() => {
        const row = listEl?.querySelector<HTMLElement>(rowSelector(id));
        if (!row) return;
        row.scrollIntoView?.({ block: "nearest" });
        if (!panelOpen && focusIsFree()) row.focus({ preventScroll: true });
    });
});

// Set while j/k load the adjacent page, so presses during the load neither
// refetch the same page nor move the cursor on the page being replaced.
let crossingPage = false;

async function moveCursor(delta: 1 | -1): Promise<void> {
    if (crossingPage || notifications.length === 0) return;
    const index = notifications.findIndex((n) => n.id === cursorId);
    let target: InboxItem | undefined =
        index === -1
            ? notifications[delta > 0 ? 0 : notifications.length - 1]
            : notifications[index + delta];
    if (!target) {
        // Past either end of the page: continue on the adjacent page.
        const page = currentPage + delta;
        if (page < 1 || page > totalPages) return;
        crossingPage = true;
        try {
            const result = await fetchNotifications(currentView, page);
            if (!result || notifications.length === 0) return;
        } finally {
            crossingPage = false;
        }
        target = notifications[delta > 0 ? 0 : notifications.length - 1];
    }
    cursorId = target.id;
    // With the detail panel open, j/k walk through PRs like Gmail's
    // conversation view.
    if (selectedId) open(target);
}

// Shortcut fields for an action on the cursor row: active only while there
// is one, so the key falls through otherwise.
function onCursorRow(
    fn: (notif: InboxItem) => void,
): Pick<Shortcut, "when" | "run"> {
    return {
        when: () => cursorRow() !== undefined,
        run: () => {
            const notif = cursorRow();
            if (notif) fn(notif);
        },
    };
}

$effect(() =>
    registerShortcuts([
        {
            keys: ["j", "ArrowDown"],
            description: "Next pull request",
            group: "Navigation",
            repeat: true,
            run: () => void moveCursor(1),
        },
        {
            keys: ["k", "ArrowUp"],
            description: "Previous pull request",
            group: "Navigation",
            repeat: true,
            run: () => void moveCursor(-1),
        },
        {
            keys: ["o", "Enter"],
            description: "Open pull request (on GitHub if already open)",
            group: "Navigation",
            ...onCursorRow(activate),
        },
        {
            keys: ["e"],
            description: "Archive (unarchive in Archived)",
            group: "Actions",
            ...onCursorRow((notif) => {
                void moveOut(
                    notif,
                    currentView === "archived" ? "unarchive" : "archive",
                );
            }),
        },
        {
            keys: ["I"],
            description: "Mark as read",
            group: "Actions",
            ...onCursorRow((notif) => void markRead(notif)),
        },
        {
            keys: ["v", "O"],
            description: "Open on GitHub",
            group: "Actions",
            ...onCursorRow(openOnGitHub),
            when: () => cursorRow()?.pr_id != null,
        },
    ]),
);
</script>

<div class="main">
    <div class="list-header">
        <span class="list-title">{viewTitle}</span>
        <span class="list-count"
            >{totalCount}
            {#if currentView !== "archived"}
                · {unreadCount} unread
            {/if}</span
        >
        <div class="list-spacer"></div>
    </div>

    <div class="pr-list" bind:this={listEl}>
        {#if count === 0}
            <div class="empty-state">
                {#if hasActiveFilters}
                    <span>No pull requests match your filters.</span>
                    <button
                        type="button"
                        class="empty-clear-btn"
                        onclick={onClearFilters}
                    >
                        Clear filters
                    </button>
                {:else}
                    {emptyMessage}
                {/if}
            </div>
        {:else}
            {#each notifications as notif (notif.id)}
                <PrListItem
                    {notif}
                    selected={notif.id === selectedId}
                    cursor={notif.id === cursorId && notif.id !== selectedId}
                    {currentView}
                    onSelect={handleSelect}
                    onActivate={activate}
                    onFocus={(n) => {
                        cursorId = n.id;
                    }}
                    onArchive={(n) => moveOut(n, "archive")}
                    onUnarchive={(n) => moveOut(n, "unarchive")}
                />
            {/each}
        {/if}
    </div>

    <div class="statusbar">
        {#if totalPages > 1}
            <Pagination.Root
                page={currentPage}
                count={totalCount}
                perPage={PER_PAGE}
                onPageChange={goToPage}
                style="display:flex;align-items:center;gap:4px"
            >
                <Pagination.PrevButton
                    class="page-btn"
                    aria-label="Previous page"
                >
                    <svg
                        aria-hidden="true"
                        width="12"
                        height="12"
                        viewBox="0 0 16 16"
                        fill="currentColor"
                    >
                        <path
                            d="M9.78 12.78a.75.75 0 0 1-1.06 0L4.47 8.53a.75.75 0 0 1 0-1.06l4.25-4.25a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042L6.06 8l3.72 3.72a.75.75 0 0 1 0 1.06Z"
                        />
                    </svg>
                </Pagination.PrevButton>
                <span class="page-info"
                    >Page {currentPage} of {totalPages}</span
                >
                <Pagination.NextButton class="page-btn" aria-label="Next page">
                    <svg
                        aria-hidden="true"
                        width="12"
                        height="12"
                        viewBox="0 0 16 16"
                        fill="currentColor"
                    >
                        <path
                            d="M6.22 3.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L9.94 8 6.22 4.28a.75.75 0 0 1 0-1.06Z"
                        />
                    </svg>
                </Pagination.NextButton>
            </Pagination.Root>
        {/if}
        <div class="statusbar-spacer"></div>
        <span class="statusbar-count"
            >{totalCount}
            PRs
            {#if currentView !== "archived"}
                · {unreadCount} unread
            {/if}</span
        >
    </div>
</div>

<style>
.main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}
.list-header {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-default);
    gap: 8px;
    flex-shrink: 0;
}
.list-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--fg-default);
}
.list-count {
    font-size: 12px;
    color: var(--fg-muted);
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 2em;
    padding: 0 8px;
    line-height: 20px;
}
.list-spacer {
    flex: 1;
}
.pr-list {
    flex: 1;
    min-width: var(--pr-list-min-w);
    overflow-y: auto;
}
.empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 100%;
    color: var(--fg-muted);
    font-size: 14px;
}
.empty-clear-btn {
    font-size: 12px;
    color: var(--accent-fg);
    background: none;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 4px 12px;
    cursor: pointer;
    font-family: inherit;
}
.empty-clear-btn:hover {
    background: var(--canvas-subtle);
}

.statusbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 16px;
    height: 28px;
    border-top: 1px solid var(--border-default);
    background: var(--canvas-subtle);
    flex-shrink: 0;
    color: var(--fg-subtle);
    font-size: 12px;
}
:global(.page-btn) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
    padding: 0;
    font-family: inherit;
}
:global(.page-btn:hover:not(:disabled)) {
    background: var(--border-muted);
    color: var(--fg-default);
}
:global(.page-btn:disabled) {
    opacity: 0.4;
    cursor: default;
}
.page-info {
    font-size: 12px;
    color: var(--fg-muted);
    user-select: none;
}
.statusbar-spacer {
    flex: 1;
}
.statusbar-count {
    color: var(--fg-subtle);
}
</style>
