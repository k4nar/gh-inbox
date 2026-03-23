<script lang="ts">
import { Select } from "bits-ui";

let {
    syncStatus = "idle",
    theme = "system",
    onThemeChange,
}: {
    syncStatus?: string;
    theme?: "system" | "light" | "dark";
    onThemeChange?: (theme: "system" | "light" | "dark") => void;
} = $props();

const themeLabels: Record<"system" | "light" | "dark", string> = {
    system: "System",
    light: "Light",
    dark: "Dark",
};

let statusText = $derived(
    syncStatus === "syncing"
        ? "syncing…"
        : syncStatus === "error"
          ? "sync error"
          : "synced",
);
</script>

<header class="topbar">
    <div class="topbar-logo">
        <svg
            aria-hidden="true"
            width="20"
            height="20"
            viewBox="0 0 16 16"
            fill="none"
        >
            <rect
                x="1"
                y="1"
                width="14"
                height="14"
                rx="3"
                stroke="var(--fg-muted)"
                stroke-width="1.3"
            />
            <path
                d="M4 5h8M4 8h5M4 11h6"
                stroke="var(--accent-fg)"
                stroke-width="1.5"
                stroke-linecap="round"
            />
        </svg>
        gh-inbox
    </div>
    <div class="topbar-spacer"></div>
    <Select.Root
        type="single"
        value={theme}
        onValueChange={(v) =>
            onThemeChange?.(v as "system" | "light" | "dark")}
    >
        <Select.Trigger class="theme-trigger" aria-label="Theme">
            Theme · {themeLabels[theme]}
        </Select.Trigger>
        <Select.Portal>
            <Select.Content class="theme-content">
                <Select.Viewport>
                    {#each (["system", "light", "dark"] as const) as value}
                        <Select.Item
                            class="theme-item"
                            {value}
                            label={themeLabels[value]}
                        >
                            {themeLabels[value]}
                        </Select.Item>
                    {/each}
                </Select.Viewport>
            </Select.Content>
        </Select.Portal>
    </Select.Root>
    <div class="topbar-sync">
        <div
            class="sync-dot"
            class:syncing={syncStatus === "syncing"}
            class:error={syncStatus === "error"}
        ></div>
        {statusText}
    </div>
</header>

<style>
.topbar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 16px;
    height: 48px;
    border-bottom: 1px solid var(--border-default);
    background: var(--canvas-default);
    flex-shrink: 0;
}
.topbar-logo {
    font-size: 14px;
    font-weight: 600;
    color: var(--fg-default);
    display: flex;
    align-items: center;
    gap: 8px;
}
.topbar-spacer {
    flex: 1;
}
.topbar-sync {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--fg-muted);
    font-size: 12px;
}
.sync-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--success-fg);
}
.sync-dot.syncing {
    background: var(--attention-fg);
    animation: pulse 1s ease-in-out infinite;
}
.sync-dot.error {
    background: var(--danger-fg);
}
:global(.theme-trigger) {
    font-size: 12px;
    color: var(--fg-default);
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 2px 6px;
    cursor: pointer;
}
:global(.theme-content) {
    background: var(--canvas-default);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 4px;
    min-width: 100px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
    z-index: 100;
}
:global(.theme-item) {
    font-size: 12px;
    color: var(--fg-default);
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
}
:global(.theme-item[data-highlighted]) {
    background: var(--canvas-subtle);
}
:global(.theme-item[data-selected]) {
    color: var(--accent-fg);
}
@keyframes pulse {
    0%,
    100% {
        opacity: 1;
    }
    50% {
        opacity: 0.4;
    }
}
</style>
