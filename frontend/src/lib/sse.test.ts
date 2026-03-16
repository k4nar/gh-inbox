import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

class MockEventSource {
    static instance: MockEventSource;
    url: string;
    listeners: Record<string, (event: { data: string }) => void> = {};
    onerror: (() => void) | null = null;
    closed = false;

    constructor(url: string) {
        this.url = url;
        MockEventSource.instance = this;
    }
    addEventListener(type: string, handler: (event: { data: string }) => void) {
        this.listeners[type] = handler;
    }
    close() {
        this.closed = true;
    }
    simulateEvent(type: string, data: unknown) {
        if (this.listeners[type]) {
            this.listeners[type]({ data: JSON.stringify(data) });
        }
    }
}

describe("SSE utility", () => {
    beforeEach(() => {
        (globalThis as Record<string, unknown>).EventSource = MockEventSource;
    });

    afterEach(() => {
        vi.resetModules();
        delete (globalThis as Record<string, unknown>).EventSource;
    });

    it("connectSSE creates an EventSource to /api/events", async () => {
        const { connectSSE, disconnectSSE } = await import("./sse.svelte.ts");
        connectSSE();
        expect(MockEventSource.instance.url).toBe("/api/events");
        disconnectSSE();
    });

    it("sync:status started sets status to syncing", async () => {
        const { connectSSE, getSyncStatus, disconnectSSE } = await import(
            "./sse.svelte.ts"
        );
        connectSSE();
        MockEventSource.instance.simulateEvent("sync:status", {
            status: "started",
        });
        expect(getSyncStatus()).toBe("syncing");
        disconnectSSE();
    });

    it("sync:status completed sets status to idle", async () => {
        const { connectSSE, getSyncStatus, disconnectSSE } = await import(
            "./sse.svelte.ts"
        );
        connectSSE();
        MockEventSource.instance.simulateEvent("sync:status", {
            status: "completed",
        });
        expect(getSyncStatus()).toBe("idle");
        disconnectSSE();
    });

    it("notifications:new triggers registered callbacks", async () => {
        const { connectSSE, onNewNotifications, disconnectSSE } = await import(
            "./sse.svelte.ts"
        );
        connectSSE();
        const callback = vi.fn();
        onNewNotifications(callback);
        MockEventSource.instance.simulateEvent("notifications:new", {
            count: 5,
        });
        expect(callback).toHaveBeenCalledOnce();
        disconnectSSE();
    });

    it("open event resets status to idle after error", async () => {
        const { connectSSE, getSyncStatus, disconnectSSE } = await import(
            "./sse.svelte.ts"
        );
        connectSSE();
        MockEventSource.instance.onerror!();
        expect(getSyncStatus()).toBe("error");
        MockEventSource.instance.simulateEvent("open", {});
        expect(getSyncStatus()).toBe("idle");
        disconnectSSE();
    });

    it("unsubscribe removes callback", async () => {
        const { connectSSE, onNewNotifications, disconnectSSE } = await import(
            "./sse.svelte.ts"
        );
        connectSSE();
        const callback = vi.fn();
        const unsubscribe = onNewNotifications(callback);
        unsubscribe();
        MockEventSource.instance.simulateEvent("notifications:new", {
            count: 1,
        });
        expect(callback).not.toHaveBeenCalled();
        disconnectSSE();
    });

    it("pr:info_updated triggers registered callbacks with the full payload", async () => {
        const { connectSSE, onPrInfoUpdated, disconnectSSE } = await import(
            "./sse.svelte.ts"
        );
        connectSSE();
        const callback = vi.fn();
        onPrInfoUpdated(callback);
        const payload = {
            pr_id: 7,
            repository: "acme/api",
            author: "bob",
            pr_status: "open",
            new_commits: 2,
            new_comments: [{ author: "alice", count: 1 }],
        };
        MockEventSource.instance.simulateEvent("pr:info_updated", payload);
        expect(callback).toHaveBeenCalledOnce();
        expect(callback).toHaveBeenCalledWith(payload);
        disconnectSSE();
    });

    it("pr:teams_updated triggers registered callbacks with pr_id and teams", async () => {
        const { connectSSE, onPrTeamsUpdated, disconnectSSE } = await import(
            "./sse.svelte.ts"
        );
        connectSSE();
        const callback = vi.fn();
        onPrTeamsUpdated(callback);
        MockEventSource.instance.simulateEvent("pr:teams_updated", {
            pr_id: 42,
            teams: ["acme/platform"],
        });
        expect(callback).toHaveBeenCalledOnce();
        expect(callback).toHaveBeenCalledWith(42, ["acme/platform"]);
        disconnectSSE();
    });
});
