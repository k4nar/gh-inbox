import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Mock EventSource before importing the module
class MockEventSource {
	constructor(url) {
		this.url = url;
		this.listeners = {};
		this.onerror = null;
		MockEventSource.instance = this;
	}
	addEventListener(type, handler) {
		this.listeners[type] = handler;
	}
	close() {
		this.closed = true;
	}
	// Helper to simulate events
	simulateEvent(type, data) {
		if (this.listeners[type]) {
			this.listeners[type]({ data: JSON.stringify(data) });
		}
	}
}

describe("SSE utility", () => {
	beforeEach(() => {
		globalThis.EventSource = MockEventSource;
	});

	afterEach(() => {
		vi.resetModules();
		delete globalThis.EventSource;
	});

	it("connectSSE creates an EventSource to /api/events", async () => {
		const { connectSSE, disconnectSSE } = await import("./sse.svelte.js");
		connectSSE();
		expect(MockEventSource.instance.url).toBe("/api/events");
		disconnectSSE();
	});

	it("sync:status started sets status to syncing", async () => {
		const { connectSSE, getSyncStatus, disconnectSSE } = await import(
			"./sse.svelte.js"
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
			"./sse.svelte.js"
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
			"./sse.svelte.js"
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

	it("unsubscribe removes callback", async () => {
		const { connectSSE, onNewNotifications, disconnectSSE } = await import(
			"./sse.svelte.js"
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
});
