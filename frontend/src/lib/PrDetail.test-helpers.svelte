<script lang="ts">
import { Tooltip } from "bits-ui";
import type { ComponentProps } from "svelte";
import PrDetail from "./PrDetail.svelte";

let props: ComponentProps<typeof PrDetail> = $props();
</script>

<Tooltip.Provider delayDuration={0}> <PrDetail {...props} /> </Tooltip.Provider>
