<script lang="ts">
import "./markdown.css";
import { timeAgo } from "./timeago.ts";
import type { Comment, Thread } from "./types.ts";

let {
    thread,
    previousViewedAt = null,
    initiallyExpanded = false,
}: {
    thread: Thread;
    previousViewedAt?: string | null;
    initiallyExpanded?: boolean;
} = $props();

// eslint-disable-next-line svelte/no-unused-svelte-ignore
// svelte-ignore state_referenced_locally
let expanded = $state(initiallyExpanded);

function isNew(comment: Comment): boolean {
    if (!previousViewedAt) return false;
    return comment.created_at > previousViewedAt;
}

let newCount = $derived(thread.comments.filter(isNew).length);
let firstComment = $derived(thread.comments[0] ?? null);
let lastComment = $derived(thread.comments[thread.comments.length - 1] ?? null);

function avatarUrl(login: string): string {
    return `https://github.com/${login}.png?size=40`;
}

function firstLine(text: string): string {
    return text.split("\n")[0].slice(0, 120);
}
</script>

<div class="thread">
    <!-- Thread header — always visible, click to toggle -->
    <button
        type="button"
        class="thread-header"
        onclick={() => (expanded = !expanded)}
        aria-expanded={expanded}
    >
        {#if thread.path}
            <svg
                aria-hidden="true"
                width="14"
                height="14"
                viewBox="0 0 16 16"
                fill="currentColor"
            >
                <path
                    d="M2 1.75C2 .784 2.784 0 3.75 0h6.586c.464 0 .909.184 1.237.513l2.914 2.914c.329.328.513.773.513 1.237v9.586A1.75 1.75 0 0 1 13.25 16h-9.5A1.75 1.75 0 0 1 2 14.25Zm1.75-.25a.25.25 0 0 0-.25.25v12.5c0 .138.112.25.25.25h9.5a.25.25 0 0 0 .25-.25V6h-2.75A1.75 1.75 0 0 1 9 4.25V1.5Zm6.75.062V4.25c0 .138.112.25.25.25h2.688l-.011-.013-2.914-2.914-.013-.011Z"
                />
            </svg>
            <span class="thread-path">{thread.path}</span>
        {:else}
            <svg
                aria-hidden="true"
                width="14"
                height="14"
                viewBox="0 0 16 16"
                fill="currentColor"
            >
                <path
                    d="M1.75 1h8.5c.966 0 1.75.784 1.75 1.75v5.5A1.75 1.75 0 0 1 10.25 10H7.061l-2.574 2.573A1.458 1.458 0 0 1 2 11.543V10h-.25A1.75 1.75 0 0 1 0 8.25v-5.5C0 1.784.784 1 1.75 1ZM1.5 2.75v5.5c0 .138.112.25.25.25h1a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h3.5a.25.25 0 0 0 .25-.25v-5.5a.25.25 0 0 0-.25-.25h-8.5a.25.25 0 0 0-.25.25Z"
                />
            </svg>
            <span class="thread-path">Conversation</span>
        {/if}

        {#if newCount > 0 && !expanded}
            <span class="new-count-badge">{newCount} new</span>
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
    </button>

    {#if expanded}
        <!-- Expanded: all comments as clickable links -->
        <div class="thread-comments">
            {#each thread.comments as comment (comment.id)}
                <a
                    class="comment"
                    class:new-comment={isNew(comment)}
                    href={comment.html_url ?? "#"}
                    target="_blank"
                    rel="noopener noreferrer"
                >
                    <div class="comment-header">
                        <img
                            class="comment-avatar"
                            src={avatarUrl(comment.author)}
                            alt={comment.author}
                            width="18"
                            height="18"
                        >
                        <span class="comment-author">{comment.author}</span>
                        <span class="comment-date"
                            >· {timeAgo(comment.created_at)}</span
                        >
                        <span class="comment-link-icon" aria-hidden="true"
                            >↗</span
                        >
                    </div>
                    <div class="comment-body markdown-body">
                        {@html comment.body_html}
                    </div>
                </a>
            {/each}
        </div>
    {:else if firstComment}
        <!-- Collapsed: two-line preview -->
        <div class="thread-preview">
            <div
                class="comment-preview"
                class:new-comment-preview={isNew(firstComment)}
            >
                <img
                    class="preview-avatar"
                    src={avatarUrl(firstComment.author)}
                    alt={firstComment.author}
                    width="16"
                    height="16"
                >
                <span class="preview-author">{firstComment.author}:</span>
                <span class="preview-text">{firstLine(firstComment.body)}</span>
            </div>
            {#if lastComment && lastComment.id !== firstComment.id}
                <div
                    class="comment-preview"
                    class:new-comment-preview={isNew(lastComment)}
                >
                    <img
                        class="preview-avatar"
                        src={avatarUrl(lastComment.author)}
                        alt={lastComment.author}
                        width="16"
                        height="16"
                    >
                    <span class="preview-author">{lastComment.author}:</span>
                    <span class="preview-text"
                        >{firstLine(lastComment.body)}</span
                    >
                </div>
            {/if}
        </div>
    {/if}
</div>

<style>
.thread {
    border: 1px solid var(--border-default);
    border-radius: 6px;
    overflow: hidden;
}

.thread-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 10px;
    background: var(--canvas-subtle);
    border-bottom: 1px solid var(--border-default);
    font-size: 12px;
    color: var(--fg-muted);
    width: 100%;
    text-align: left;
    cursor: pointer;
    font-family: inherit;
    border: none;
    border-radius: 0;
}

.thread-header:hover {
    background: var(--canvas-inset, var(--canvas-subtle));
    color: var(--fg-default);
}

.thread-path {
    font-family:
        ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
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

.thread-chevron {
    flex-shrink: 0;
    transition: transform 0.15s;
    color: var(--fg-muted);
}

.thread-chevron.open {
    transform: rotate(180deg);
}

.thread-path + .thread-chevron {
    margin-left: auto;
}

/* Expanded: comments */
.thread-comments {
    display: flex;
    flex-direction: column;
}

.comment {
    display: block;
    padding: 9px 10px;
    border-bottom: 1px solid var(--border-muted);
    text-decoration: none;
    color: inherit;
    cursor: pointer;
}

.comment:last-child {
    border-bottom: none;
}

.comment:hover {
    background: var(--canvas-subtle);
}

.new-comment {
    background: rgba(47, 129, 247, 0.04);
    border-left: 3px solid var(--accent-fg);
}

.new-comment:hover {
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

.comment:hover .comment-link-icon {
    opacity: 1;
}

.comment-body {
    padding-left: 24px;
    font-size: 13px;
}

/* Collapsed: preview */
.thread-preview {
    display: flex;
    flex-direction: column;
}

.comment-preview {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-muted);
    font-size: 12px;
    overflow: hidden;
}

.comment-preview:last-child {
    border-bottom: none;
}

.new-comment-preview {
    background: rgba(47, 129, 247, 0.04);
    border-left: 3px solid var(--accent-fg);
}

.preview-avatar {
    border-radius: 50%;
    flex-shrink: 0;
    opacity: 0.7;
}

.new-comment-preview .preview-avatar {
    opacity: 1;
}

.preview-author {
    font-weight: 600;
    color: var(--fg-default);
    flex-shrink: 0;
}

.preview-text {
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
}

.new-comment-preview .preview-text {
    color: var(--fg-default);
}
</style>
