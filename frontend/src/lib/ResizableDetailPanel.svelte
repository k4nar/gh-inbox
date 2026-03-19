<script lang="ts">
import type { Snippet } from "svelte";
import { onMount } from "svelte";

const DEFAULT_STORAGE_KEY = "gh-inbox.pr-detail-width";
const DEFAULT_PANEL_ID = "pr-detail-panel";
const DEFAULT_RESIZE_LABEL = "Resize PR detail panel";
const DEFAULT_DETAIL_WIDTH = 520;
const KEYBOARD_RESIZE_STEP = 24;
const DETAIL_MIN_WIDTH_CSS_VAR = "--pr-detail-min-w";
const LIST_MIN_WIDTH_CSS_VAR = "--pr-list-min-w";
const SIDEBAR_WIDTH_CSS_VAR = "--sidebar-w";
const FALLBACK_DETAIL_MIN_WIDTH = 400;
const FALLBACK_LIST_MIN_WIDTH = 360;
const FALLBACK_SIDEBAR_WIDTH = 240;

let {
    storageKey = DEFAULT_STORAGE_KEY,
    panelId = DEFAULT_PANEL_ID,
    resizeLabel = DEFAULT_RESIZE_LABEL,
    children,
}: {
    storageKey?: string;
    panelId?: string;
    resizeLabel?: string;
    children?: Snippet;
} = $props();

let detailWidth = $state(DEFAULT_DETAIL_WIDTH);
let panelEl = $state<HTMLDivElement | null>(null);

function getLayoutEl(): HTMLElement | null {
    return panelEl?.parentElement ?? null;
}

function getStorage(): Pick<Storage, "getItem" | "setItem"> | null {
    const storage = window.localStorage;
    if (
        typeof storage?.getItem !== "function" ||
        typeof storage?.setItem !== "function"
    ) {
        return null;
    }
    return storage;
}

function persistDetailWidth(width: number): void {
    getStorage()?.setItem(storageKey, String(Math.round(width)));
}

function readCssPixelValue(variableName: string, fallback: number): number {
    const styleTarget = getLayoutEl() ?? document.documentElement;
    const rawValue =
        getComputedStyle(styleTarget).getPropertyValue(variableName);
    const parsedValue = Number.parseFloat(rawValue);
    return Number.isFinite(parsedValue) ? parsedValue : fallback;
}

function getMinDetailWidth(): number {
    return readCssPixelValue(
        DETAIL_MIN_WIDTH_CSS_VAR,
        FALLBACK_DETAIL_MIN_WIDTH,
    );
}

function getMinListWidth(): number {
    return readCssPixelValue(LIST_MIN_WIDTH_CSS_VAR, FALLBACK_LIST_MIN_WIDTH);
}

function getSidebarWidth(): number {
    return readCssPixelValue(SIDEBAR_WIDTH_CSS_VAR, FALLBACK_SIDEBAR_WIDTH);
}

function getMaxDetailWidth(): number {
    const layoutEl = getLayoutEl();
    const availableWidth =
        layoutEl && layoutEl.clientWidth > 0
            ? layoutEl.clientWidth
            : window.innerWidth;
    return Math.max(
        getMinDetailWidth(),
        availableWidth - getSidebarWidth() - getMinListWidth(),
    );
}

function clampDetailWidth(width: number): number {
    return Math.min(Math.max(width, getMinDetailWidth()), getMaxDetailWidth());
}

function setDetailWidth(width: number): void {
    detailWidth = clampDetailWidth(width);
    persistDetailWidth(detailWidth);
}

function handleResizeStart(event: PointerEvent): void {
    const layoutEl = getLayoutEl();
    if (!layoutEl) return;

    const handle = event.currentTarget as HTMLElement;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;

    const updateWidth = (clientX: number) => {
        const nextWidth = layoutEl.getBoundingClientRect().right - clientX;
        setDetailWidth(nextWidth);
    };

    const stopResize = () => {
        window.removeEventListener("pointermove", onPointerMove);
        window.removeEventListener("pointerup", stopResize);
        window.removeEventListener("pointercancel", stopResize);
        if (handle.hasPointerCapture(event.pointerId)) {
            handle.releasePointerCapture(event.pointerId);
        }
        document.body.style.cursor = previousCursor;
        document.body.style.userSelect = previousUserSelect;
    };

    const onPointerMove = (moveEvent: PointerEvent) => {
        updateWidth(moveEvent.clientX);
    };

    handle.setPointerCapture(event.pointerId);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    updateWidth(event.clientX);

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", stopResize);
    window.addEventListener("pointercancel", stopResize);
}

function handleResizeKeydown(event: KeyboardEvent): void {
    if (event.key === "ArrowLeft") {
        event.preventDefault();
        setDetailWidth(detailWidth + KEYBOARD_RESIZE_STEP);
    } else if (event.key === "ArrowRight") {
        event.preventDefault();
        setDetailWidth(detailWidth - KEYBOARD_RESIZE_STEP);
    } else if (event.key === "Home") {
        event.preventDefault();
        setDetailWidth(getMinDetailWidth());
    } else if (event.key === "End") {
        event.preventDefault();
        setDetailWidth(getMaxDetailWidth());
    }
}

onMount(() => {
    const storedWidth = getStorage()?.getItem(storageKey) ?? null;
    const parsedWidth = storedWidth ? Number.parseInt(storedWidth, 10) : NaN;
    if (Number.isFinite(parsedWidth)) {
        detailWidth = clampDetailWidth(parsedWidth);
    }

    const handleWindowResize = () => {
        detailWidth = clampDetailWidth(detailWidth);
    };

    window.addEventListener("resize", handleWindowResize);

    return () => {
        window.removeEventListener("resize", handleWindowResize);
    };
});
</script>

<div
    id={panelId}
    class="detail-panel"
    style:width={`${detailWidth}px`}
    bind:this={panelEl}
>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
        class="detail-resizer"
        role="separator"
        aria-label={resizeLabel}
        aria-controls={panelId}
        aria-orientation="vertical"
        aria-valuemin={Math.round(getMinDetailWidth())}
        aria-valuemax={Math.round(getMaxDetailWidth())}
        aria-valuenow={Math.round(detailWidth)}
        tabindex="0"
        onpointerdown={handleResizeStart}
        onkeydown={handleResizeKeydown}
    ></div>
    {@render children?.()}
</div>

<style>
.detail-panel {
    position: relative;
    display: flex;
    flex: 0 0 auto;
    min-width: var(--pr-detail-min-w);
}

.detail-resizer {
    position: absolute;
    left: -4px;
    top: 0;
    bottom: 0;
    width: 8px;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: col-resize;
    touch-action: none;
    z-index: 2;
}

.detail-resizer::before {
    content: "";
    position: absolute;
    left: 3px;
    top: 0;
    bottom: 0;
    width: 2px;
    background: transparent;
    transition: background 120ms ease;
}

.detail-resizer:hover::before,
.detail-resizer:focus-visible::before {
    background: var(--accent-fg);
}

.detail-resizer:focus-visible {
    outline: 2px solid var(--accent-fg);
    outline-offset: -2px;
}
</style>
