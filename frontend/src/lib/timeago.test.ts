import { afterEach, describe, expect, it, vi } from "vitest";
import { timeAgo } from "./timeago.ts";

describe("timeAgo", () => {
    afterEach(() => {
        vi.useRealTimers();
    });

    it('returns "just now" for timestamps less than a minute ago', () => {
        vi.useFakeTimers({ now: new Date("2025-06-01T12:00:30Z") });
        expect(timeAgo("2025-06-01T12:00:00Z")).toBe("just now");
    });

    it("returns minutes ago", () => {
        vi.useFakeTimers({ now: new Date("2025-06-01T12:05:00Z") });
        expect(timeAgo("2025-06-01T12:00:00Z")).toBe("5 min ago");
    });

    it("returns hours ago", () => {
        vi.useFakeTimers({ now: new Date("2025-06-01T15:00:00Z") });
        expect(timeAgo("2025-06-01T12:00:00Z")).toBe("3 hr ago");
    });

    it('returns "Yesterday" for 1 day ago', () => {
        vi.useFakeTimers({ now: new Date("2025-06-02T12:00:00Z") });
        expect(timeAgo("2025-06-01T12:00:00Z")).toBe("Yesterday");
    });

    it("returns days ago for multiple days", () => {
        vi.useFakeTimers({ now: new Date("2025-06-04T12:00:00Z") });
        expect(timeAgo("2025-06-01T12:00:00Z")).toBe("3 days ago");
    });

    it("returns months ago instead of large day counts", () => {
        vi.useFakeTimers({ now: new Date("2025-06-01T12:00:00Z") });
        expect(timeAgo("2025-03-01T12:00:00Z")).toBe("3 months ago");
        expect(timeAgo("2025-04-25T12:00:00Z")).toBe("1 month ago");
    });

    it("returns years ago beyond twelve months", () => {
        vi.useFakeTimers({ now: new Date("2025-06-01T12:00:00Z") });
        expect(timeAgo("2023-05-01T12:00:00Z")).toBe("2 years ago");
        expect(timeAgo("2024-01-01T12:00:00Z")).toBe("1 year ago");
    });

    it("returns an empty string for unparseable dates", () => {
        expect(timeAgo("not a date")).toBe("");
        expect(timeAgo("")).toBe("");
    });
});
