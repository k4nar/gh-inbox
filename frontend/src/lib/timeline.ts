// Pure derivations for the PR detail timeline: what counts as "new" since the
// previous visit, how commits/threads/reviews split into new/old zones, and
// the CI summary. Kept free of Svelte reactivity so they can be unit-tested
// directly; PrDetail wraps them in thin `$derived` expressions.
import type { CheckRun, Commit, Review, Thread } from "./types.ts";

export function isNew(
    timestamp: string,
    previousViewedAt: string | null,
): boolean {
    if (!previousViewedAt) return true;
    return timestamp > previousViewedAt;
}

export function partitionCommits(
    commits: Commit[],
    previousViewedAt: string | null,
): { newCommits: Commit[]; oldCommits: Commit[] } {
    return {
        newCommits: commits.filter((c) =>
            isNew(c.committed_at, previousViewedAt),
        ),
        oldCommits: commits.filter(
            (c) => !isNew(c.committed_at, previousViewedAt),
        ),
    };
}

/** Number of comments newer than the previous visit, per thread_id. */
export function countNewCommentsPerThread(
    threads: Thread[],
    previousViewedAt: string | null,
): Map<string, number> {
    return new Map(
        threads.map((t) => [
            t.thread_id,
            t.comments.filter((c) => isNew(c.created_at, previousViewedAt))
                .length,
        ]),
    );
}

export function partitionThreads(
    threads: Thread[],
    newCounts: Map<string, number>,
): { newThreads: Thread[]; oldThreads: Thread[] } {
    return {
        newThreads: threads.filter(
            (t) => (newCounts.get(t.thread_id) ?? 0) > 0,
        ),
        oldThreads: threads.filter(
            (t) => (newCounts.get(t.thread_id) ?? 0) === 0,
        ),
    };
}

/** Sort by submission time, then split into new/old relative to the previous visit. */
export function partitionReviews(
    reviews: Review[],
    previousViewedAt: string | null,
): { newReviews: Review[]; oldReviews: Review[] } {
    const sorted = [...reviews].sort((a, b) =>
        a.submitted_at.localeCompare(b.submitted_at),
    );
    return {
        newReviews: sorted.filter((r) =>
            isNew(r.submitted_at, previousViewedAt),
        ),
        oldReviews: sorted.filter(
            (r) => !isNew(r.submitted_at, previousViewedAt),
        ),
    };
}

export function isPassing(cr: CheckRun): boolean {
    return (
        cr.status === "completed" &&
        (cr.conclusion === "success" ||
            cr.conclusion === "skipped" ||
            cr.conclusion === "neutral")
    );
}

export function ciSummary(checkRuns: CheckRun[]): {
    text: string;
    cls: string;
} {
    if (checkRuns.length === 0) return { text: "", cls: "" };
    // A completed run with an unknown (null) conclusion counts as failing,
    // never as passing — matching the backend's derive_ci_status.
    const failing = checkRuns.filter(
        (cr) => !isPassing(cr) && cr.status === "completed",
    );
    const pending = checkRuns.filter((cr) => cr.status !== "completed");
    if (failing.length > 0)
        return { text: `${failing.length} failing`, cls: "ci-failing" };
    if (pending.length > 0)
        return { text: `${pending.length} running`, cls: "ci-pending" };
    return { text: "CI passing", cls: "ci-passing" };
}
