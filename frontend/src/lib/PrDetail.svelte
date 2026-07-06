<script lang="ts">
import { avatarUrl } from "./avatar.ts";
import { Collapsible, Tooltip } from "bits-ui";
import { untrack } from "svelte";
import { SvelteSet } from "svelte/reactivity";
import { apiFetch } from "./api.ts";
import CiWheel from "./CiWheel.svelte";
import CommentThread from "./CommentThread.svelte";
import CommitItem from "./CommitItem.svelte";
import "./markdown.css";
import ReviewItem from "./ReviewItem.svelte";
import { onPrInfoUpdated } from "./sse.svelte.ts";
import {
    countNewCommentsPerThread,
    ciSummary as deriveCiSummary,
    isPassing,
    partitionCommits,
    partitionReviews,
    partitionThreads,
} from "./timeline.ts";
import type {
    CheckRun,
    Label,
    Notification,
    PrDetailResponse,
    Review,
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
let reviews = $state<Review[]>([]);
let labels = $state<Label[]>([]);
let loading = $state(true);
let error: string | null = $state(null);
$effect(() => {
    if (notification?.pr_id && notification?.repository) {
        // Switching PRs: drop the previous PR's content so the panel shows
        // "Loading..." instead of the wrong PR under the new title. SSE-driven
        // reloads for the *same* PR call loadDetail() directly and keep the
        // current content visible while the refresh happens in the background.
        detail = null;
        threads = [];
        reviews = [];
        labels = [];
        loadDetail();
    }
});

$effect(() => {
    const prId = notification?.pr_id;
    const repo = notification?.repository;
    return onPrInfoUpdated((data) => {
        if (data.pr_id !== prId || data.repository !== repo) return;
        const hasNewData =
            (data.new_commits !== null && data.new_commits > 0) ||
            (data.new_comments !== null && data.new_comments.length > 0) ||
            (data.new_reviews !== null && data.new_reviews.length > 0);
        if (hasNewData) {
            loadDetail();
        }
    });
});

// Monotonic id per loadDetail call: a slow response for a previously-selected
// PR must not overwrite the panel after a faster load for the current one.
let loadSeq = 0;

async function loadDetail(): Promise<void> {
    const seq = ++loadSeq;
    // Only blank the panel when there is nothing to show yet (first load or PR
    // switch). Background reloads keep the current timeline — and the reader's
    // scroll position — while fresh data arrives. Untracked: loadDetail runs
    // inside the prop-change effect, which also writes `detail` — a tracked
    // read here would make that effect re-trigger itself forever.
    const isInitialLoad = untrack(() => detail) === null;
    if (isInitialLoad) loading = true;
    error = null;

    const [owner, repo] = notification.repository.split("/");
    const number = notification.pr_id;

    try {
        const result = await apiFetch<PrDetailResponse>(
            `/api/pull-requests/${owner}/${repo}/${number}`,
        );
        if (seq !== loadSeq) return; // superseded by a newer load
        detail = result;
        reviews = result.reviews ?? [];
        labels = result.labels ?? [];
        threads = result.threads ?? [];
        // Description default is decided once per PR: expanded on first visit.
        // Background reloads must not reset a manual expand/collapse (each GET
        // advances last_viewed_at server-side, so previous_viewed_at changes).
        if (isInitialLoad) {
            expandedDescription = result.previous_viewed_at === null;
        }
    } catch (e) {
        if (seq !== loadSeq) return;
        error = e instanceof Error ? e.message : String(e);
    } finally {
        if (seq === loadSeq) loading = false;
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


function ciDotClass(cr: CheckRun): string {
    if (cr.status !== "completed") return "ci-pending";
    return isPassing(cr) ? "ci-success" : "ci-failure";
}

function ciLabel(cr: CheckRun): string {
    if (cr.status !== "completed") return "running";
    return cr.conclusion ?? "unknown";
}

let ciSummary = $derived(
    detail ? deriveCiSummary(detail.check_runs) : { text: "", cls: "" },
);

// --- Timeline derived values (pure derivations live in timeline.ts) ---

let previousViewedAt = $derived(detail?.previous_viewed_at ?? null);

let commitParts = $derived(
    partitionCommits(detail?.commits ?? [], previousViewedAt),
);
let newCommits = $derived(commitParts.newCommits);
let oldCommits = $derived(commitParts.oldCommits);

let threadNewCounts = $derived(
    countNewCommentsPerThread(threads, previousViewedAt),
);

let threadParts = $derived(partitionThreads(threads, threadNewCounts));
let newThreads = $derived(threadParts.newThreads);
let oldThreads = $derived(threadParts.oldThreads);

let reviewParts = $derived(partitionReviews(reviews, previousViewedAt));
let newReviews = $derived(reviewParts.newReviews);
let oldReviews = $derived(reviewParts.oldReviews);

// SvelteSet: plain Set mutations are invisible to Svelte's reactivity, so
// .add()/.delete() in onOpenChange would never re-render the review body.
// Kept here (not as local ReviewItem state) so an expanded review survives
// moving between the new/old zones when a background reload advances
// previous_viewed_at.
let expandedReviews = new SvelteSet<number>();

function setReviewExpanded(id: number, expanded: boolean): void {
    if (expanded) expandedReviews.add(id);
    else expandedReviews.delete(id);
}

// Description toggling logic: expand by default if PR hasn't been viewed.
// Set in loadDetail on each PR's initial load, so manual collapse/expand state
// neither carries over between PRs nor gets reset by background reloads.
let expandedDescription = $state(true);

function hasRenderableDescription(
    pr: PrDetailResponse["pull_request"],
): boolean {
    if (pr.body.trim().length > 0) return true;
    const htmlWithoutEmptyParagraphs = pr.body_html
        .replace(/<p>\s*<\/p>/gi, "")
        .trim();
    return htmlWithoutEmptyParagraphs.length > 0;
}

let hasNewItems = $derived(
    previousViewedAt !== null &&
        (newCommits.length > 0 ||
            newThreads.length > 0 ||
            newReviews.length > 0),
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
            <Tooltip.Root>
                <Tooltip.Trigger>
                    {#snippet child({ props })}
                        <a
                            {...props}
                            class="gh-link"
                            href={detail?.pull_request.url}
                            target="_blank"
                            rel="noopener"
                            aria-label="Open on GitHub"
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
                    {/snippet}
                </Tooltip.Trigger>
                <Tooltip.Portal>
                    <Tooltip.Content class="tooltip-content"
                        >Open on GitHub</Tooltip.Content
                    >
                </Tooltip.Portal>
            </Tooltip.Root>
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
            <div class="status-top">
                <span class="state-pill {pill.cls}">{pill.label}</span>
                <img
                    class="status-avatar"
                    src={avatarUrl(pr.author, pr.author_avatar_url)}
                    alt={pr.author}
                    width="18"
                    height="18"
                >
                <span class="status-author">{pr.author}</span>
                <span class="status-sep">·</span>
                <Tooltip.Root>
                    <Tooltip.Trigger>
                        {#snippet child({ props })}
                            <a
                                {...props}
                                class="diff-link"
                                href="{detail?.pull_request.url}/files"
                                target="_blank"
                                rel="noopener noreferrer"
                                aria-label="View diff on GitHub"
                            >
                                <span class="additions">+{pr.additions}</span>
                                <span class="deletions">−{pr.deletions}</span>
                                <span class="status-files"
                                    >in {pr.changed_files} files</span
                                >
                            </a>
                        {/snippet}
                    </Tooltip.Trigger>
                    <Tooltip.Portal>
                        <Tooltip.Content class="tooltip-content"
                            >View diff on GitHub</Tooltip.Content
                        >
                    </Tooltip.Portal>
                </Tooltip.Root>
                <div class="status-right">
                    {#if labels.length > 0}
                        <Tooltip.Root>
                            <Tooltip.Trigger
                                class="labels-wrapper"
                                type="button"
                            >
                                <span class="labels-pill">
                                    <svg
                                        aria-hidden="true"
                                        width="11"
                                        height="11"
                                        viewBox="0 0 16 16"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M1 7.775V2.75C1 1.784 1.784 1 2.75 1h5.025c.464 0 .91.184 1.238.513l6.25 6.25a1.75 1.75 0 0 1 0 2.474l-5.026 5.026a1.75 1.75 0 0 1-2.474 0l-6.25-6.25A1.752 1.752 0 0 1 1 7.775Zm1.5 0c0 .066.026.13.073.177l6.25 6.25a.25.25 0 0 0 .354 0l5.025-5.025a.25.25 0 0 0 0-.354l-6.25-6.25a.25.25 0 0 0-.177-.073H2.75a.25.25 0 0 0-.25.25ZM6 5a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z"
                                        />
                                    </svg>
                                    {labels.length}
                                </span>
                            </Tooltip.Trigger>
                            <Tooltip.Portal>
                                <Tooltip.Content class="tooltip-content">
                                    {#each labels as label}
                                        <span
                                            class="label-chip"
                                            style="color: #{label.color}; border-color: #{label.color}; background: #{label.color}1a"
                                            >{label.name}</span
                                        >
                                    {/each}
                                </Tooltip.Content>
                            </Tooltip.Portal>
                        </Tooltip.Root>
                    {/if}
                    {#if detail.check_runs.length > 0}
                        <Tooltip.Root>
                            <Tooltip.Trigger class="ci-wrapper" type="button">
                                <CiWheel checkRuns={detail.check_runs} />
                            </Tooltip.Trigger>
                            <Tooltip.Portal>
                                <Tooltip.Content class="tooltip-content">
                                    <div class="ci-tooltip-title">
                                        CI Checks
                                    </div>
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
                                        <div
                                            class="ci-tooltip-row ci-tooltip-summary"
                                        >
                                            <span
                                                class="ci-dot ci-success"
                                            ></span>
                                            <span class="ci-tooltip-name"
                                                >{ciSucceededCount}
                                                succeeded</span
                                            >
                                        </div>
                                    {/if}
                                </Tooltip.Content>
                            </Tooltip.Portal>
                        </Tooltip.Root>
                    {/if}
                </div>
            </div>
        </div>

        <!-- Timeline -->

        <div class="timeline">
            <div class="timeline-item description-item">
                <Collapsible.Root
                    open={expandedDescription}
                    onOpenChange={(v) => (expandedDescription = v)}
                >
                    <Collapsible.Trigger
                        class="description-header"
                        type="button"
                    >
                        <span class="description-title">Description</span>
                        <span
                            class="thread-chevron"
                            class:open={expandedDescription}
                        >
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
                    <Collapsible.Content hiddenUntilFound>
                        <div
                            class="description-content"
                            class:description-content--new={!previousViewedAt}
                        >
                            {#if hasRenderableDescription(pr)}
                                <div class="markdown-body">
                                    {@html pr.body_html}
                                </div>
                            {:else}
                                <p class="description-empty">
                                    No description provided.
                                </p>
                            {/if}
                        </div>
                    </Collapsible.Content>
                </Collapsible.Root>
            </div>
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
                        <CommitItem
                            {commit}
                            repo={detail.pull_request.repo}
                            isNew={true}
                        />
                    {/each}

                    {#each newReviews as review (review.id)}
                        <ReviewItem
                            {review}
                            showBadge={true}
                            expanded={expandedReviews.has(review.id)}
                            onExpandedChange={(v) => setReviewExpanded(review.id, v)}
                        />
                    {/each}

                    {#each newThreads as thread (thread.thread_id)}
                        <CommentThread
                            {thread}
                            {previousViewedAt}
                            initiallyExpanded={(threadNewCounts.get(thread.thread_id) ?? 0) === thread.comments.length}
                        />
                    {/each}
                </div>

                {#if oldCommits.length > 0 || oldThreads.length > 0 || oldReviews.length > 0}
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
                            <CommitItem
                                {commit}
                                repo={detail.pull_request.repo}
                                isNew={false}
                            />
                        {/each}

                        {#each oldReviews as review (review.id)}
                            <ReviewItem
                                {review}
                                showBadge={false}
                                expanded={expandedReviews.has(review.id)}
                                onExpandedChange={(v) => setReviewExpanded(review.id, v)}
                            />
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
                        <CommitItem
                            {commit}
                            repo={detail.pull_request.repo}
                            isNew={false}
                        />
                    {/each}
                    {#each reviews as review (review.id)}
                        <ReviewItem
                            {review}
                            showBadge={false}
                            expanded={expandedReviews.has(review.id)}
                            onExpandedChange={(v) => setReviewExpanded(review.id, v)}
                        />
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
    min-width: 0;
    width: 100%;
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
}

.status-top {
    display: contents;
}

.status-right {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
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
:global(.ci-wrapper) {
    position: relative;
    background: none;
    border: none;
    padding: 0;
    font-family: inherit;
    cursor: default;
    display: inline-flex;
    align-items: center;
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

/* Labels pill */
:global(.labels-wrapper) {
    position: relative;
    background: none;
    border: none;
    padding: 0;
    font-family: inherit;
    cursor: default;
}

.labels-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    padding: 2px 7px;
    border-radius: 4px;
    color: var(--fg-muted);
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    cursor: default;
}

/* Label chips */
.label-chip {
    display: inline-block;
    padding: 0 7px;
    border-radius: 2em;
    border: 1px solid;
    font-size: 11px;
    font-weight: 500;
    line-height: 18px;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

/* Description section */
.timeline-item {
    border-radius: 6px;
    border: 1px solid var(--border-default);
}

.thread-chevron {
    flex-shrink: 0;
    transition: transform 0.15s;
    color: var(--fg-muted);
    margin-left: auto;
}

.thread-chevron.open {
    transform: rotate(180deg);
}

.description-item {
    margin-bottom: 10px;
}

:global(.description-header) {
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
    border-radius: 6px 6px 0 0;
    text-decoration: none;
}

:global(.description-header:hover) {
    background: var(--canvas-inset, var(--canvas-subtle));
    color: var(--fg-default);
}

.description-title {
    font-weight: 600;
    color: var(--fg-default);
}

.description-content {
    padding: 16px;
    border: 1px solid var(--border-default);
    border-top: none;
    border-radius: 0 0 6px 6px;
    background: var(--canvas-default);
}

.description-content--new {
    background: rgba(47, 129, 247, 0.02);
    border-left: 3px solid var(--accent-fg);
    border-top: 1px solid var(--border-default);
    padding-left: 7px;
}

.description-content .markdown-body {
    padding-left: 0;
}

.description-empty {
    margin: 8px 0 0;
    color: var(--fg-muted);
    font-size: 12px;
}
</style>
