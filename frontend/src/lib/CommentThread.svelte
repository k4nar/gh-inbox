<script>
import { timeAgo } from "./timeago.js";

/** @type {{ thread: { thread_id: string, path: string|null, comments: any[] }, lastViewedAt: string|null }} */
let { thread, lastViewedAt = null } = $props();

function isNew(comment) {
	if (!lastViewedAt) return false;
	return comment.created_at > lastViewedAt;
}
</script>

<div class="thread">
  {#if thread.path}
    <div class="thread-header">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M2 1.75C2 .784 2.784 0 3.75 0h6.586c.464 0 .909.184 1.237.513l2.914 2.914c.329.328.513.773.513 1.237v9.586A1.75 1.75 0 0 1 13.25 16h-9.5A1.75 1.75 0 0 1 2 14.25Zm1.75-.25a.25.25 0 0 0-.25.25v12.5c0 .138.112.25.25.25h9.5a.25.25 0 0 0 .25-.25V6h-2.75A1.75 1.75 0 0 1 9 4.25V1.5Zm6.75.062V4.25c0 .138.112.25.25.25h2.688l-.011-.013-2.914-2.914-.013-.011Z"/></svg>
      <span class="thread-path">{thread.path}</span>
    </div>
  {:else}
    <div class="thread-header">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M1.75 1h8.5c.966 0 1.75.784 1.75 1.75v5.5A1.75 1.75 0 0 1 10.25 10H7.061l-2.574 2.573A1.458 1.458 0 0 1 2 11.543V10h-.25A1.75 1.75 0 0 1 0 8.25v-5.5C0 1.784.784 1 1.75 1ZM1.5 2.75v5.5c0 .138.112.25.25.25h1a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h3.5a.25.25 0 0 0 .25-.25v-5.5a.25.25 0 0 0-.25-.25h-8.5a.25.25 0 0 0-.25.25Z"/></svg>
      <span class="thread-path">Conversation</span>
    </div>
  {/if}

  <div class="thread-comments">
    {#each thread.comments as comment (comment.id)}
      <div class="comment" class:new-comment={isNew(comment)}>
        <div class="comment-header">
          <span class="comment-author">{comment.author}</span>
          <span class="comment-date">{timeAgo(comment.created_at)}</span>
          {#if isNew(comment)}
            <span class="new-badge">new</span>
          {/if}
        </div>
        <div class="comment-body">{comment.body}</div>
      </div>
    {/each}
  </div>
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
    padding: 8px 12px;
    background: var(--canvas-subtle);
    border-bottom: 1px solid var(--border-default);
    font-size: 12px;
    color: var(--fg-muted);
  }
  .thread-path {
    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    font-size: 12px;
  }
  .thread-comments {
    display: flex;
    flex-direction: column;
  }
  .comment {
    padding: 12px;
    border-bottom: 1px solid var(--border-muted);
  }
  .comment:last-child {
    border-bottom: none;
  }
  .new-comment {
    background: rgba(47, 129, 247, 0.05);
    border-left: 3px solid var(--accent-fg);
  }
  .comment-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .comment-author {
    font-size: 13px;
    font-weight: 600;
    color: var(--fg-default);
  }
  .comment-date {
    font-size: 12px;
    color: var(--fg-subtle);
  }
  .new-badge {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--accent-fg);
    background: rgba(47, 129, 247, 0.15);
    border: 1px solid rgba(47, 129, 247, 0.4);
    border-radius: 2em;
    padding: 0 6px;
    line-height: 18px;
  }
  .comment-body {
    font-size: 13px;
    color: var(--fg-default);
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
