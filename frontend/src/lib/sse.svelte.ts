type SyncStatus = "idle" | "syncing" | "error";

let syncStatus: SyncStatus = $state("idle");

let newNotificationCallbacks: Array<() => void> = [];

export interface PrInfoUpdatedPayload {
    pr_id: number;
    repository: string;
    author: string;
    pr_status: "open" | "draft" | "merged" | "closed";
    ci_status: string | null;
    new_commits: number | null;
    new_comments: { author: string; count: number }[] | null;
    new_reviews: { reviewer: string; state: string }[] | null;
    teams: string[] | null;
}
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
        ) as { notification_id: string; message: string };
        for (const cb of githubSyncErrorCallbacks) {
            cb(notification_id, message);
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
