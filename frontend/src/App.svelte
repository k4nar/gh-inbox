<script lang="ts">
import { Tooltip } from "bits-ui";
import { onMount } from "svelte";
import { apiFetch } from "./lib/api.ts";
import PrDetail from "./lib/PrDetail.svelte";
import PrList from "./lib/PrList.svelte";
import ResizableDetailPanel from "./lib/ResizableDetailPanel.svelte";
import Sidebar from "./lib/Sidebar.svelte";
import {
    connectSSE,
    disconnectSSE,
    getSyncErrorMessage,
    getSyncStatus,
    onGithubSyncError,
    onNewNotifications,
} from "./lib/sse.svelte.ts";
import Toast from "./lib/Toast.svelte";
import Topbar from "./lib/Topbar.svelte";
import { showError } from "./lib/toast.svelte.ts";
import type { InboxItem, Preferences, Theme } from "./lib/types.ts";

let currentView = $state("inbox");
let selectedNotification: InboxItem | null = $state(null);
let refreshKey = $state(0);
let theme: Theme = $state("system");

function applyTheme(t: Theme) {
    if (t === "system") {
        delete document.documentElement.dataset.theme;
    } else {
        document.documentElement.dataset.theme = t;
    }
}

async function handleThemeChange(t: Theme) {
    const prev = theme;
    theme = t;
    applyTheme(t);
    try {
        await apiFetch("/api/preferences", {
            method: "PATCH",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ theme: t }),
        });
    } catch {
        // Roll back on failure so the DOM and dropdown stay in sync with the DB.
        theme = prev;
        applyTheme(prev);
        showError("Failed to save theme preference");
    }
}

async function handleSync(): Promise<void> {
    try {
        await apiFetch<void>("/api/sync", { method: "POST" });
    } catch {
        // SSE stream provides feedback — ignore fetch errors here
    }
}

function handleSelect(notification: InboxItem | null): void {
    if (notification && selectedNotification?.id === notification.id) {
        selectedNotification = null;
        return;
    }

    selectedNotification = notification;
}

function handleClose(): void {
    selectedNotification = null;
}

function handleViewChange(view: string): void {
    currentView = view;
    selectedNotification = null;
}

onMount(() => {
    connectSSE();
    const unsubNotifications = onNewNotifications(() => {
        refreshKey++;
    });
    const unsubGithubError = onGithubSyncError((_notificationId, message) => {
        showError("Failed to sync with GitHub");
        console.error("GitHub sync error:", message);
    });

    apiFetch<Preferences>("/api/preferences")
        .then((prefs) => {
            theme = prefs.theme;
            applyTheme(prefs.theme);
        })
        .catch(() => {
            showError("Failed to load theme preference");
        });

    return () => {
        unsubNotifications();
        unsubGithubError();
        disconnectSSE();
    };
});
</script>

<Tooltip.Provider delayDuration={0}>
    <Topbar
        syncStatus={getSyncStatus()}
        syncErrorMessage={getSyncErrorMessage()}
        {theme}
        onThemeChange={handleThemeChange}
        onSync={handleSync}
    />
    <div class="layout">
        <Sidebar {currentView} onViewChange={handleViewChange} />
        <PrList
            {currentView}
            onSelect={handleSelect}
            onSelectionChange={handleSelect}
            selectedId={selectedNotification?.id}
            {refreshKey}
        />
        {#if selectedNotification}
            <ResizableDetailPanel>
                <PrDetail
                    notification={selectedNotification}
                    onClose={handleClose}
                />
            </ResizableDetailPanel>
        {/if}
    </div>
    <Toast />
</Tooltip.Provider>

<style>
.layout {
    display: flex;
    flex: 1;
    overflow: hidden;
}
</style>
