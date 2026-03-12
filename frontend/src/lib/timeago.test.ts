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
});
