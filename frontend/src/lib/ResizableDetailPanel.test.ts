import {
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ResizableDetailPanel from "./ResizableDetailPanel.svelte";

function setViewportWidth(width: number): void {
    Object.defineProperty(window, "innerWidth", {
        configurable: true,
        writable: true,
        value: width,
    });
}

function installLocalStorageMock(): void {
    const storage = new Map<string, string>();

    Object.defineProperty(window, "localStorage", {
        configurable: true,
        value: {
            getItem: (key: string) => storage.get(key) ?? null,
            setItem: (key: string, value: string) => {
                storage.set(key, value);
            },
            removeItem: (key: string) => {
                storage.delete(key);
            },
            clear: () => {
                storage.clear();
            },
        },
    });
}

function renderPanel() {
    const result = render(ResizableDetailPanel);
    const panel = result.container.querySelector(
        ".detail-panel",
    ) as HTMLDivElement;
    const layout = panel.parentElement as HTMLDivElement;

    Object.defineProperty(layout, "clientWidth", {
        configurable: true,
        value: 1400,
    });
    layout.getBoundingClientRect = () =>
        ({
            x: 0,
            y: 0,
            top: 0,
            left: 0,
            bottom: 900,
            right: 1400,
            width: 1400,
            height: 900,
            toJSON: () => ({}),
        }) as DOMRect;

    return { ...result, panel };
}

beforeEach(() => {
    cleanup();
    vi.restoreAllMocks();
    installLocalStorageMock();
    window.localStorage.clear();
    setViewportWidth(1400);

    HTMLElement.prototype.setPointerCapture = vi.fn();
    HTMLElement.prototype.releasePointerCapture = vi.fn();
    HTMLElement.prototype.hasPointerCapture = vi.fn(() => true);
});

afterEach(() => {
    cleanup();
});

describe("ResizableDetailPanel", () => {
    it("restores saved detail width from localStorage", async () => {
        window.localStorage.setItem("gh-inbox.pr-detail-width", "560");
        const { panel } = renderPanel();

        await waitFor(() => {
            expect(panel.style.width).toBe("560px");
        });
    });

    it("exposes separator semantics for the resizer", async () => {
        renderPanel();

        const resizer = await screen.findByRole("separator", {
            name: "Resize PR detail panel",
        });

        expect(resizer).toHaveAttribute("aria-controls", "pr-detail-panel");
        expect(resizer).toHaveAttribute("aria-orientation", "vertical");
        expect(resizer).toHaveAttribute("aria-valuemin", "400");
        expect(resizer).toHaveAttribute("aria-valuemax", "800");
        expect(resizer).toHaveAttribute("aria-valuenow", "520");
    });

    it("updates and persists detail width while dragging", async () => {
        const { panel } = renderPanel();
        const resizer = await screen.findByLabelText("Resize PR detail panel");

        await fireEvent.pointerDown(resizer, {
            pointerId: 1,
            clientX: 900,
        });
        await fireEvent.pointerMove(window, {
            pointerId: 1,
            clientX: 860,
        });
        await fireEvent.pointerUp(window, {
            pointerId: 1,
            clientX: 860,
        });

        expect(panel.style.width).toBe("540px");
        expect(window.localStorage.getItem("gh-inbox.pr-detail-width")).toBe(
            "540",
        );
    });
});
