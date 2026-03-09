<script>
import PrList from "./lib/PrList.svelte";
import Sidebar from "./lib/Sidebar.svelte";
import Topbar from "./lib/Topbar.svelte";
</script>

<Topbar />
<div class="layout">
  <Sidebar />
  <PrList />
</div>

<style>
  .layout {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
</style>
