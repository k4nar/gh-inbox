type SyncStatus = "idle" | "syncing" | "error";

let syncStatus: SyncStatus = $state("idle");

let newNotificationCallbacks: Array<() => void> = [];

type TeamsUpdatedCallback = (pr_id: number, teams: string[]) => void;
let teamsUpdatedCallbacks: TeamsUpdatedCallback[] = [];

export interface PrInfoUpdatedPayload {
    pr_id: number;
    repository: string;
    author: string;
    pr_status: "open" | "draft" | "merged" | "closed";
    new_commits: number | null;
    new_comments: { author: string; count: number }[] | null;
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

export function onPrTeamsUpdated(callback: TeamsUpdatedCallback): () => void {
    teamsUpdatedCallbacks.push(callback);
    return () => {
        teamsUpdatedCallbacks = teamsUpdatedCallbacks.filter(
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

    eventSource.addEventListener("pr:teams_updated", (e) => {
        const { pr_id, teams } = JSON.parse((e as MessageEvent).data) as {
            pr_id: number;
            teams: string[];
        };
        for (const cb of teamsUpdatedCallbacks) {
            cb(pr_id, teams);
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
