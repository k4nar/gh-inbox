<script lang="ts">
import { Tooltip } from "bits-ui";
import { activitySentence } from "./activity.ts";
import { avatarUrl } from "./avatar.ts";
import { timeAgo } from "./timeago.ts";
import type { InboxItem } from "./types.ts";

let {
    notif,
    selected = false,
    currentView = "inbox",
    onSelect,
    onArchive,
    onUnarchive,
}: {
    notif: InboxItem;
    selected?: boolean;
    currentView?: string;
    onSelect: (notification: InboxItem) => void;
    onArchive: (e: MouseEvent, notification: InboxItem) => void;
    onUnarchive: (e: MouseEvent, notification: InboxItem) => void;
} = $props();

// SVG path data for GitHub Octicons (16px)
const STATUS_ICONS: Record<string, string> = {
    open: "M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354ZM3.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm8.25.75a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Z",
    draft: "M3.25 1A2.25 2.25 0 0 1 4 5.372v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 3.25 1Zm9.5 14a2.25 2.25 0 1 1 0-4.5 2.25 2.25 0 0 1 0 4.5ZM3.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm9.5 0a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5ZM8.655 5.5h-.31c-.337 0-.67-.033-.994-.097l-.28 1.476c.396.075.803.113 1.274.12h.31c.47-.007.878-.045 1.274-.12l-.28-1.476a6.21 6.21 0 0 1-.994.097Zm-4.275-2.8-.478 1.388c.34.117.696.21 1.052.275l.28-1.476a8.258 8.258 0 0 1-.854-.187ZM11.62 2.7a8.28 8.28 0 0 1-.854.187l.28 1.476c.356-.065.711-.158 1.052-.275L11.62 2.7ZM8.5 2.015V.5a.5.5 0 0 0-1 0v1.515c-.179.01-.357.03-.534.055L7.247 3.546c.248-.038.502-.063.753-.07V3.5h0Zm0 0-.003.031c.252.007.506.032.753.07l.281-1.476A8.342 8.342 0 0 0 9 2.015v.031h0V.5a.5.5 0 0 0-1 0v1.515Z",
    merged: "M5.45 5.154A4.25 4.25 0 0 0 9.25 7.5h1.378a2.251 2.251 0 1 1 0 1.5H9.25A5.734 5.734 0 0 1 5 7.123v3.505a2.25 2.25 0 1 1-1.5 0V5.372A2.25 2.25 0 1 1 5.45 5.154ZM4.25 13.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm8.5-4.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5ZM5 3.25a.75.75 0 1 0 0 .005V3.25Z",
    closed: "M3.25 1A2.25 2.25 0 0 1 4 5.372v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 3.25 1Zm9.96 5.016a.75.75 0 1 0-1.06-1.06L10.5 6.61 8.84 4.94a.75.75 0 0 0-1.061 1.06l1.661 1.661-1.661 1.661a.75.75 0 1 0 1.06 1.06L10.5 8.72l1.661 1.661a.75.75 0 1 0 1.06-1.06L11.56 7.66l1.65-1.644ZM3.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z",
};

function initials(login: string | null): string {
    return login ? login.charAt(0).toUpperCase() : "?";
}

// Render an initials fallback when the avatar fails to load. Reactive state
// (not an onerror outerHTML swap) so Svelte keeps owning the DOM and the
// scoped .author-avatar styles actually apply.
let avatarFailed = $state(false);

let sentence = $derived(activitySentence(notif));
</script>

<div
    class="pr-item"
    class:read={!notif.unread}
    class:selected
    data-notif-id={notif.id}
    data-ci-status={notif.ci_status ?? ""}
    onclick={() => onSelect(notif)}
    role="button"
    tabindex="0"
    onkeydown={(e) => e.key === 'Enter' && onSelect(notif)}
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
        <!-- Top meta: repo · author · teams -->
        <div class="pr-meta-top">
            <span class="pr-repo">{notif.repository}</span>
            {#if notif.author}
                <span class="divider">·</span>
                <span class="pr-author">
                    by
                    {#if avatarFailed}
                        <span class="author-avatar author-avatar-initials"
                            >{initials(notif.author)}</span
                        >
                    {:else}
                        <img
                            class="author-avatar"
                            src={avatarUrl(notif.author, notif.author_avatar_url, 64)}
                            alt={notif.author}
                            onerror={() => {
                                avatarFailed = true;
                            }}
                        >
                    {/if}
                    <span class="author-name">{notif.author}</span>
                </span>
            {/if}
            {#if notif.teams && notif.teams.length > 0}
                {#each notif.teams as team}
                    <span class="divider">·</span>
                    <span class="badge badge-team">@{team}</span>
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

        <!-- Activity line (always rendered so the row height is fixed) -->
        <div class="pr-activity">
            {#if !notif.author}
                {#if notif.pr_id && notif.pr_status === null}
                    <div class="activity-shimmer"></div>
                {/if}
            {:else if notif.new_commits === null}
                <span class="activity-new-pr">✦ New pull request</span>
            {:else if sentence === ""}
                <span class="activity-quiet"
                    >No new activity since your last visit</span
                >
            {:else if sentence}
                <span class="activity-text">{sentence}</span>
            {/if}
        </div>
    </div>

    <!-- Right column -->
    <div class="pr-right">
        <span class="pr-date">{timeAgo(notif.updated_at)}</span>
        <div class="pr-actions">
            {#if currentView === "inbox"}
                <Tooltip.Root>
                    <Tooltip.Trigger
                        class="action-btn"
                        type="button"
                        aria-label="Archive"
                        onclick={(e) => onArchive(e, notif)}
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
                    </Tooltip.Trigger>
                    <Tooltip.Portal>
                        <Tooltip.Content class="tooltip-content"
                            >Archive</Tooltip.Content
                        >
                    </Tooltip.Portal>
                </Tooltip.Root>
            {:else}
                <Tooltip.Root>
                    <Tooltip.Trigger
                        class="action-btn"
                        type="button"
                        aria-label="Unarchive"
                        onclick={(e) => onUnarchive(e, notif)}
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
                    </Tooltip.Trigger>
                    <Tooltip.Portal>
                        <Tooltip.Content class="tooltip-content"
                            >Unarchive</Tooltip.Content
                        >
                    </Tooltip.Portal>
                </Tooltip.Root>
            {/if}
        </div>
    </div>
</div>

<style>
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

/* Inline author (activity line) */
.pr-author {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 400;
    color: var(--fg-muted);
    white-space: nowrap;
    flex-shrink: 0;
}
.author-avatar {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid var(--border-default);
    object-fit: cover;
    vertical-align: middle;
}
.author-avatar-initials {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #21262d;
    border: 1px solid var(--border-default);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 8px;
    font-weight: 600;
    color: var(--fg-muted);
}
.author-name {
    color: var(--fg-muted);
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
    /* Avatar (16px + 1px borders) is the tallest child; reserve its height
       up front so the line does not grow when PR info loads */
    height: 18px;
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
    height: 16px;
    line-height: 16px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.activity-shimmer {
    width: 180px;
    max-width: 100%;
    height: 8px;
    margin-top: 4px;
    border-radius: 4px;
    background: linear-gradient(
        90deg,
        var(--border-default) 25%,
        var(--border-muted) 50%,
        var(--border-default) 75%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s infinite;
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
:global(.action-btn) {
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
.pr-item:hover :global(.action-btn) {
    opacity: 1;
}
:global(.action-btn:hover) {
    background: var(--border-muted);
    color: var(--fg-default);
}
</style>
