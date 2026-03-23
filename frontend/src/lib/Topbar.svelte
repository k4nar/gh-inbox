<script lang="ts">
let {
    syncStatus = "idle",
    theme = "system",
    onThemeChange,
}: {
    syncStatus?: string;
    theme?: "system" | "light" | "dark";
    onThemeChange?: (theme: "system" | "light" | "dark") => void;
} = $props();

let statusText = $derived(
    syncStatus === "syncing"
        ? "syncing…"
        : syncStatus === "error"
          ? "sync error"
          : "synced",
);

function handleThemeChange(e: Event) {
    const val = (e.target as HTMLSelectElement).value as
        | "system"
        | "light"
        | "dark";
    onThemeChange?.(val);
}
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
    <label class="theme-label" for="theme-select">Theme</label>
    <select
        id="theme-select"
        class="theme-select"
        value={theme}
        onchange={handleThemeChange}
    >
        <option value="system">System</option>
        <option value="light">Light</option>
        <option value="dark">Dark</option>
    </select>
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
.theme-label {
    font-size: 12px;
    color: var(--fg-muted);
}
.theme-select {
    font-size: 12px;
    color: var(--fg-default);
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 2px 6px;
    cursor: pointer;
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
