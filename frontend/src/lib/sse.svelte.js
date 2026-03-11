/** @type {"idle" | "syncing" | "error"} */
let syncStatus = $state("idle");

/** @type {Array<() => void>} */
let newNotificationCallbacks = [];

/** @type {EventSource | null} */
let eventSource = null;

export function getSyncStatus() {
	return syncStatus;
}

export function onNewNotifications(callback) {
	newNotificationCallbacks.push(callback);
	return () => {
		newNotificationCallbacks = newNotificationCallbacks.filter(
			(cb) => cb !== callback,
		);
	};
}

export function connectSSE() {
	if (eventSource) {
		eventSource.close();
	}

	eventSource = new EventSource("/api/events");

	eventSource.addEventListener("sync:status", (e) => {
		const { status } = JSON.parse(e.data);
		if (status === "started") {
			syncStatus = "syncing";
		} else if (status === "completed") {
			syncStatus = "idle";
		} else {
			// "errored" variant is an object like {"errored":{"message":"..."}}
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

export function disconnectSSE() {
	if (eventSource) {
		eventSource.close();
		eventSource = null;
	}
}
