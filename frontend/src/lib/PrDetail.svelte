<script>
import { onMount } from "svelte";
import CommentThread from "./CommentThread.svelte";
import { timeAgo } from "./timeago.js";

/** @type {{ notification: { repository: string, pr_id: number|null, title: string }, onClose: () => void }} */
let { notification, onClose } = $props();

let detail = $state(null);
let threads = $state([]);
let loading = $state(true);
let error = $state(null);

$effect(() => {
	if (notification?.pr_id && notification?.repository) {
		loadDetail();
	}
});

async function loadDetail() {
	loading = true;
	error = null;

	const [owner, repo] = notification.repository.split("/");
	const number = notification.pr_id;

	try {
		const detailRes = await fetch(
			`/api/pull-requests/${owner}/${repo}/${number}`,
		);
		if (!detailRes.ok) {
			throw new Error(`Failed to load PR: ${detailRes.status}`);
		}
		detail = await detailRes.json();

		const tRes = await fetch(
			`/api/pull-requests/${owner}/${repo}/${number}/threads`,
		);
		if (tRes.ok) {
			threads = await tRes.json();
		}
	} catch (e) {
		error = e.message;
	} finally {
		loading = false;
	}
}

function ciClass(status, conclusion) {
	if (status !== "completed") return "ci-pending";
	if (conclusion === "success") return "ci-success";
	if (conclusion === "failure" || conclusion === "timed_out")
		return "ci-failure";
	return "ci-neutral";
}

function ciLabel(status, conclusion) {
	if (status !== "completed") return "Running";
	return conclusion || "unknown";
}
</script>

<div class="pr-detail">
  <div class="detail-header">
    <button class="back-btn" onclick={onClose} aria-label="Back to list">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M7.78 12.53a.75.75 0 0 1-1.06 0L2.47 8.28a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 1.06L4.81 7h7.44a.75.75 0 0 1 0 1.5H4.81l2.97 2.97a.75.75 0 0 1 0 1.06Z"/></svg>
    </button>
    <span class="detail-title">{notification.title}</span>
  </div>

  {#if loading}
    <div class="detail-loading">Loading...</div>
  {:else if error}
    <div class="detail-error">{error}</div>
  {:else if detail}
    <div class="detail-content">
      <div class="pr-meta">
        <div class="pr-meta-row">
          <span class="meta-label">Author</span>
          <span class="meta-value">{detail.pull_request.author}</span>
        </div>
        <div class="pr-meta-row">
          <span class="meta-label">State</span>
          <span class="meta-value state-{detail.pull_request.state}">{detail.pull_request.state}</span>
        </div>
        <div class="pr-meta-row">
          <span class="meta-label">Changes</span>
          <span class="meta-value">
            <span class="additions">+{detail.pull_request.additions}</span>
            <span class="deletions">-{detail.pull_request.deletions}</span>
            in {detail.pull_request.changed_files} files
          </span>
        </div>
      </div>

      {#if detail.check_runs.length > 0}
        <div class="ci-section">
          <h3 class="section-title">CI Status</h3>
          <div class="ci-list">
            {#each detail.check_runs as cr}
              <div class="ci-item">
                <span class="ci-dot {ciClass(cr.status, cr.conclusion)}"></span>
                <span class="ci-name">{cr.name}</span>
                <span class="ci-conclusion">{ciLabel(cr.status, cr.conclusion)}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      {#if detail.pull_request.body}
        <div class="pr-description">
          <h3 class="section-title">Description</h3>
          <div class="description-body">{detail.pull_request.body}</div>
        </div>
      {/if}

      <div class="threads-section">
        <h3 class="section-title">
          Comments
          {#if detail.comments.length > 0}
            <span class="comment-count">{detail.comments.length}</span>
          {/if}
        </h3>
        {#if threads.length === 0}
          <div class="no-comments">No comments yet.</div>
        {:else}
          <div class="threads-list">
            {#each threads as thread (thread.thread_id)}
              <CommentThread {thread} lastViewedAt={detail.pull_request.last_viewed_at} />
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .pr-detail {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-left: 1px solid var(--border-default);
    min-width: 400px;
    max-width: 600px;
  }
  .detail-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-default);
    flex-shrink: 0;
  }
  .back-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    color: var(--fg-muted);
    cursor: pointer;
    padding: 4px;
  }
  .back-btn:hover {
    color: var(--fg-default);
    background: var(--canvas-subtle);
  }
  .detail-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--fg-default);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail-loading, .detail-error, .no-comments {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px;
    color: var(--fg-muted);
    font-size: 14px;
  }
  .detail-error {
    color: var(--danger-fg);
  }

  .detail-content {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .pr-meta {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    background: var(--canvas-subtle);
    border-radius: 6px;
    border: 1px solid var(--border-default);
  }
  .pr-meta-row {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
  }
  .meta-label {
    width: 80px;
    color: var(--fg-muted);
    font-weight: 500;
    flex-shrink: 0;
  }
  .meta-value {
    color: var(--fg-default);
  }
  .state-open { color: var(--success-fg); }
  .state-closed { color: var(--danger-fg); }
  .state-merged { color: var(--done-fg); }
  .additions { color: var(--success-fg); }
  .deletions { color: var(--danger-fg); }

  .section-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--fg-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 8px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .comment-count {
    font-size: 11px;
    font-weight: 500;
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 2em;
    padding: 0 6px;
    line-height: 18px;
  }

  .ci-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .ci-item {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    padding: 4px 0;
  }
  .ci-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .ci-success { background: var(--success-fg); }
  .ci-failure { background: var(--danger-fg); }
  .ci-pending { background: var(--attention-fg); }
  .ci-neutral { background: var(--fg-muted); }
  .ci-name { color: var(--fg-default); }
  .ci-conclusion {
    color: var(--fg-muted);
    font-size: 12px;
  }

  .pr-description {
    display: flex;
    flex-direction: column;
  }
  .description-body {
    font-size: 13px;
    color: var(--fg-default);
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    padding: 12px;
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 6px;
  }

  .threads-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
