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

let newComments = $derived(thread.comments.filter(isNew));
let oldComments = $derived(thread.comments.filter((c) => !isNew(c)));
let newCount = $derived(newComments.length);
let hasNewComments = $derived(newCount > 0);
let firstComment = $derived(thread.comments[0] ?? null);
let lastComment = $derived(thread.comments[thread.comments.length - 1] ?? null);
let diffHunk = $derived(firstComment?.diff_hunk ?? null);
let lastOldComment = $derived(oldComments[oldComments.length - 1] ?? null);
let lastNewComment = $derived(newComments[newComments.length - 1] ?? null);

function parseDiffLines(
    hunk: string,
): { type: "header" | "add" | "del" | "ctx"; text: string }[] {
    const all = hunk.split("\n").map((line) => {
        if (line.startsWith("@@"))
            return { type: "header" as const, text: line };
        if (line.startsWith("+")) return { type: "add" as const, text: line };
        if (line.startsWith("-")) return { type: "del" as const, text: line };
        return { type: "ctx" as const, text: line };
    });
    const body = all.filter((l) => l.type !== "header");
    return body.slice(-4);
}

let diffLines = $derived(diffHunk ? parseDiffLines(diffHunk) : []);

function avatarUrl(login: string): string {
    return `https://github.com/${login}.png?size=40`;
}

function firstLine(text: string): string {
    const line = text.split("\n")[0];
    return line.length > 120 ? line.slice(0, 120) + "…" : line;
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
            {#if firstComment?.html_url}
                <a
                    class="thread-path thread-path-link"
                    href={firstComment.html_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    onclick={(e) => e.stopPropagation()}
                    >{thread.path}</a
                >
            {:else}
                <span class="thread-path">{thread.path}</span>
            {/if}
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

        {#if !expanded}
            {#if newCount > 0}
                <span class="new-count-badge">{newCount} new</span>
            {:else}
                <span class="comment-count"
                    >{thread.comments.length}
                    comments</span
                >
            {/if}
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

    <!-- Diff hunk context (inline review threads only, always visible) -->
    {#if diffLines.length > 0}
        <div class="diff-hunk" aria-label="Code context">
            {#each diffLines as line}
                <div class="diff-line diff-line--{line.type}">{line.text}</div>
            {/each}
        </div>
    {/if}

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
    {:else if hasNewComments && lastNewComment}
        <!-- Has new comments: one-liner for old + last new comment expanded -->
        {#if firstComment}
            <button
                type="button"
                class="thread-preview"
                onclick={() => (expanded = true)}
            >
                <div class="comment-preview">
                    <img
                        class="preview-avatar"
                        src={avatarUrl(firstComment.author)}
                        alt={firstComment.author}
                        width="16"
                        height="16"
                    >
                    <span class="preview-author">{firstComment.author}:</span>
                    <span class="preview-text"
                        >{firstLine(firstComment.body)}</span
                    >
                </div>
                {#if lastOldComment && lastOldComment.id !== firstComment.id}
                    <div class="comment-preview">
                        <img
                            class="preview-avatar"
                            src={avatarUrl(lastOldComment.author)}
                            alt={lastOldComment.author}
                            width="16"
                            height="16"
                        >
                        <span class="preview-author"
                            >{lastOldComment.author}:</span
                        >
                        <span class="preview-text"
                            >{firstLine(lastOldComment.body)}</span
                        >
                    </div>
                {/if}
            </button>
        {/if}
        <div class="thread-comments">
            <a
                class="comment new-comment"
                href={lastNewComment.html_url ?? "#"}
                target="_blank"
                rel="noopener noreferrer"
            >
                <div class="comment-header">
                    <img
                        class="comment-avatar"
                        src={avatarUrl(lastNewComment.author)}
                        alt={lastNewComment.author}
                        width="18"
                        height="18"
                    >
                    <span class="comment-author">{lastNewComment.author}</span>
                    <span class="comment-date"
                        >· {timeAgo(lastNewComment.created_at)}</span
                    >
                    <span class="comment-link-icon" aria-hidden="true">↗</span>
                </div>
                <div class="comment-body markdown-body">
                    {@html lastNewComment.body_html}
                </div>
            </a>
        </div>
    {:else if firstComment}
        <!-- No new comments, collapsed: two-line preview, click to expand -->
        <button
            type="button"
            class="thread-preview"
            onclick={() => (expanded = true)}
        >
            <div class="comment-preview">
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
                <div class="comment-preview">
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
        </button>
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
    font-size: 12px;
    color: var(--fg-muted);
    width: 100%;
    text-align: left;
    cursor: pointer;
    font-family: inherit;
    border: none;
    border-bottom: 1px solid var(--border-default);
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
    color: inherit;
    text-decoration: none;
}

.thread-path-link:hover {
    text-decoration: underline;
    color: var(--accent-fg);
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

/* Diff hunk */
.diff-hunk {
    font-family:
        ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    font-size: 11px;
    line-height: 1.5;
    overflow-x: auto;
    background: var(--canvas-default);
    border-bottom: 1px solid var(--border-default);
}

.diff-line {
    padding: 0 10px;
    white-space: pre;
}

.diff-line--header {
    background: rgba(47, 129, 247, 0.08);
    color: var(--accent-fg);
}

.diff-line--add {
    background: rgba(46, 160, 67, 0.1);
    color: var(--color-success-fg, #3fb950);
}

.diff-line--del {
    background: rgba(248, 81, 73, 0.1);
    color: var(--color-danger-fg, #f85149);
}

.diff-line--ctx {
    color: var(--fg-muted);
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
    width: 100%;
    background: none;
    border: none;
    border-radius: 0;
    padding: 0;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
}

.comment-count {
    font-size: 11px;
    color: var(--fg-muted);
    flex-shrink: 0;
    margin-left: auto;
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

.thread-preview:hover .comment-preview {
    background: var(--canvas-subtle);
}

.preview-avatar {
    border-radius: 50%;
    flex-shrink: 0;
    opacity: 0.7;
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
</style>
