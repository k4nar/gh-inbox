<script lang="ts">
import { Tooltip } from "bits-ui";
import PrList from "./PrList.svelte";

let { ...props } = $props();
</script>

<Tooltip.Provider delayDuration={0}> <PrList {...props} /> </Tooltip.Provider>
