import type { ActiveFilters } from "./types.ts";

const STATE_LABELS: Record<string, string> = {
    open: "Open",
    draft: "Draft",
    merged: "Merged",
    closed: "Closed",
};

const STATE_ORDER = ["open", "draft", "merged", "closed"] as const;

/**
 * Human-readable list of the currently active filters, one entry per criterion.
 * Status include/exclude collapse into a single "Status: …" entry, e.g.
 * include=draft + exclude=open → "Status: Draft, not Open".
 */
export function describeActiveFilters(f: ActiveFilters): string[] {
    const out: string[] = [];
    if (f.repo) out.push(`Repository: ${f.repo}`);
    if (f.team) out.push(`Team: ${f.team}`);
    if (f.author) out.push(`Author: ${f.author}`);

    const states = f.states ?? {};
    const parts: string[] = [];
    for (const s of STATE_ORDER) {
        if (states[s] === "include") parts.push(STATE_LABELS[s] ?? s);
    }
    for (const s of STATE_ORDER) {
        if (states[s] === "exclude") parts.push(`not ${STATE_LABELS[s] ?? s}`);
    }
    if (parts.length > 0) out.push(`Status: ${parts.join(", ")}`);

    return out;
}

/** Number of active filter criteria (matches describeActiveFilters length). */
export function countActiveFilters(f: ActiveFilters): number {
    return describeActiveFilters(f).length;
}
