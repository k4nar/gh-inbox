<script lang="ts">
import CommentThread from "./CommentThread.svelte";
import { timeAgo } from "./timeago.ts";
import type {
	CheckRun,
	Commit,
	Notification,
	PrDetailResponse,
	Thread,
} from "./types.ts";

let {
	notification,
	onClose,
}: {
	notification: Pick<Notification, "repository" | "pr_id" | "title">;
	onClose: () => void;
} = $props();

let detail = $state<PrDetailResponse | null>(null);
let threads: Thread[] = $state([]);
let loading = $state(true);
let error: string | null = $state(null);

$effect(() => {
	if (notification?.pr_id && notification?.repository) {
		loadDetail();
	}
});

async function loadDetail(): Promise<void> {
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
		error = e instanceof Error ? e.message : String(e);
	} finally {
		loading = false;
	}
}

function ciClass(status: string, conclusion: string | null): string {
	if (status !== "completed") return "ci-pending";
	if (conclusion === "success") return "ci-success";
	if (conclusion === "failure" || conclusion === "timed_out")
		return "ci-failure";
	return "ci-neutral";
}

function ciLabel(status: string, conclusion: string | null): string {
	if (status !== "completed") return "Running";
	return conclusion || "unknown";
}

function isPassing(cr: CheckRun): boolean {
	return (
		cr.status === "completed" &&
		(cr.conclusion === "success" ||
			cr.conclusion === "skipped" ||
			cr.conclusion === "neutral")
	);
}

let failedOrPending: CheckRun[] = $derived(
	detail != null ? detail.check_runs.filter((cr) => !isPassing(cr)) : [],
);
let passingChecks: CheckRun[] = $derived(
	detail != null ? detail.check_runs.filter((cr) => isPassing(cr)) : [],
);
let showPassing = $state(false);

function isNewCommit(commit: Commit): boolean {
	if (!detail?.pull_request?.last_viewed_at) return false;
	return commit.committed_at > detail.pull_request.last_viewed_at;
}
</script>

<div class="pr-detail">
  <div class="detail-header">
    <button class="back-btn" onclick={onClose} aria-label="Back to list">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M7.78 12.53a.75.75 0 0 1-1.06 0L2.47 8.28a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 1.06L4.81 7h7.44a.75.75 0 0 1 0 1.5H4.81l2.97 2.97a.75.75 0 0 1 0 1.06Z"/></svg>
    </button>
    <span class="detail-title">{notification.title}</span>
    {#if detail?.pull_request?.url}
      <a class="gh-link" href={detail.pull_request.url} target="_blank" rel="noopener" title="Open on GitHub">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor"><path d="M3.75 2h3.5a.75.75 0 0 1 0 1.5h-3.5a.25.25 0 0 0-.25.25v8.5c0 .138.112.25.25.25h8.5a.25.25 0 0 0 .25-.25v-3.5a.75.75 0 0 1 1.5 0v3.5A1.75 1.75 0 0 1 12.25 14h-8.5A1.75 1.75 0 0 1 2 12.25v-8.5C2 2.784 2.784 2 3.75 2Zm6.854-1h4.146a.25.25 0 0 1 .25.25v4.146a.25.25 0 0 1-.427.177L13.03 4.03 9.28 7.78a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042l3.75-3.75-1.543-1.543A.25.25 0 0 1 10.604 1Z"/></svg>
      </a>
    {/if}
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
          {#if failedOrPending.length === 0 && passingChecks.length > 0}
            <div class="ci-all-passing">
              <span class="ci-dot ci-success"></span>
              All checks passed
            </div>
          {:else}
            <div class="ci-list">
              {#each failedOrPending as cr}
                <div class="ci-item">
                  <span class="ci-dot {ciClass(cr.status, cr.conclusion)}"></span>
                  <span class="ci-name">{cr.name}</span>
                  <span class="ci-conclusion">{ciLabel(cr.status, cr.conclusion)}</span>
                </div>
              {/each}
            </div>
            {#if passingChecks.length > 0}
              <button class="ci-passing-toggle" onclick={() => showPassing = !showPassing}>
                <span class="ci-dot ci-success"></span>
                {passingChecks.length} passing
                <svg class="toggle-chevron" class:open={showPassing} width="12" height="12" viewBox="0 0 16 16" fill="currentColor"><path d="M12.78 5.22a.749.749 0 0 1 0 1.06l-4.25 4.25a.749.749 0 0 1-1.06 0L3.22 6.28a.749.749 0 1 1 1.06-1.06L8 8.939l3.72-3.719a.749.749 0 0 1 1.06 0Z"/></svg>
              </button>
              {#if showPassing}
                <div class="ci-list">
                  {#each passingChecks as cr}
                    <div class="ci-item">
                      <span class="ci-dot {ciClass(cr.status, cr.conclusion)}"></span>
                      <span class="ci-name">{cr.name}</span>
                      <span class="ci-conclusion">{ciLabel(cr.status, cr.conclusion)}</span>
                    </div>
                  {/each}
                </div>
              {/if}
            {/if}
          {/if}
        </div>
      {/if}

      {#if detail.commits && detail.commits.length > 0}
        <div class="commits-section">
          <h3 class="section-title">
            Commits
            <span class="comment-count">{detail.commits.length}</span>
          </h3>
          <div class="commits-list">
            {#each detail.commits as commit (commit.sha)}
              <div class="commit-item" class:new-commit={isNewCommit(commit)}>
                <span class="commit-sha">{commit.sha.slice(0, 7)}</span>
                <span class="commit-message">{commit.message}</span>
                <span class="commit-author">{commit.author}</span>
                <span class="commit-date">{timeAgo(commit.committed_at)}</span>
                {#if isNewCommit(commit)}
                  <span class="new-badge">new</span>
                {/if}
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

  .gh-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-muted);
    margin-left: auto;
    flex-shrink: 0;
    padding: 4px;
    border-radius: 6px;
  }
  .gh-link:hover {
    color: var(--fg-default);
    background: var(--canvas-subtle);
  }

  .ci-all-passing {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--success-fg);
    padding: 4px 0;
  }
  .ci-passing-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--fg-muted);
    background: none;
    border: none;
    cursor: pointer;
    padding: 6px 0;
    font-family: inherit;
  }
  .ci-passing-toggle:hover {
    color: var(--fg-default);
  }
  .toggle-chevron {
    transition: transform 0.15s;
  }
  .toggle-chevron.open {
    transform: rotate(180deg);
  }

  .commits-list {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    overflow: hidden;
  }
  .commit-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    font-size: 13px;
    border-bottom: 1px solid var(--border-muted);
  }
  .commit-item:last-child {
    border-bottom: none;
  }
  .new-commit {
    background: rgba(47, 129, 247, 0.05);
    border-left: 3px solid var(--accent-fg);
  }
  .commit-sha {
    font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    color: var(--accent-fg);
    flex-shrink: 0;
  }
  .commit-message {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--fg-default);
  }
  .commit-author {
    font-size: 12px;
    color: var(--fg-muted);
    flex-shrink: 0;
  }
  .commit-date {
    font-size: 12px;
    color: var(--fg-subtle);
    flex-shrink: 0;
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
    flex-shrink: 0;
  }
</style>
