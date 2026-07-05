<script lang="ts">
import { Pagination } from "bits-ui";
import { untrack } from "svelte";
import { apiFetch } from "./api.ts";
import { countActiveFilters } from "./filters.ts";
import PrListItem from "./PrListItem.svelte";
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

    listEl.querySelectorAll("[data-notif-id]").forEach((el) => {
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

async function handleSelect(notif: InboxItem): Promise<void> {
    if (notif.unread) {
        notif.unread = false;
        notifications = [...notifications];
        try {
            await apiFetch(`/api/inbox/${notif.id}/read`, { method: "POST" });
        } catch (err) {
            console.error("Failed to mark read:", err);
            showError("Failed to mark notification as read");
        }
    }
    onSelect(notif);
}

function getAdjacentNotification(
    items: InboxItem[],
    removedId: string,
): InboxItem | null {
    const removedIndex = items.findIndex((item) => item.id === removedId);
    if (removedIndex === -1) return null;
    return items[removedIndex + 1] ?? items[removedIndex - 1] ?? null;
}

function resolveNextSelection(
    removedNotification: InboxItem,
    previousItems: InboxItem[],
    refreshedItems: InboxItem[] | null,
): InboxItem | null {
    const adjacent = getAdjacentNotification(
        previousItems,
        removedNotification.id,
    );
    if (adjacent === null) {
        return refreshedItems?.[0] ?? null;
    }

    return refreshedItems?.find((item) => item.id === adjacent.id) ?? adjacent;
}

async function handleArchive(e: MouseEvent, notif: InboxItem): Promise<void> {
    e.stopPropagation();
    const prevNotifications = [...notifications];
    const archivedWasSelected = selectedId === notif.id;
    notifications = notifications.filter((n) => n.id !== notif.id);
    try {
        await apiFetch(`/api/inbox/${notif.id}/archive`, { method: "POST" });
        // Refetch; if page is now empty and not page 1, go back
        const page =
            notifications.length === 0 && currentPage > 1
                ? currentPage - 1
                : currentPage;
        const result = await fetchNotifications(currentView, page);
        if (archivedWasSelected) {
            onSelectionChange(
                resolveNextSelection(
                    notif,
                    prevNotifications,
                    result?.items ?? null,
                ),
            );
        }
    } catch (err) {
        console.error("Failed to archive:", err);
        showError("Failed to archive notification");
        notifications = prevNotifications;
        if (archivedWasSelected) {
            onSelectionChange(notif);
        }
    }
}

function goToPage(page: number): void {
    fetchNotifications(currentView, page);
}

async function handleUnarchive(e: MouseEvent, notif: InboxItem): Promise<void> {
    e.stopPropagation();
    const prevNotifications = [...notifications];
    const unarchivedWasSelected = selectedId === notif.id;
    notifications = notifications.filter((n) => n.id !== notif.id);
    try {
        await apiFetch(`/api/inbox/${notif.id}/unarchive`, { method: "POST" });
        const page =
            notifications.length === 0 && currentPage > 1
                ? currentPage - 1
                : currentPage;
        const result = await fetchNotifications(currentView, page);
        if (unarchivedWasSelected) {
            onSelectionChange(
                resolveNextSelection(
                    notif,
                    prevNotifications,
                    result?.items ?? null,
                ),
            );
        }
    } catch (err) {
        console.error("Failed to unarchive:", err);
        showError("Failed to unarchive notification");
        notifications = prevNotifications;
        if (unarchivedWasSelected) {
            onSelectionChange(notif);
        }
    }
}
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
                    {currentView}
                    onSelect={handleSelect}
                    onArchive={handleArchive}
                    onUnarchive={handleUnarchive}
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
