<script lang="ts">
import { apiFetch } from "./api.ts";
import { reasonClass, reasonLabel } from "./reason.ts";
import { onPrInfoUpdated, onPrTeamsUpdated } from "./sse.svelte.ts";
import { timeAgo } from "./timeago.ts";
import { showError } from "./toast.svelte.ts";
import type { InboxItem } from "./types.ts";

let {
    currentView = "inbox",
    onSelect = (_notification: InboxItem) => {},
    selectedId = null,
    refreshKey = 0,
}: {
    currentView?: string;
    onSelect?: (notification: InboxItem) => void;
    selectedId?: string | null;
    refreshKey?: number;
} = $props();

let notifications: InboxItem[] = $state([]);
let listEl: HTMLElement | undefined = $state(undefined);
// Incremented only when the full list is re-fetched (not on per-item SSE updates).
// The IntersectionObserver effect depends on this so it doesn't restart on every mutation.
let listVersion = $state(0);
// Tracks which notification IDs have already been sent to the prefetch endpoint.
// Cleared when the list is re-fetched.
const prefetchedIds = new Set<string>();

const unsubTeams = onPrTeamsUpdated((pr_id, teams) => {
    const item = notifications.find((n) => n.pr_id === pr_id);
    if (item) {
        item.teams = teams;
        notifications = [...notifications];
    }
});
// Svelte 5 runes cleanup — return the unsubscribe fn from $effect:
$effect(() => {
    return unsubTeams;
});

const unsubInfo = onPrInfoUpdated((data) => {
    const item = notifications.find(
        (n) => n.pr_id === data.pr_id && n.repository === data.repository,
    );
    if (item) {
        item.author = data.author;
        item.pr_status = data.pr_status as InboxItem["pr_status"];
        if (data.new_commits !== null) item.new_commits = data.new_commits;
        if (data.new_comments !== null) item.new_comments = data.new_comments;
        notifications = [...notifications];
    }
});
$effect(() => {
    return unsubInfo;
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

async function fetchNotifications(view: string): Promise<void> {
    try {
        notifications = await apiFetch<InboxItem[]>(
            `/api/inbox?status=${view}`,
        );
        prefetchedIds.clear();
        listVersion++;
    } catch (err) {
        console.error("Failed to fetch notifications:", err);
        showError("Failed to load notifications");
    }
}

$effect(() => {
    void refreshKey;
    fetchNotifications(currentView);
});

let count = $derived(notifications.length);
let unreadCount = $derived(notifications.filter((n) => n.unread).length);
let viewTitle = $derived(currentView === "archived" ? "Archived" : "Inbox");
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

async function handleArchive(e: MouseEvent, notif: InboxItem): Promise<void> {
    e.stopPropagation();
    notifications = notifications.filter((n) => n.id !== notif.id);
    try {
        await apiFetch(`/api/inbox/${notif.id}/archive`, { method: "POST" });
    } catch (err) {
        console.error("Failed to archive:", err);
        showError("Failed to archive notification");
        notifications = [...notifications, notif];
    }
}

async function handleUnarchive(e: MouseEvent, notif: InboxItem): Promise<void> {
    e.stopPropagation();
    notifications = notifications.filter((n) => n.id !== notif.id);
    try {
        await apiFetch(`/api/inbox/${notif.id}/unarchive`, { method: "POST" });
    } catch (err) {
        console.error("Failed to unarchive:", err);
        showError("Failed to unarchive notification");
        notifications = [...notifications, notif];
    }
}

// SVG path data for GitHub Octicons (16px)
const STATUS_ICONS: Record<string, string> = {
    open: "M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354ZM3.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm8.25.75a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Z",
    draft: "M3.25 1A2.25 2.25 0 0 1 4 5.372v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 3.25 1Zm9.5 14a2.25 2.25 0 1 1 0-4.5 2.25 2.25 0 0 1 0 4.5ZM3.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm9.5 0a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5ZM8.655 5.5h-.31c-.337 0-.67-.033-.994-.097l-.28 1.476c.396.075.803.113 1.274.12h.31c.47-.007.878-.045 1.274-.12l-.28-1.476a6.21 6.21 0 0 1-.994.097Zm-4.275-2.8-.478 1.388c.34.117.696.21 1.052.275l.28-1.476a8.258 8.258 0 0 1-.854-.187ZM11.62 2.7a8.28 8.28 0 0 1-.854.187l.28 1.476c.356-.065.711-.158 1.052-.275L11.62 2.7ZM8.5 2.015V.5a.5.5 0 0 0-1 0v1.515c-.179.01-.357.03-.534.055L7.247 3.546c.248-.038.502-.063.753-.07V3.5h0Zm0 0-.003.031c.252.007.506.032.753.07l.281-1.476A8.342 8.342 0 0 0 9 2.015v.031h0V.5a.5.5 0 0 0-1 0v1.515Z",
    merged: "M5.45 5.154A4.25 4.25 0 0 0 9.25 7.5h1.378a2.251 2.251 0 1 1 0 1.5H9.25A5.734 5.734 0 0 1 5 7.123v3.505a2.25 2.25 0 1 1-1.5 0V5.372A2.25 2.25 0 1 1 5.45 5.154ZM4.25 13.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm8.5-4.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5ZM5 3.25a.75.75 0 1 0 0 .005V3.25Z",
    closed: "M3.25 1A2.25 2.25 0 0 1 4 5.372v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 3.25 1Zm9.96 5.016a.75.75 0 1 0-1.06-1.06L10.5 6.61 8.84 4.94a.75.75 0 0 0-1.061 1.06l1.661 1.661-1.661 1.661a.75.75 0 1 0 1.06 1.06L10.5 8.72l1.661 1.661a.75.75 0 1 0 1.06-1.06L11.56 7.66l1.65-1.644ZM3.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z",
};

function activitySentence(item: InboxItem): string | null {
    if (!item.author) return null;
    if (item.new_commits === null) return null; // first visit — handled separately
    const parts: string[] = [];
    if (item.new_commits > 0) {
        const n = item.new_commits;
        parts.push(`${item.author} pushed ${n} commit${n === 1 ? "" : "s"}`);
    }
    if (item.new_comments && item.new_comments.length > 0) {
        const actors = formatActors(item.new_comments.map((c) => c.author));
        const total = item.new_comments.reduce((s, c) => s + c.count, 0);
        parts.push(`${actors} left ${total} comment${total === 1 ? "" : "s"}`);
    }
    return parts.length > 0 ? parts.join(" · ") : "";
}

function formatActors(names: string[]): string {
    if (names.length === 0) return "";
    if (names.length === 1) return names[0];
    return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

function avatarUrl(login: string): string {
    return `https://github.com/${login}.png?size=64`;
}

function initials(login: string): string {
    return login.charAt(0).toUpperCase();
}
</script>

<div class="main">
    <div class="list-header">
        <span class="list-title">{viewTitle}</span>
        <span class="list-count"
            >{count}
            {#if currentView !== "archived"}
                · {unreadCount} unread
            {/if}</span
        >
        <div class="list-spacer"></div>
        <button type="button" class="filter-btn">
            <svg
                aria-hidden="true"
                width="14"
                height="14"
                viewBox="0 0 16 16"
                fill="currentColor"
            >
                <path
                    d="M.75 3h14.5a.75.75 0 0 1 0 1.5H.75a.75.75 0 0 1 0-1.5ZM3 7.75A.75.75 0 0 1 3.75 7h8.5a.75.75 0 0 1 0 1.5h-8.5A.75.75 0 0 1 3 7.75Zm3 4a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z"
                />
            </svg>
            Filter
        </button>
    </div>

    <div class="pr-list" bind:this={listEl}>
        {#if count === 0}
            <div class="empty-state">{emptyMessage}</div>
        {:else}
            {#each notifications as notif (notif.id)}
                {@const sentence = activitySentence(notif)}
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
                    <div class="unread-dot" class:read={!notif.unread}></div>

                    <!-- Status icon -->
                    <div class="status-icon-slot">
                        {#if notif.pr_status && STATUS_ICONS[notif.pr_status]}
                            <svg
                                role="img"
                                aria-label={notif.pr_status}
                                class="status-icon status-icon-{notif.pr_status}"
                                width="16"
                                height="16"
                                viewBox="0 0 16 16"
                                fill="currentColor"
                            >
                                <title>{notif.pr_status}</title>
                                <path d={STATUS_ICONS[notif.pr_status]} />
                            </svg>
                        {:else if notif.pr_id && notif.pr_status === null}
                            <div class="status-icon-shimmer"></div>
                        {:else}
                            <div class="status-icon-empty"></div>
                        {/if}
                    </div>

                    <!-- Body -->
                    <div class="pr-body">
                        <!-- Top meta: repo · teams -->
                        <div class="pr-meta-top">
                            <span class="pr-repo">{notif.repository}</span>
                            {#if notif.teams && notif.teams.length > 0}
                                {#each notif.teams as team}
                                    <span class="divider">·</span>
                                    <span class="badge badge-team"
                                        >@{team}</span
                                    >
                                {/each}
                            {/if}
                        </div>

                        <!-- Title -->
                        <div class="pr-title">
                            {#if notif.pr_id}
                                <span class="pr-num">#{notif.pr_id}</span>
                            {/if}
                            {notif.title}
                        </div>

                        <!-- Activity line -->
                        {#if notif.author}
                            <div class="pr-activity">
                                {#if notif.new_commits === null}
                                    <span class="activity-new-pr"
                                        >✦ New pull request</span
                                    >
                                {:else if sentence === ""}
                                    <span class="activity-quiet"
                                        >No new activity since your last visit</span
                                    >
                                {:else if sentence}
                                    <span class="activity-text"
                                        >{sentence}</span
                                    >
                                {/if}
                            </div>
                        {/if}
                    </div>

                    <!-- Right column -->
                    <div class="pr-right">
                        <!-- Avatar -->
                        <div class="avatar-slot">
                            {#if notif.author}
                                <img
                                    class="avatar"
                                    src={avatarUrl(notif.author)}
                                    alt={notif.author}
                                    onerror={(e) => {
                                        const el = e.currentTarget as HTMLElement;
                                        el.outerHTML = `<div class="avatar avatar-initials">${initials(notif.author!)}</div>`;
                                    }}
                                >
                            {:else}
                                <div class="avatar avatar-empty"></div>
                            {/if}
                        </div>
                        <span class="label label-{reasonClass(notif.reason)}"
                            >{reasonLabel(notif.reason)}</span
                        >
                        <span class="pr-date">{timeAgo(notif.updated_at)}</span>
                        <div class="pr-actions">
                            {#if currentView === "inbox"}
                                <button
                                    class="action-btn"
                                    type="button"
                                    title="Archive"
                                    onclick={(e) => handleArchive(e, notif)}
                                >
                                    <svg
                                        aria-hidden="true"
                                        width="14"
                                        height="14"
                                        viewBox="0 0 16 16"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M1.75 1h12.5c.966 0 1.75.784 1.75 1.75v2.5A1.75 1.75 0 0 1 14.25 7H1.75A1.75 1.75 0 0 1 0 5.25v-2.5C0 1.784.784 1 1.75 1Zm0 1.5a.25.25 0 0 0-.25.25v2.5c0 .138.112.25.25.25h12.5a.25.25 0 0 0 .25-.25v-2.5a.25.25 0 0 0-.25-.25ZM1 8.75v5.5c0 .966.784 1.75 1.75 1.75h10.5A1.75 1.75 0 0 0 15 14.25v-5.5a.75.75 0 0 0-1.5 0v5.5a.25.25 0 0 1-.25.25H2.75a.25.25 0 0 1-.25-.25v-5.5a.75.75 0 0 0-1.5 0ZM5 10.25a.75.75 0 0 1 .75-.75h4.5a.75.75 0 0 1 0 1.5h-4.5a.75.75 0 0 1-.75-.75Z"
                                        />
                                    </svg>
                                </button>
                            {:else}
                                <button
                                    class="action-btn"
                                    type="button"
                                    title="Unarchive"
                                    onclick={(e) => handleUnarchive(e, notif)}
                                >
                                    <svg
                                        aria-hidden="true"
                                        width="14"
                                        height="14"
                                        viewBox="0 0 16 16"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v9.5A1.75 1.75 0 0 1 14.25 13H8.06l-2.573 2.573A1.458 1.458 0 0 1 3 14.543V13H1.75A1.75 1.75 0 0 1 0 11.25Zm1.75-.25a.25.25 0 0 0-.25.25v9.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-9.5a.25.25 0 0 0-.25-.25Z"
                                        />
                                    </svg>
                                </button>
                            {/if}
                        </div>
                    </div>
                </div>
            {/each}
        {/if}
    </div>

    <div class="statusbar">
        <span
            >{count}
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
.filter-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 500;
    color: var(--fg-default);
    cursor: pointer;
    font-family: inherit;
}
.filter-btn:hover {
    background: var(--border-muted);
}
.pr-list {
    flex: 1;
    overflow-y: auto;
}
.empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--fg-muted);
    font-size: 14px;
}

/* PR row */
.pr-item {
    position: relative;
    display: flex;
    align-items: flex-start;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-muted);
    cursor: pointer;
    gap: 12px;
}
.pr-item:hover {
    background: var(--canvas-subtle);
}
.pr-item.selected {
    background: var(--accent-subtle);
    border-left: 2px solid var(--accent-fg);
}

.unread-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-fg);
    flex-shrink: 0;
    margin-top: 12px;
}
.unread-dot.read {
    background: transparent;
}

/* Status icon (left column) */
.status-icon-slot {
    flex-shrink: 0;
    width: 16px;
    display: flex;
    align-items: flex-start;
    padding-top: 3px;
}
.status-icon {
    flex-shrink: 0;
}
.status-icon-empty {
    width: 16px;
    height: 16px;
}
.status-icon-shimmer {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: linear-gradient(
        90deg,
        var(--border-default) 25%,
        var(--border-muted) 50%,
        var(--border-default) 75%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s infinite;
}
.status-icon-open {
    color: #3fb950;
}
.status-icon-draft {
    color: var(--fg-muted);
}
.status-icon-merged {
    color: #a371f7;
}
.status-icon-closed {
    color: #f85149;
}

/* Avatar (right column) */
.avatar-slot {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
}
.avatar {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px solid var(--border-default);
    object-fit: cover;
}
.avatar-initials {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: #21262d;
    border: 1px solid var(--border-default);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 600;
    color: var(--fg-muted);
}
.avatar-empty {
    width: 24px;
    height: 24px;
}

.pr-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
}
.pr-meta-top {
    display: flex;
    align-items: center;
    gap: 6px;
}
.pr-repo {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
    white-space: nowrap;
}
.pr-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--fg-default);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.pr-item.read .pr-title {
    font-weight: 400;
    color: var(--fg-muted);
}
.pr-num {
    font-size: 12px;
    font-weight: 400;
    color: var(--fg-subtle);
    margin-right: 4px;
}

/* Team badges */
.badge-team {
    display: inline-flex;
    align-items: center;
    font-size: 10px;
    padding: 0 6px;
    border-radius: 2em;
    border: 1px solid rgba(210, 153, 34, 0.4);
    background: rgba(210, 153, 34, 0.1);
    color: #e3b341;
    white-space: nowrap;
    line-height: 17px;
}
@keyframes shimmer {
    0% {
        background-position: 200% 0;
    }
    100% {
        background-position: -200% 0;
    }
}

/* Activity line */
.pr-activity {
    font-size: 11px;
    color: var(--fg-muted);
}
.activity-new-pr {
    color: var(--fg-default);
    font-weight: 500;
}
.activity-quiet {
    color: var(--fg-subtle);
    font-style: italic;
}
.activity-text {
    color: var(--fg-muted);
}

/* Divider between meta items */
.divider {
    font-size: 11px;
    color: var(--fg-subtle);
}

/* Reason pill */
.label {
    display: inline-flex;
    align-items: center;
    font-size: 12px;
    font-weight: 500;
    padding: 0 8px;
    border-radius: 2em;
    border: 1px solid;
    white-space: nowrap;
    line-height: 20px;
}
.label-review {
    border-color: rgba(47, 129, 247, 0.4);
    background: rgba(47, 129, 247, 0.1);
    color: #79c0ff;
}
.label-mention {
    border-color: rgba(163, 113, 247, 0.4);
    background: rgba(163, 113, 247, 0.1);
    color: #c9b1f7;
}
.label-assign {
    border-color: rgba(210, 153, 34, 0.4);
    background: rgba(210, 153, 34, 0.1);
    color: #e3b341;
}
.label-default {
    border-color: var(--border-default);
    background: var(--canvas-subtle);
    color: var(--fg-muted);
}

/* Right column */
.pr-right {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    padding-top: 2px;
}

/* Date */
.pr-date {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--fg-subtle);
    white-space: nowrap;
}

/* Action buttons */
.pr-actions {
    position: absolute;
    bottom: 10px;
    right: 16px;
    display: flex;
    align-items: center;
}
.action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--canvas-subtle);
    color: var(--fg-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.1s;
}
.pr-item:hover .action-btn {
    opacity: 1;
}
.action-btn:hover {
    background: var(--border-muted);
    color: var(--fg-default);
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
</style>
