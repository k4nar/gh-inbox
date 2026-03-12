<script lang="ts">
import { getToasts } from "./toast.svelte.ts";

let toasts = $derived(getToasts());
</script>

{#if toasts.length > 0}
    <div class="toast-container">
        {#each toasts as toast (toast.id)}
            <div class="toast">{toast.message}</div>
        {/each}
    </div>
{/if}

<style>
.toast-container {
    position: fixed;
    bottom: 16px;
    right: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 1000;
}
.toast {
    background: var(--danger-subtle, #3d1214);
    border: 1px solid var(--danger-emphasis, #da3633);
    color: var(--danger-fg, #f85149);
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    max-width: 360px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    animation: toast-in 0.2s ease-out;
}
@keyframes toast-in {
    from {
        opacity: 0;
        transform: translateY(8px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}
</style>
