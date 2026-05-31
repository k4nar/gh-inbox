<script lang="ts">
import { Tooltip } from "bits-ui";
import Sidebar from "./Sidebar.svelte";

let { ...props } = $props();
</script>

<Tooltip.Provider delayDuration={0}> <Sidebar {...props} /> </Tooltip.Provider>
