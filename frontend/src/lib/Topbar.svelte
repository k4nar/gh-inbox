<script lang="ts">
import { Select } from "bits-ui";
import type { Theme } from "./types.ts";

let {
    syncStatus = "idle",
    theme = "system",
    onThemeChange,
    onSync,
}: {
    syncStatus?: string;
    theme?: Theme;
    onThemeChange?: (theme: Theme) => void;
    onSync?: () => void;
} = $props();

const themeGroups: {
    label: string;
    themes: { value: Theme; label: string }[];
}[] = [
    {
        label: "Github",
        themes: [
            { value: "system", label: "System" },
            { value: "light", label: "Light" },
            { value: "dark", label: "Dark" },
        ],
    },
    {
        label: "Catppuccin",
        themes: [
            { value: "catppuccin-latte", label: "Latte" },
            { value: "catppuccin-frappe", label: "Frappé" },
            { value: "catppuccin-macchiato", label: "Macchiato" },
            { value: "catppuccin-mocha", label: "Mocha" },
        ],
    },
];

const themeLabel = $derived(
    themeGroups.flatMap((g) => g.themes).find((t) => t.value === theme)
        ?.label ?? theme,
);

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
                y="2"
                width="14"
                height="12"
                rx="2"
                fill="var(--accent-fg)"
            />
            <path
                d="M1 9.5h3.5l1.5 2.5h4l1.5-2.5H15"
                stroke="white"
                stroke-width="1.2"
                stroke-linejoin="round"
            />
            <line
                x1="4"
                y1="5.5"
                x2="12"
                y2="5.5"
                stroke="white"
                stroke-width="1.2"
                stroke-linecap="round"
            />
            <line
                x1="4"
                y1="7.5"
                x2="9"
                y2="7.5"
                stroke="white"
                stroke-width="1.2"
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
            onThemeChange?.(v as Theme)}
    >
        <Select.Trigger class="theme-trigger" aria-label="Theme">
            Theme · {themeLabel}
        </Select.Trigger>
        <Select.Portal>
            <Select.Content class="theme-content">
                <Select.Viewport>
                    {#each themeGroups as group}
                        <Select.Group>
                            <Select.GroupHeading class="theme-group-heading">
                                {group.label}
                            </Select.GroupHeading>
                            {#each group.themes as { value, label }}
                                <Select.Item class="theme-item" {value} {label}>
                                    {label}
                                </Select.Item>
                            {/each}
                        </Select.Group>
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
        <button
            type="button"
            class="sync-btn"
            disabled={syncStatus === "syncing"}
            onclick={() => onSync?.()}
            aria-label="Force sync"
            title="Force sync"
        >
            <svg
                width="12"
                height="12"
                viewBox="0 0 16 16"
                fill="currentColor"
                aria-hidden="true"
            >
                <path
                    d="M11.534 7h3.932a.25.25 0 0 1 .192.41l-1.966 2.36a.25.25 0 0 1-.384 0l-1.966-2.36a.25.25 0 0 1 .192-.41zm-11 2h3.932a.25.25 0 0 0 .192-.41L2.692 6.23a.25.25 0 0 0-.384 0L.342 8.59A.25.25 0 0 0 .534 9z"
                />
                <path
                    fill-rule="evenodd"
                    d="M8 3a4.995 4.995 0 0 0-4.192 2.268a.75.75 0 1 1-1.255-.823A6.5 6.5 0 0 1 14.466 7H13.46A5.001 5.001 0 0 0 8 3zM3.534 9H2.528A6.5 6.5 0 0 0 13.44 11.55a.75.75 0 1 1-1.255.822A5 5 0 0 1 8 13a5.001 5.001 0 0 1-4.466-2.75l-.001-.25z"
                />
            </svg>
        </button>
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
.sync-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    padding: 2px;
    cursor: pointer;
    color: var(--fg-muted);
    border-radius: 4px;
    line-height: 0;
}
.sync-btn:hover:not(:disabled) {
    color: var(--fg-default);
    background: var(--canvas-subtle);
}
.sync-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
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
:global(.theme-group-heading) {
    font-size: 11px;
    font-weight: 600;
    color: var(--fg-subtle);
    padding: 4px 8px 2px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
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
