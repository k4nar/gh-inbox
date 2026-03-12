type SyncStatus = "idle" | "syncing" | "error";

let syncStatus: SyncStatus = $state("idle");

let newNotificationCallbacks: Array<() => void> = [];

let eventSource: EventSource | null = null;

export function getSyncStatus(): SyncStatus {
	return syncStatus;
}

export function onNewNotifications(callback: () => void): () => void {
	newNotificationCallbacks.push(callback);
	return () => {
		newNotificationCallbacks = newNotificationCallbacks.filter(
			(cb) => cb !== callback,
		);
	};
}

export function connectSSE(): void {
	if (eventSource) {
		eventSource.close();
	}

	eventSource = new EventSource("/api/events");

	eventSource.addEventListener("sync:status", (e) => {
		const { status } = JSON.parse((e as MessageEvent).data);
		if (status === "started") {
			syncStatus = "syncing";
		} else if (status === "completed") {
			syncStatus = "idle";
		} else {
			syncStatus = "error";
		}
	});

	eventSource.addEventListener("notifications:new", () => {
		for (const cb of newNotificationCallbacks) {
			cb();
		}
	});

	eventSource.addEventListener("open", () => {
		syncStatus = "idle";
	});

	eventSource.onerror = () => {
		syncStatus = "error";
	};
}

export function disconnectSSE(): void {
	if (eventSource) {
		eventSource.close();
		eventSource = null;
	}
}
