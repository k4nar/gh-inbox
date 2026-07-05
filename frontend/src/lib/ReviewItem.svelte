<script lang="ts">
import { Collapsible } from "bits-ui";
import { timeAgo } from "./timeago.ts";
import type { Review } from "./types.ts";

// Expansion state lives in PrDetail (a SvelteSet keyed by review.id), not
// here: on background reloads previous_viewed_at advances, so a review can
// migrate from the "Since your last visit" zone to the "Earlier" zone. That
// moves it between `{#each}` blocks, which recreates this component — local
// state would collapse a review the user was reading.
let {
    review,
    showBadge,
    expanded,
    onExpandedChange,
}: {
    review: Review;
    showBadge: boolean;
    expanded: boolean;
    onExpandedChange: (expanded: boolean) => void;
} = $props();

function avatarUrl(login: string, apiUrl: string | null): string {
    return apiUrl ?? `https://github.com/${login}.png?size=40`;
}
</script>

<div class="timeline-item review-item">
    {#if review.body}
        <Collapsible.Root
            open={expanded}
            onOpenChange={(v) => onExpandedChange(v)}
        >
            <Collapsible.Trigger class="review-thread-header" type="button">
                <img
                    class="avatar avatar-sm"
                    src={avatarUrl(review.reviewer, review.reviewer_avatar_url)}
                    alt={review.reviewer}
                    width="18"
                    height="18"
                >
                <span class="reviewer-name">{review.reviewer}</span>
                <span
                    class="review-state-pill {review.state === 'APPROVED' ? 'pill-approved' : review.state === 'CHANGES_REQUESTED' ? 'pill-changes' : 'pill-dismissed'}"
                >
                    {review.state === 'APPROVED' ? 'Approved' : review.state === 'CHANGES_REQUESTED' ? 'Changes requested' : 'Dismissed'}
                </span>
                <span class="timestamp">{timeAgo(review.submitted_at)}</span>
                {#if showBadge}
                    <span class="new-count-badge">New</span>
                {/if}
                <span class="thread-chevron" class:open={expanded}>
                    <svg
                        aria-hidden="true"
                        width="12"
                        height="12"
                        viewBox="0 0 16 16"
                        fill="currentColor"
                    >
                        <path
                            d="M12.78 5.22a.749.749 0 0 1 0 1.06l-4.25 4.25a.749.749 0 0 1-1.06 0L3.22 6.28a.749.749 0 1 1 1.06-1.06L8 8.939l3.72-3.719a.749.749 0 0 1 1.06 0Z"
                        />
                    </svg>
                </span>
            </Collapsible.Trigger>
            <Collapsible.Content>
                {#if review.body && expanded}
                    <a
                        class="review-comment"
                        class:review-comment--new={showBadge}
                        href={review.html_url}
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        <div class="comment-header">
                            <img
                                class="comment-avatar"
                                src={avatarUrl(review.reviewer, review.reviewer_avatar_url)}
                                alt={review.reviewer}
                                width="18"
                                height="18"
                            >
                            <span class="comment-author"
                                >{review.reviewer}</span
                            >
                            <span class="comment-date"
                                >·
                                {timeAgo(review.submitted_at)}</span
                            >
                            <span class="comment-link-icon" aria-hidden="true"
                                >↗</span
                            >
                        </div>
                        <div class="comment-body">
                            <p>{review.body}</p>
                        </div>
                    </a>
                {/if}
            </Collapsible.Content>
        </Collapsible.Root>
    {:else}
        <a
            class="review-thread-header review-thread-header--link"
            href={review.html_url}
            target="_blank"
            rel="noopener noreferrer"
        >
            <img
                class="avatar avatar-sm"
                src={avatarUrl(review.reviewer, review.reviewer_avatar_url)}
                alt={review.reviewer}
                width="18"
                height="18"
            >
            <span class="reviewer-name">{review.reviewer}</span>
            <span
                class="review-state-pill {review.state === 'APPROVED' ? 'pill-approved' : review.state === 'CHANGES_REQUESTED' ? 'pill-changes' : 'pill-dismissed'}"
            >
                {review.state === 'APPROVED' ? 'Approved' : review.state === 'CHANGES_REQUESTED' ? 'Changes requested' : 'Dismissed'}
            </span>
            <span class="timestamp">{timeAgo(review.submitted_at)}</span>
            {#if showBadge}
                <span class="new-count-badge">New</span>
            {/if}
            <span class="review-link-icon" aria-hidden="true">↗</span>
        </a>
    {/if}
</div>

<style>
/* Shared with PrDetail's description-item — Svelte scopes styles per
   component, so the rule is duplicated here for the review card. */
.timeline-item {
    border-radius: 6px;
    border: 1px solid var(--border-default);
}

.review-item {
    font-size: 12px;
}

:global(.review-thread-header) {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 10px;
    background: var(--canvas-subtle);
    font-size: 12px;
    color: var(--fg-muted);
    width: 100%;
    text-align: left;
    cursor: pointer;
    font-family: inherit;
    border: none;
    border-radius: 0;
    text-decoration: none;
}

:global(.review-thread-header:hover) {
    background: var(--canvas-inset, var(--canvas-subtle));
    color: var(--fg-default);
}

.review-thread-header--link {
    cursor: default;
}

.avatar-sm {
    border-radius: 50%;
    flex-shrink: 0;
}

.reviewer-name {
    font-weight: 600;
    color: var(--fg-default);
}

.review-state-pill {
    border-radius: 2em;
    padding: 1px 7px;
    font-size: 11px;
    font-weight: 600;
    flex-shrink: 0;
}

.pill-approved {
    background: rgba(46, 160, 67, 0.15);
    color: var(--success-fg, #1a7f37);
    border: 1px solid rgba(46, 160, 67, 0.3);
}

.pill-changes {
    background: rgba(248, 81, 73, 0.1);
    color: var(--danger-fg);
    border: 1px solid rgba(248, 81, 73, 0.25);
}

.pill-dismissed {
    background: var(--canvas-subtle);
    color: var(--fg-muted);
    border: 1px solid var(--border-muted);
    text-decoration: line-through;
}

.timestamp {
    font-size: 11px;
    color: var(--fg-subtle);
    flex-shrink: 0;
}

.new-count-badge {
    font-size: 10px;
    font-weight: 600;
    color: var(--accent-fg);
    background: rgba(47, 129, 247, 0.15);
    border: 1px solid rgba(47, 129, 247, 0.4);
    border-radius: 2em;
    padding: 0 6px;
    line-height: 18px;
    flex-shrink: 0;
    margin-left: auto;
}

/* Shared with PrDetail's description chevron — duplicated for the same
   style-scoping reason as .timeline-item. */
.thread-chevron {
    flex-shrink: 0;
    transition: transform 0.15s;
    color: var(--fg-muted);
    margin-left: auto;
}

.thread-chevron.open {
    transform: rotate(180deg);
}

.new-count-badge + .thread-chevron {
    margin-left: 0;
}

.review-link-icon {
    margin-left: auto;
    font-size: 11px;
    color: var(--fg-muted);
    opacity: 0;
    transition: opacity 0.1s;
}

:global(.review-thread-header:hover) .review-link-icon {
    opacity: 1;
}

.review-comment {
    display: block;
    padding: 9px 10px;
    border-top: 1px solid var(--border-muted);
    text-decoration: none;
    color: inherit;
    cursor: pointer;
}

.review-comment:hover {
    background: var(--canvas-subtle);
}

.review-comment--new {
    background: rgba(47, 129, 247, 0.04);
    border-left: 3px solid var(--accent-fg);
}

.review-comment--new:hover {
    background: rgba(47, 129, 247, 0.09);
}

.comment-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 5px;
}

.comment-avatar {
    border-radius: 50%;
    flex-shrink: 0;
}

.comment-author {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-default);
}

.comment-date {
    font-size: 11px;
    color: var(--fg-subtle);
}

.comment-link-icon {
    margin-left: auto;
    font-size: 11px;
    color: var(--fg-muted);
    opacity: 0;
    transition: opacity 0.1s;
}

.review-comment:hover .comment-link-icon {
    opacity: 1;
}

.comment-body {
    padding-left: 24px;
    font-size: 13px;
}

.comment-body p {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--fg-default);
}
</style>
