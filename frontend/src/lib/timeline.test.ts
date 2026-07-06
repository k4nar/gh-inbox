import { describe, expect, it } from "vitest";
import {
    ciSummary,
    countNewCommentsPerThread,
    isNew,
    isPassing,
    partitionCommits,
    partitionReviews,
    partitionThreads,
} from "./timeline.ts";
import type { CheckRun, Commit, Review, Thread } from "./types.ts";

const VIEWED_AT = "2025-06-01T10:00:00Z";

function makeCommit(sha: string, committed_at: string): Commit {
    return {
        sha,
        repo: "owner/repo",
        pr_id: 42,
        message: `commit ${sha}`,
        author: "alice",
        committed_at,
    };
}

function makeThread(thread_id: string, commentDates: string[]): Thread {
    return {
        thread_id,
        path: null,
        resolved: false,
        comments: commentDates.map((created_at, i) => ({
            id: i + 1,
            repo: "owner/repo",
            pr_id: 42,
            thread_id,
            author: "bob",
            author_avatar_url: null,
            body: "hi",
            body_html: "<p>hi</p>",
            created_at,
            comment_type: "issue_comment",
            path: null,
            position: null,
            in_reply_to_id: null,
            html_url: null,
            diff_hunk: null,
            resolved: false,
        })),
    };
}

function makeReview(id: number, submitted_at: string): Review {
    return {
        id,
        reviewer: "carol",
        reviewer_avatar_url: null,
        state: "APPROVED",
        body: "",
        submitted_at,
        html_url: `https://github.com/owner/repo/pull/42#pullrequestreview-${id}`,
    };
}

describe("isNew", () => {
    it("treats everything as new on first visit (null previousViewedAt)", () => {
        expect(isNew("2025-06-01T09:00:00Z", null)).toBe(true);
    });

    it("is new when the timestamp is after the previous visit", () => {
        expect(isNew("2025-06-01T11:00:00Z", VIEWED_AT)).toBe(true);
    });

    it("is not new when the timestamp is before the previous visit", () => {
        expect(isNew("2025-06-01T09:00:00Z", VIEWED_AT)).toBe(false);
    });

    it("is not new when the timestamp equals the previous visit", () => {
        expect(isNew(VIEWED_AT, VIEWED_AT)).toBe(false);
    });
});

describe("partitionCommits", () => {
    it("splits commits around previousViewedAt, preserving order", () => {
        const commits = [
            makeCommit("aaa", "2025-06-01T08:00:00Z"),
            makeCommit("bbb", "2025-06-01T11:00:00Z"),
            makeCommit("ccc", "2025-06-01T12:00:00Z"),
        ];
        const { newCommits, oldCommits } = partitionCommits(commits, VIEWED_AT);
        expect(newCommits.map((c) => c.sha)).toEqual(["bbb", "ccc"]);
        expect(oldCommits.map((c) => c.sha)).toEqual(["aaa"]);
    });

    it("puts every commit in newCommits on first visit", () => {
        const commits = [makeCommit("aaa", "2025-06-01T08:00:00Z")];
        const { newCommits, oldCommits } = partitionCommits(commits, null);
        expect(newCommits).toHaveLength(1);
        expect(oldCommits).toHaveLength(0);
    });
});

describe("countNewCommentsPerThread", () => {
    it("counts only comments created after previousViewedAt", () => {
        const threads = [
            makeThread("t1", ["2025-06-01T09:00:00Z", "2025-06-01T11:00:00Z"]),
            makeThread("t2", ["2025-06-01T08:00:00Z"]),
        ];
        const counts = countNewCommentsPerThread(threads, VIEWED_AT);
        expect(counts.get("t1")).toBe(1);
        expect(counts.get("t2")).toBe(0);
    });

    it("counts all comments on first visit", () => {
        const threads = [
            makeThread("t1", ["2025-06-01T09:00:00Z", "2025-06-01T11:00:00Z"]),
        ];
        const counts = countNewCommentsPerThread(threads, null);
        expect(counts.get("t1")).toBe(2);
    });
});

describe("partitionThreads", () => {
    it("splits threads by whether they have new comments", () => {
        const threads = [
            makeThread("t1", ["2025-06-01T11:00:00Z"]),
            makeThread("t2", ["2025-06-01T08:00:00Z"]),
        ];
        const counts = countNewCommentsPerThread(threads, VIEWED_AT);
        const { newThreads, oldThreads } = partitionThreads(threads, counts);
        expect(newThreads.map((t) => t.thread_id)).toEqual(["t1"]);
        expect(oldThreads.map((t) => t.thread_id)).toEqual(["t2"]);
    });

    it("treats a thread missing from the counts map as old", () => {
        const threads = [makeThread("t1", ["2025-06-01T11:00:00Z"])];
        const { newThreads, oldThreads } = partitionThreads(threads, new Map());
        expect(newThreads).toHaveLength(0);
        expect(oldThreads).toHaveLength(1);
    });
});

describe("partitionReviews", () => {
    it("sorts by submitted_at then splits around previousViewedAt", () => {
        const reviews = [
            makeReview(3, "2025-06-01T12:00:00Z"),
            makeReview(1, "2025-06-01T08:00:00Z"),
            makeReview(2, "2025-06-01T11:00:00Z"),
        ];
        const { newReviews, oldReviews } = partitionReviews(reviews, VIEWED_AT);
        expect(newReviews.map((r) => r.id)).toEqual([2, 3]);
        expect(oldReviews.map((r) => r.id)).toEqual([1]);
    });

    it("puts every review in newReviews on first visit, sorted", () => {
        const reviews = [
            makeReview(2, "2025-06-01T11:00:00Z"),
            makeReview(1, "2025-06-01T08:00:00Z"),
        ];
        const { newReviews, oldReviews } = partitionReviews(reviews, null);
        expect(newReviews.map((r) => r.id)).toEqual([1, 2]);
        expect(oldReviews).toHaveLength(0);
    });
});

describe("isPassing", () => {
    it.each([
        "success",
        "skipped",
        "neutral",
    ])("treats a completed %s run as passing", (conclusion) => {
        expect(isPassing({ name: "CI", status: "completed", conclusion })).toBe(
            true,
        );
    });

    it("does not treat an in-progress run as passing", () => {
        expect(
            isPassing({ name: "CI", status: "in_progress", conclusion: null }),
        ).toBe(false);
    });

    it("does not treat a completed run with null conclusion as passing", () => {
        expect(
            isPassing({ name: "CI", status: "completed", conclusion: null }),
        ).toBe(false);
    });
});

describe("ciSummary", () => {
    const run = (status: string, conclusion: string | null): CheckRun => ({
        name: "CI",
        status,
        conclusion,
    });

    it("returns empty text and class when there are no check runs", () => {
        expect(ciSummary([])).toBe("");
    });

    it("reports passing when all runs succeed", () => {
        expect(ciSummary([run("completed", "success")])).toBe("CI passing");
    });

    it("counts skipped and neutral conclusions as passing", () => {
        expect(
            ciSummary([
                run("completed", "skipped"),
                run("completed", "neutral"),
            ]),
        ).toBe("CI passing");
    });

    it("reports running when some runs are not completed", () => {
        expect(
            ciSummary([run("completed", "success"), run("in_progress", null)]),
        ).toBe("1 running");
    });

    it("reports failing when a run fails, even if others are pending", () => {
        expect(
            ciSummary([run("completed", "failure"), run("in_progress", null)]),
        ).toBe("1 failing");
    });

    // Regression: matches the backend's derive_ci_status — an unknown (null)
    // conclusion on a completed run must count as failing, never as passing.
    it("counts a completed run with null conclusion as failing", () => {
        expect(
            ciSummary([run("completed", null), run("completed", "success")]),
        ).toBe("1 failing");
    });
});
