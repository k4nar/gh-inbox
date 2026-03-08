<script>
  import Topbar from './lib/Topbar.svelte'
  import Sidebar from './lib/Sidebar.svelte'
  import PrList from './lib/PrList.svelte'
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
