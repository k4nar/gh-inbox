import { describe, expect, it } from "vitest";
import { activitySentence, formatActors } from "./activity.ts";
import type { InboxItem } from "./types.ts";

function item(overrides: Partial<InboxItem> = {}): InboxItem {
    return {
        id: "n1",
        pr_id: 42,
        title: "Fix bug",
        repository: "owner/repo",
        reason: "review_requested",
        unread: true,
        archived: false,
        updated_at: "2025-01-01T00:00:00Z",
        author: "alice",
        author_avatar_url: null,
        pr_status: "open",
        ci_status: null,
        teams: null,
        new_commits: 0,
        new_comments: [],
        new_reviews: [],
        ...overrides,
    };
}

describe("activitySentence", () => {
    it("returns null without an author", () => {
        expect(activitySentence(item({ author: null }))).toBeNull();
    });

    it("returns null while not yet enriched (undefined)", () => {
        expect(activitySentence(item({ new_commits: undefined }))).toBeNull();
    });

    it("returns null on first visit (null)", () => {
        expect(activitySentence(item({ new_commits: null }))).toBeNull();
    });

    it("returns empty string when enriched but quiet", () => {
        expect(activitySentence(item())).toBe("");
    });

    it("describes commits with singular/plural", () => {
        expect(activitySentence(item({ new_commits: 1 }))).toBe(
            "alice pushed 1 commit",
        );
        expect(activitySentence(item({ new_commits: 3 }))).toBe(
            "alice pushed 3 commits",
        );
    });

    it("aggregates comments across authors", () => {
        const sentence = activitySentence(
            item({
                new_comments: [
                    { author: "bob", count: 2 },
                    { author: "carol", count: 1 },
                ],
            }),
        );
        expect(sentence).toBe("bob and carol left 3 comments");
    });

    it("describes review outcomes", () => {
        const sentence = activitySentence(
            item({
                new_reviews: [
                    { reviewer: "bob", state: "APPROVED" },
                    { reviewer: "carol", state: "CHANGES_REQUESTED" },
                    { reviewer: "dave", state: "DISMISSED" },
                ],
            }),
        );
        expect(sentence).toBe(
            "bob approved · carol requested changes · dave's review dismissed",
        );
    });

    it("joins multiple kinds with a separator", () => {
        const sentence = activitySentence(
            item({
                new_commits: 2,
                new_reviews: [{ reviewer: "bob", state: "APPROVED" }],
            }),
        );
        expect(sentence).toBe("alice pushed 2 commits · bob approved");
    });
});

describe("formatActors", () => {
    it("handles zero, one, two and many names", () => {
        expect(formatActors([])).toBe("");
        expect(formatActors(["a"])).toBe("a");
        expect(formatActors(["a", "b"])).toBe("a and b");
        expect(formatActors(["a", "b", "c"])).toBe("a, b and c");
    });
});
