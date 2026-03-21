<script lang="ts">
import { Tooltip } from "bits-ui";
import { onMount } from "svelte";
import PrDetail from "./lib/PrDetail.svelte";
import PrList from "./lib/PrList.svelte";
import ResizableDetailPanel from "./lib/ResizableDetailPanel.svelte";
import Sidebar from "./lib/Sidebar.svelte";
import {
    connectSSE,
    disconnectSSE,
    getSyncStatus,
    onGithubSyncError,
    onNewNotifications,
} from "./lib/sse.svelte.ts";
import Toast from "./lib/Toast.svelte";
import Topbar from "./lib/Topbar.svelte";
import { showError } from "./lib/toast.svelte.ts";
import type { InboxItem } from "./lib/types.ts";

let currentView = $state("inbox");
let selectedNotification: InboxItem | null = $state(null);
let refreshKey = $state(0);

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

    return () => {
        unsubNotifications();
        unsubGithubError();
        disconnectSSE();
    };
});
</script>

<Tooltip.Provider delayDuration={0}>
    <Topbar syncStatus={getSyncStatus()} />
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
