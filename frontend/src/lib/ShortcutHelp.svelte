<script lang="ts">
import { Dialog } from "bits-ui";
import { formatKeys, listShortcuts, type Shortcut } from "./shortcuts.ts";

let { open = $bindable(false) }: { open?: boolean } = $props();

// Read the registry when the dialog opens, so the list always matches what
// is actually bound.
let groups = $derived.by(() => {
    if (!open) return [];
    const byGroup = new Map<string, Shortcut[]>();
    for (const s of listShortcuts()) {
        const list = byGroup.get(s.group) ?? [];
        list.push(s);
        byGroup.set(s.group, list);
    }
    return [...byGroup];
});
</script>

<Dialog.Root bind:open>
    <Dialog.Portal>
        <Dialog.Overlay class="shortcut-overlay" />
        <Dialog.Content class="shortcut-dialog">
            <div class="shortcut-header">
                <Dialog.Title class="shortcut-title"
                    >Keyboard shortcuts</Dialog.Title
                >
                <Dialog.Close class="shortcut-close" aria-label="Close">
                    <svg
                        aria-hidden="true"
                        width="16"
                        height="16"
                        viewBox="0 0 16 16"
                        fill="currentColor"
                    >
                        <path
                            d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z"
                        />
                    </svg>
                </Dialog.Close>
            </div>
            <div class="shortcut-groups">
                {#each groups as [group, shortcuts] (group)}
                    <section>
                        <h3>{group}</h3>
                        <dl>
                            {#each shortcuts as s (s.description)}
                                <div class="shortcut-row">
                                    <dt>{s.description}</dt>
                                    <dd>
                                        {#each s.keys as binding, i (binding)}
                                            {#if i > 0}
                                                <span class="or">or</span>
                                            {/if}
                                            {#each formatKeys(
                                                binding,
                                            ) as key, j}
                                                {#if j > 0}
                                                    <span class="then"
                                                        >then</span
                                                    >
                                                {/if}
                                                <kbd>{key}</kbd>
                                            {/each}
                                        {/each}
                                    </dd>
                                </div>
                            {/each}
                        </dl>
                    </section>
                {/each}
            </div>
        </Dialog.Content>
    </Dialog.Portal>
</Dialog.Root>

<style>
:global(.shortcut-overlay) {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 50;
}
:global(.shortcut-dialog) {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow-y: auto;
    background: var(--canvas-default);
    border: 1px solid var(--border-default);
    border-radius: 12px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
    z-index: 51;
    color: var(--fg-default);
}
.shortcut-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-default);
}
:global(.shortcut-title) {
    font-size: 14px;
    font-weight: 600;
    margin: 0;
}
:global(.shortcut-close) {
    display: inline-flex;
    padding: 4px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
}
:global(.shortcut-close:hover) {
    background: var(--canvas-subtle);
    color: var(--fg-default);
}
.shortcut-groups {
    padding: 8px 16px 16px;
}
h3 {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
    margin: 12px 0 4px;
}
dl {
    margin: 0;
}
.shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 4px 0;
    font-size: 13px;
}
dt {
    color: var(--fg-default);
}
dd {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
}
kbd {
    display: inline-block;
    min-width: 20px;
    padding: 1px 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    line-height: 18px;
    text-align: center;
    color: var(--fg-default);
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-bottom-width: 2px;
    border-radius: 6px;
}
.or,
.then {
    font-size: 11px;
    color: var(--fg-subtle);
}
</style>
