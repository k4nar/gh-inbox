import { describe, expect, it } from "vitest";
import { countActiveFilters, describeActiveFilters } from "./filters.ts";

describe("describeActiveFilters", () => {
    it("returns an empty list when no filters are set", () => {
        expect(describeActiveFilters({})).toEqual([]);
    });

    it("describes scalar filters", () => {
        expect(
            describeActiveFilters({
                repo: "acme/api",
                team: "acme/platform",
                author: "alice",
            }),
        ).toEqual([
            "Repository: acme/api",
            "Team: acme/platform",
            "Author: alice",
        ]);
    });

    it("groups status include/exclude into one entry as 'Status: X, not Y'", () => {
        expect(
            describeActiveFilters({
                states: { draft: "include", open: "exclude" },
            }),
        ).toEqual(["Status: Draft, not Open"]);
    });

    it("orders included before excluded and follows state order", () => {
        expect(
            describeActiveFilters({
                states: {
                    closed: "exclude",
                    open: "include",
                    merged: "exclude",
                    draft: "include",
                },
            }),
        ).toEqual(["Status: Open, Draft, not Merged, not Closed"]);
    });

    it("combines scalar and status filters", () => {
        expect(
            describeActiveFilters({
                repo: "acme/api",
                states: { merged: "exclude" },
            }),
        ).toEqual(["Repository: acme/api", "Status: not Merged"]);
    });
});

describe("countActiveFilters", () => {
    it("counts each criterion, with status grouped as one", () => {
        expect(
            countActiveFilters({
                repo: "acme/api",
                author: "alice",
                states: { open: "include", merged: "exclude" },
            }),
        ).toBe(3);
    });

    it("is zero with no filters", () => {
        expect(countActiveFilters({})).toBe(0);
    });
});
