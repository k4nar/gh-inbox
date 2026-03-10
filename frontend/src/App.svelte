<script>
import PrDetail from "./lib/PrDetail.svelte";
import PrList from "./lib/PrList.svelte";
import Sidebar from "./lib/Sidebar.svelte";
import Topbar from "./lib/Topbar.svelte";

let selectedNotification = $state(null);

function handleSelect(notification) {
	selectedNotification = notification;
}

function handleClose() {
	selectedNotification = null;
}
</script>

<Topbar />
<div class="layout">
  <Sidebar />
  <PrList onSelect={handleSelect} selectedId={selectedNotification?.id} />
  {#if selectedNotification}
    <PrDetail notification={selectedNotification} onClose={handleClose} />
  {/if}
</div>

<style>
  .layout {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
</style>
