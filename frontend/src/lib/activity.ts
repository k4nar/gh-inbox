import type { InboxItem } from "./types.ts";

/**
 * Human sentence describing what changed on a PR since the last visit,
 * e.g. "alice pushed 2 commits · bob left 3 comments · carol approved".
 *
 * Returns:
 * - null — nothing to render (no author, not yet enriched, or first visit,
 *   which the row template renders as "✦ New pull request")
 * - ""   — enriched, but no new activity since the last visit
 * - text — the sentence
 */
export function activitySentence(item: InboxItem): string | null {
    if (!item.author) return null;
    if (item.new_commits === undefined) return null; // not yet enriched — render nothing
    if (item.new_commits === null) return null; // first visit — handled separately
    const parts: string[] = [];
    if (item.new_commits > 0) {
        const n = item.new_commits;
        parts.push(`${item.author} pushed ${n} commit${n === 1 ? "" : "s"}`);
    }
    if (item.new_comments && item.new_comments.length > 0) {
        const actors = formatActors(item.new_comments.map((c) => c.author));
        const total = item.new_comments.reduce((s, c) => s + c.count, 0);
        parts.push(`${actors} left ${total} comment${total === 1 ? "" : "s"}`);
    }
    if (item.new_reviews && item.new_reviews.length > 0) {
        for (const review of item.new_reviews) {
            if (review.state === "APPROVED") {
                parts.push(`${review.reviewer} approved`);
            } else if (review.state === "CHANGES_REQUESTED") {
                parts.push(`${review.reviewer} requested changes`);
            } else if (review.state === "DISMISSED") {
                parts.push(`${review.reviewer}'s review dismissed`);
            }
        }
    }
    return parts.length > 0 ? parts.join(" · ") : "";
}

/** "a", "a and b", "a, b and c" */
export function formatActors(names: string[]): string {
    if (names.length === 0) return "";
    if (names.length === 1) return names[0];
    return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}
