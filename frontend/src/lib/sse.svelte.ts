import type { GithubSyncErrorData } from "./generated/GithubSyncErrorData";
import type { PrInfoUpdatedData } from "./generated/PrInfoUpdatedData";
import type { SyncStatusData } from "./generated/SyncStatusData";

type SyncStatus = "idle" | "syncing" | "error";

let syncStatus: SyncStatus = $state("idle");
let syncErrorMessage: string | null = $state(null);

let newNotificationCallbacks: Array<() => void> = [];

// Generated from the Rust PrInfoUpdatedData — re-exported under the name the
// components already use.
export type PrInfoUpdatedPayload = PrInfoUpdatedData;
type PrInfoUpdatedCallback = (data: PrInfoUpdatedPayload) => void;
let prInfoUpdatedCallbacks: PrInfoUpdatedCallback[] = [];

type GithubSyncErrorCallback = (
    notificationId: string,
    message: string,
) => void;
let githubSyncErrorCallbacks: GithubSyncErrorCallback[] = [];

let eventSource: EventSource | null = null;

export function getSyncStatus(): SyncStatus {
    return syncStatus;
}

export function getSyncErrorMessage(): string | null {
    return syncErrorMessage;
}

export function onNewNotifications(callback: () => void): () => void {
    newNotificationCallbacks.push(callback);
    return () => {
        newNotificationCallbacks = newNotificationCallbacks.filter(
            (cb) => cb !== callback,
        );
    };
}

export function onPrInfoUpdated(callback: PrInfoUpdatedCallback): () => void {
    prInfoUpdatedCallbacks.push(callback);
    return () => {
        prInfoUpdatedCallbacks = prInfoUpdatedCallbacks.filter(
            (cb) => cb !== callback,
        );
    };
}

export function onGithubSyncError(
    callback: GithubSyncErrorCallback,
): () => void {
    githubSyncErrorCallbacks.push(callback);
    return () => {
        githubSyncErrorCallbacks = githubSyncErrorCallbacks.filter(
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
        const { status } = JSON.parse(
            (e as MessageEvent).data,
        ) as SyncStatusData;
        if (status === "started") {
            syncStatus = "syncing";
            syncErrorMessage = null;
        } else if (status === "completed") {
            syncStatus = "idle";
            syncErrorMessage = null;
        } else if (
            status !== null &&
            typeof status === "object" &&
            "errored" in status
        ) {
            syncStatus = "error";
            syncErrorMessage = (status as { errored: { message: string } })
                .errored.message;
        } else {
            syncStatus = "error";
        }
    });

    eventSource.addEventListener("notifications:new", () => {
        for (const cb of newNotificationCallbacks) {
            cb();
        }
    });

    eventSource.addEventListener("pr:info_updated", (e) => {
        const data = JSON.parse(
            (e as MessageEvent).data,
        ) as PrInfoUpdatedPayload;
        for (const cb of prInfoUpdatedCallbacks) {
            cb(data);
        }
    });

    eventSource.addEventListener("github:sync_error", (e) => {
        const { notification_id, message } = JSON.parse(
            (e as MessageEvent).data,
        ) as GithubSyncErrorData;
        for (const cb of githubSyncErrorCallbacks) {
            cb(notification_id, message);
        }
    });

    eventSource.addEventListener("open", () => {
        syncStatus = "idle";
        syncErrorMessage = null;
        // A (re)connect means we may have missed `notifications:new` pushes
        // while disconnected (broadcast events are not replayed). Refetch so the
        // UI reflects whatever the backend synced in the meantime.
        for (const cb of newNotificationCallbacks) {
            cb();
        }
    });

    eventSource.onerror = () => {
        syncStatus = "error";
        syncErrorMessage = "Connection to server lost.";
    };
}

export function disconnectSSE(): void {
    if (eventSource) {
        eventSource.close();
        eventSource = null;
    }
}
