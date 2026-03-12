<script lang="ts">
import { onMount } from "svelte";
import PrDetail from "./lib/PrDetail.svelte";
import PrList from "./lib/PrList.svelte";
import Sidebar from "./lib/Sidebar.svelte";
import {
    connectSSE,
    disconnectSSE,
    getSyncStatus,
    onNewNotifications,
} from "./lib/sse.svelte.ts";
import Toast from "./lib/Toast.svelte";
import Topbar from "./lib/Topbar.svelte";
import type { Notification } from "./lib/types.ts";

let currentView = $state("inbox");
let selectedNotification: Notification | null = $state(null);
let refreshKey = $state(0);

function handleSelect(notification: Notification): void {
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
    const unsubscribe = onNewNotifications(() => {
        refreshKey++;
    });
    return () => {
        unsubscribe();
        disconnectSSE();
    };
});
</script>

<Topbar syncStatus={getSyncStatus()} />
<div class="layout">
    <Sidebar {currentView} onViewChange={handleViewChange} />
    <PrList
        {currentView}
        onSelect={handleSelect}
        selectedId={selectedNotification?.id}
        {refreshKey}
    />
    {#if selectedNotification}
        <PrDetail notification={selectedNotification} onClose={handleClose} />
    {/if}
</div>
<Toast />

<style>
.layout {
    display: flex;
    flex: 1;
    overflow: hidden;
}
</style>
