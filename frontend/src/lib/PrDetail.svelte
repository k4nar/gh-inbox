<script lang="ts">
import "./markdown.css";
import { apiFetch } from "./api.ts";
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
let showCiTooltip = $state(false);

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
        detail = await apiFetch<PrDetailResponse>(
            `/api/pull-requests/${owner}/${repo}/${number}`,
        );
        threads = await apiFetch<Thread[]>(
            `/api/pull-requests/${owner}/${repo}/${number}/threads`,
        );
    } catch (e) {
        error = e instanceof Error ? e.message : String(e);
    } finally {
        loading = false;
    }
}

// --- Status bar derived values ---

function deriveStatePill(pr: PrDetailResponse["pull_request"]): {
    label: string;
    cls: string;
} {
    if (pr.merged_at) return { label: "Merged", cls: "pill-merged" };
    if (pr.state === "closed") return { label: "Closed", cls: "pill-closed" };
    if (pr.draft) return { label: "Draft", cls: "pill-draft" };
    return { label: "Open", cls: "pill-open" };
}

function avatarUrl(login: string): string {
    return `https://github.com/${login}.png?size=40`;
}

function commitUrl(repo: string, sha: string): string {
    return `https://github.com/${repo}/commit/${sha}`;
}

function isPassing(cr: CheckRun): boolean {
    return (
        cr.status === "completed" &&
        (cr.conclusion === "success" ||
            cr.conclusion === "skipped" ||
            cr.conclusion === "neutral")
    );
}

function ciDotClass(cr: CheckRun): string {
    if (cr.status !== "completed") return "ci-pending";
    if (
        cr.conclusion === "success" ||
        cr.conclusion === "skipped" ||
        cr.conclusion === "neutral"
    )
        return "ci-success";
    return "ci-failure";
}

function ciLabel(cr: CheckRun): string {
    if (cr.status !== "completed") return "running";
    return cr.conclusion ?? "unknown";
}

let ciSummary = $derived.by((): { text: string; cls: string } => {
    if (!detail || detail.check_runs.length === 0) return { text: "", cls: "" };
    const failing = detail.check_runs.filter(
        (cr) =>
            !isPassing(cr) &&
            cr.status === "completed" &&
            cr.conclusion !== null,
    );
    const pending = detail.check_runs.filter((cr) => cr.status !== "completed");
    if (failing.length > 0)
        return { text: `${failing.length} failing`, cls: "ci-failing" };
    if (pending.length > 0)
        return { text: `${pending.length} running`, cls: "ci-pending" };
    return { text: "CI passing", cls: "ci-passing" };
});

// --- Timeline derived values ---

let previousViewedAt = $derived(detail?.previous_viewed_at ?? null);

function isNew(timestamp: string): boolean {
    if (!previousViewedAt) return false;
    return timestamp > previousViewedAt;
}

let newCommits = $derived(
    detail?.commits.filter((c) => isNew(c.committed_at)) ?? [],
);
let oldCommits = $derived(
    detail?.commits.filter((c) => !isNew(c.committed_at)) ?? [],
);

let threadNewCounts = $derived(
    new Map(
        threads.map((t) => [
            t.thread_id,
            t.comments.filter((c) => isNew(c.created_at)).length,
        ]),
    ),
);

let newThreads = $derived(
    threads.filter((t) => (threadNewCounts.get(t.thread_id) ?? 0) > 0),
);
let oldThreads = $derived(
    threads.filter((t) => (threadNewCounts.get(t.thread_id) ?? 0) === 0),
);

let hasNewItems = $derived(
    previousViewedAt !== null &&
        (newCommits.length > 0 || newThreads.length > 0),
);

let ciActiveRuns = $derived(
    detail?.check_runs.filter(
        (cr) => cr.status !== "completed" || !isPassing(cr),
    ) ?? [],
);
let ciSucceededCount = $derived(
    detail?.check_runs.filter(
        (cr) => cr.status === "completed" && cr.conclusion === "success",
    ).length ?? 0,
);

let diffSinceBase = $derived(oldCommits[oldCommits.length - 1]?.sha ?? null);
let diffSinceUrl = $derived(
    detail && newCommits.length > 0
        ? diffSinceBase
            ? `${detail.pull_request.url}/files/${diffSinceBase}..${detail.pull_request.head_sha}`
            : `${detail.pull_request.url}/files`
        : null,
);
</script>

<div class="pr-detail">
    <!-- Header -->
    <div class="detail-header">
        <button
            type="button"
            class="back-btn"
            onclick={onClose}
            aria-label="Back to list"
        >
            <svg
                aria-hidden="true"
                width="16"
                height="16"
                viewBox="0 0 16 16"
                fill="currentColor"
            >
                <path
                    d="M7.78 12.53a.75.75 0 0 1-1.06 0L2.47 8.28a.75.75 0 0 1 0-1.06l4.25-4.25a.75.75 0 0 1 1.06 1.06L4.81 7h7.44a.75.75 0 0 1 0 1.5H4.81l2.97 2.97a.75.75 0 0 1 0 1.06Z"
                />
            </svg>
        </button>
        {#if detail?.pull_request?.url}
            <a
                class="detail-title detail-title-link"
                href={detail.pull_request.url}
                target="_blank"
                rel="noopener noreferrer"
                >{notification.title}</a
            >
        {:else}
            <span class="detail-title">{notification.title}</span>
        {/if}
        {#if detail?.pull_request?.url}
            <a
                class="gh-link"
                href={detail.pull_request.url}
                target="_blank"
                rel="noopener"
                title="Open on GitHub"
            >
                <svg
                    aria-hidden="true"
                    width="16"
                    height="16"
                    viewBox="0 0 16 16"
                    fill="currentColor"
                >
                    <path
                        d="M3.75 2h3.5a.75.75 0 0 1 0 1.5h-3.5a.25.25 0 0 0-.25.25v8.5c0 .138.112.25.25.25h8.5a.25.25 0 0 0 .25-.25v-3.5a.75.75 0 0 1 1.5 0v3.5A1.75 1.75 0 0 1 12.25 14h-8.5A1.75 1.75 0 0 1 2 12.25v-8.5C2 2.784 2.784 2 3.75 2Zm6.854-1h4.146a.25.25 0 0 1 .25.25v4.146a.25.25 0 0 1-.427.177L13.03 4.03 9.28 7.78a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042l3.75-3.75-1.543-1.543A.25.25 0 0 1 10.604 1Z"
                    />
                </svg>
            </a>
        {/if}
    </div>

    {#if loading}
        <div class="detail-loading">Loading...</div>
    {:else if error}
        <div class="detail-error">{error}</div>
    {:else if detail}
        {@const pr = detail.pull_request}
        {@const pill = deriveStatePill(pr)}

        <!-- Status bar -->
        <div class="status-bar">
            <span class="state-pill {pill.cls}">{pill.label}</span>
            <img
                class="status-avatar"
                src={avatarUrl(pr.author)}
                alt={pr.author}
                width="18"
                height="18"
            >
            <span class="status-author">{pr.author}</span>
            <span class="status-sep">·</span>
            <a
                class="diff-link"
                href="{detail.pull_request.url}/files"
                target="_blank"
                rel="noopener noreferrer"
                title="View diff on GitHub"
            >
                <span class="additions">+{pr.additions}</span>
                <span class="deletions">−{pr.deletions}</span>
                <span class="status-files">in {pr.changed_files} files</span>
            </a>

            {#if detail.check_runs.length > 0}
                <button
                    type="button"
                    class="ci-wrapper"
                    onmouseenter={() => (showCiTooltip = true)}
                    onmouseleave={() => (showCiTooltip = false)}
                    onfocus={() => (showCiTooltip = true)}
                    onblur={() => (showCiTooltip = false)}
                >
                    <span class="ci-indicator {ciSummary.cls}">
                        <span class="ci-dot-indicator"></span>
                        {ciSummary.text}
                    </span>
                    {#if showCiTooltip}
                        <div class="ci-tooltip">
                            <div class="ci-tooltip-title">CI Checks</div>
                            {#each ciActiveRuns as cr}
                                <div class="ci-tooltip-row">
                                    <span
                                        class="ci-dot {ciDotClass(cr)}"
                                    ></span>
                                    <span class="ci-tooltip-name"
                                        >{cr.name}</span
                                    >
                                    <span class="ci-tooltip-conclusion"
                                        >{ciLabel(cr)}</span
                                    >
                                </div>
                            {/each}
                            {#if ciSucceededCount > 0}
                                <div class="ci-tooltip-row ci-tooltip-summary">
                                    <span class="ci-dot ci-success"></span>
                                    <span class="ci-tooltip-name"
                                        >{ciSucceededCount}
                                        succeeded</span
                                    >
                                </div>
                            {/if}
                        </div>
                    {/if}
                </button>
            {/if}
        </div>

        <!-- Timeline -->
        <div class="timeline">
            {#if hasNewItems}
                <!-- "Since your last visit" zone -->
                <div class="divider divider-new">
                    <div class="divider-line divider-line-new"></div>
                    <span class="divider-label divider-label-new"
                        >Since your last visit</span
                    >
                    {#if diffSinceUrl}
                        <a
                            class="diff-since-link"
                            href={diffSinceUrl}
                            target="_blank"
                            rel="noopener noreferrer"
                            >View changes ↗</a
                        >
                    {/if}
                    <div class="divider-line divider-line-new"></div>
                </div>

                <div class="zone zone-new">
                    {#each newCommits as commit (commit.sha)}
                        <a
                            class="commit-row commit-row-new"
                            href={commitUrl(detail.pull_request.repo, commit.sha)}
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            <span class="commit-icon">⬆</span>
                            <span class="commit-sha"
                                >{commit.sha.slice(0, 7)}</span
                            >
                            <span class="commit-message">{commit.message}</span>
                            <span class="commit-date"
                                >{timeAgo(commit.committed_at)}</span
                            >
                        </a>
                    {/each}

                    {#each newThreads as thread (thread.thread_id)}
                        <CommentThread
                            {thread}
                            {previousViewedAt}
                            initiallyExpanded={(threadNewCounts.get(thread.thread_id) ?? 0) === thread.comments.length}
                        />
                    {/each}
                </div>

                {#if oldCommits.length > 0 || oldThreads.length > 0}
                    <!-- "Earlier" zone -->
                    <div class="divider divider-old">
                        <div class="divider-line divider-line-old"></div>
                        <span class="divider-label divider-label-old"
                            >Earlier</span
                        >
                        <div class="divider-line divider-line-old"></div>
                    </div>

                    <div class="zone zone-old">
                        {#each oldCommits as commit (commit.sha)}
                            <a
                                class="commit-row commit-row-old"
                                href={commitUrl(detail.pull_request.repo, commit.sha)}
                                target="_blank"
                                rel="noopener noreferrer"
                            >
                                <span class="commit-icon commit-icon-old"
                                    >⬆</span
                                >
                                <span class="commit-sha commit-sha-old"
                                    >{commit.sha.slice(0, 7)}</span
                                >
                                <span class="commit-message commit-message-old"
                                    >{commit.message}</span
                                >
                                <span class="commit-date"
                                    >{timeAgo(commit.committed_at)}</span
                                >
                            </a>
                        {/each}

                        {#each oldThreads as thread (thread.thread_id)}
                            <CommentThread {thread} {previousViewedAt} />
                        {/each}
                    </div>
                {/if}
            {:else}
                <!-- No dividers: first visit or nothing new -->
                <div class="zone">
                    {#each detail.commits as commit (commit.sha)}
                        <a
                            class="commit-row commit-row-old"
                            href={commitUrl(detail.pull_request.repo, commit.sha)}
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            <span class="commit-icon commit-icon-old">⬆</span>
                            <span class="commit-sha commit-sha-old"
                                >{commit.sha.slice(0, 7)}</span
                            >
                            <span class="commit-message commit-message-old"
                                >{commit.message}</span
                            >
                            <span class="commit-date"
                                >{timeAgo(commit.committed_at)}</span
                            >
                        </a>
                    {/each}
                    {#each threads as thread (thread.thread_id)}
                        <CommentThread {thread} {previousViewedAt} />
                    {/each}
                </div>
            {/if}
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
    padding: 10px 14px;
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
    font-size: 13px;
    font-weight: 600;
    color: var(--fg-default);
    text-decoration: none;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
}

.detail-title-link:hover {
    color: var(--accent-fg);
    text-decoration: underline;
}

.gh-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-muted);
    flex-shrink: 0;
    padding: 4px;
    border-radius: 6px;
}

.gh-link:hover {
    color: var(--fg-default);
    background: var(--canvas-subtle);
}

.detail-loading,
.detail-error {
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

/* Status bar */
.status-bar {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 14px;
    border-bottom: 1px solid var(--border-default);
    font-size: 12px;
    flex-shrink: 0;
    flex-wrap: wrap;
}

.state-pill {
    border-radius: 2em;
    padding: 1px 8px;
    font-size: 11px;
    font-weight: 600;
    flex-shrink: 0;
}

.pill-open {
    background: var(--color-success-emphasis, #1a7f37);
    color: #fff;
}
.pill-draft {
    background: var(--canvas-subtle);
    color: var(--fg-muted);
    border: 1px solid var(--border-default);
}
.pill-merged {
    background: var(--color-done-emphasis, #6e40c9);
    color: #fff;
}
.pill-closed {
    background: var(--canvas-subtle);
    color: var(--danger-fg);
    border: 1px solid var(--border-default);
}

.status-avatar {
    border-radius: 50%;
    flex-shrink: 0;
}

.status-author {
    color: var(--fg-muted);
}

.status-sep {
    color: var(--fg-subtle);
}

.additions {
    color: var(--success-fg);
    font-weight: 500;
}

.deletions {
    color: var(--danger-fg);
    font-weight: 500;
}

.status-files {
    color: var(--fg-muted);
}

.diff-link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    text-decoration: none;
    border-radius: 4px;
    padding: 1px 3px;
    margin: -1px -3px;
}

.diff-link:hover {
    background: var(--canvas-subtle);
    text-decoration: underline;
}

/* CI indicator */
.ci-wrapper {
    position: relative;
    margin-left: auto;
    background: none;
    border: none;
    padding: 0;
    font-family: inherit;
    cursor: default;
}

.ci-indicator {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    padding: 2px 7px;
    border-radius: 4px;
    cursor: default;
    white-space: nowrap;
}

.ci-passing {
    color: var(--success-fg);
    background: rgba(46, 160, 67, 0.1);
    border: 1px solid rgba(46, 160, 67, 0.25);
}

.ci-failing {
    color: var(--danger-fg);
    background: rgba(248, 81, 73, 0.1);
    border: 1px solid rgba(248, 81, 73, 0.25);
}

.ci-pending {
    color: var(--attention-fg);
    background: rgba(210, 153, 34, 0.1);
    border: 1px solid rgba(210, 153, 34, 0.25);
}

.ci-dot-indicator {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
}

.ci-passing .ci-dot-indicator {
    background: var(--success-fg);
}
.ci-failing .ci-dot-indicator {
    background: var(--danger-fg);
}
.ci-pending .ci-dot-indicator {
    background: var(--attention-fg);
}

.ci-tooltip {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    background: var(--canvas-overlay, var(--canvas-default));
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 8px 10px;
    min-width: 220px;
    z-index: 100;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
}

.ci-tooltip-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--fg-muted);
    text-transform: uppercase;
    letter-spacing: 0.4px;
    margin-bottom: 6px;
}

.ci-tooltip-row {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 3px 0;
    font-size: 12px;
}

.ci-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
}

.ci-success {
    background: var(--success-fg);
}
.ci-failure {
    background: var(--danger-fg);
}

.ci-tooltip-name {
    color: var(--fg-default);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.ci-tooltip-conclusion {
    color: var(--fg-muted);
    font-size: 11px;
    flex-shrink: 0;
}

.ci-tooltip-summary {
    border-top: 1px solid var(--border-muted);
    margin-top: 2px;
    padding-top: 4px;
    color: var(--fg-muted);
}

/* Timeline */
.timeline {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0;
}

.divider {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
}

.divider-line {
    flex: 1;
    height: 1px;
}

.divider-line-new {
    background: rgba(47, 129, 247, 0.3);
}
.divider-line-old {
    background: var(--border-muted);
}

.divider-label {
    font-size: 11px;
    white-space: nowrap;
    font-weight: 500;
}

.divider-label-new {
    color: var(--accent-fg);
}

.diff-since-link {
    font-size: 11px;
    white-space: nowrap;
    color: var(--accent-fg);
    text-decoration: none;
    opacity: 0.7;
}

.diff-since-link:hover {
    opacity: 1;
    text-decoration: underline;
}
.divider-label-old {
    color: var(--fg-subtle);
}

.zone {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 14px 10px;
}

/* Commit rows */
.commit-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 12px;
    border: 1px solid var(--border-default);
    text-decoration: none;
    color: inherit;
}

.commit-row:hover {
    background: var(--canvas-subtle);
    border-color: var(--border-muted);
}

.commit-row-new {
    background: rgba(47, 129, 247, 0.06);
    border-color: rgba(47, 129, 247, 0.2);
}

.commit-icon {
    color: var(--accent-fg);
    flex-shrink: 0;
    font-size: 13px;
}

.commit-icon-old {
    color: var(--fg-muted);
}

.commit-sha {
    font-family:
        ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    font-size: 11px;
    color: var(--accent-fg);
    flex-shrink: 0;
}

.commit-sha-old {
    color: var(--fg-muted);
}

.commit-message {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg-default);
}

.commit-message-old {
    color: var(--fg-muted);
}

.commit-date {
    font-size: 11px;
    color: var(--fg-subtle);
    flex-shrink: 0;
}
</style>
