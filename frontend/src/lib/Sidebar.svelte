<script lang="ts">
let {
    currentView = "inbox",
    onViewChange = (_view: string) => {},
}: { currentView?: string; onViewChange?: (view: string) => void } = $props();
</script>

<nav class="sidebar">
    <div class="sidebar-section">
        <button
            type="button"
            class="sidebar-item"
            class:active={currentView === "inbox"}
            onclick={() => onViewChange("inbox")}
        >
            <svg
                aria-hidden="true"
                width="16"
                height="16"
                viewBox="0 0 16 16"
                fill="currentColor"
            >
                <path
                    d="M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v9.5A1.75 1.75 0 0 1 14.25 13H8.06l-2.573 2.573A1.458 1.458 0 0 1 3 14.543V13H1.75A1.75 1.75 0 0 1 0 11.25Zm1.75-.25a.25.25 0 0 0-.25.25v9.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-9.5a.25.25 0 0 0-.25-.25Z"
                />
            </svg>
            Inbox
        </button>
        <button
            type="button"
            class="sidebar-item"
            class:active={currentView === "archived"}
            onclick={() => onViewChange("archived")}
        >
            <svg
                aria-hidden="true"
                width="16"
                height="16"
                viewBox="0 0 16 16"
                fill="currentColor"
            >
                <path
                    d="M1.75 2.5h12.5a.25.25 0 0 1 .25.25v7.5a.25.25 0 0 1-.25.25H7.5a.75.75 0 0 0-.53.22L4.5 12.94V11.25a.75.75 0 0 0-.75-.75h-2a.25.25 0 0 1-.25-.25v-7.5a.25.25 0 0 1 .25-.25ZM14.25 1H1.75A1.75 1.75 0 0 0 0 2.75v7.5C0 11.216.784 12 1.75 12H3v1.543a1.457 1.457 0 0 0 2.487 1.03L8.06 12h6.19A1.75 1.75 0 0 0 16 10.25v-7.5A1.75 1.75 0 0 0 14.25 1Z"
                />
            </svg>
            Archived
        </button>
    </div>

    <div class="sidebar-section">
        <div class="sidebar-label">Repositories</div>
    </div>

    <div class="sidebar-section">
        <div class="sidebar-label">Codeowner Teams</div>
    </div>
</nav>

<style>
.sidebar {
    width: var(--sidebar-w);
    background: var(--canvas-default);
    border-right: 1px solid var(--border-default);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    overflow-y: auto;
    padding: 16px 0;
    gap: 24px;
}
.sidebar-section {
    display: flex;
    flex-direction: column;
    gap: 1px;
}
.sidebar-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
    padding: 0 16px;
    margin-bottom: 4px;
}
.sidebar-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px;
    cursor: pointer;
    color: var(--fg-muted);
    font-size: 14px;
    user-select: none;
    position: relative;
    border-radius: 0;
    text-decoration: none;
    background: none;
    border: none;
    font-family: inherit;
    text-align: left;
    width: 100%;
}
.sidebar-item:hover {
    background: var(--canvas-subtle);
    color: var(--fg-default);
}
.sidebar-item.active {
    color: var(--fg-default);
    font-weight: 600;
    background: var(--canvas-subtle);
}
.sidebar-item.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 4px;
    bottom: 4px;
    width: 2px;
    background: var(--accent-fg);
    border-radius: 0 2px 2px 0;
}
</style>
