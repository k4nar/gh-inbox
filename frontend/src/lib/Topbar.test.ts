import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Topbar from "./Topbar.svelte";

describe("Topbar", () => {
    it("shows 'synced' when status is idle", () => {
        render(Topbar, { props: { syncStatus: "idle" } });
        expect(screen.getByText("synced")).toBeInTheDocument();
    });

    it("shows 'syncing…' when status is syncing", () => {
        render(Topbar, { props: { syncStatus: "syncing" } });
        expect(screen.getByText("syncing…")).toBeInTheDocument();
    });

    it("shows 'sync error' when status is error", () => {
        render(Topbar, { props: { syncStatus: "error" } });
        expect(screen.getByText("sync error")).toBeInTheDocument();
    });

    it("sync dot has syncing class when syncing", () => {
        const { container } = render(Topbar, {
            props: { syncStatus: "syncing" },
        });
        const dot = container.querySelector(".sync-dot")!;
        expect(dot.classList.contains("syncing")).toBe(true);
    });

    it("sync dot has error class when error", () => {
        const { container } = render(Topbar, {
            props: { syncStatus: "error" },
        });
        const dot = container.querySelector(".sync-dot")!;
        expect(dot.classList.contains("error")).toBe(true);
    });
});

describe("Topbar theme dropdown", () => {
    it("renders a theme trigger button showing current theme label", () => {
        render(Topbar, { props: { syncStatus: "idle", theme: "system" } });
        const trigger = screen.getByRole("button", { name: /theme/i });
        expect(trigger).toBeInTheDocument();
        expect(trigger).toHaveTextContent("Theme · System");
    });

    it("trigger reflects the current theme prop", () => {
        render(Topbar, { props: { syncStatus: "idle", theme: "dark" } });
        const trigger = screen.getByRole("button", { name: /theme/i });
        expect(trigger).toHaveTextContent("Theme · Dark");
    });

    it("calls onThemeChange when an item is selected", async () => {
        const onThemeChange = vi.fn();
        render(Topbar, {
            props: { syncStatus: "idle", theme: "system", onThemeChange },
        });
        const trigger = screen.getByRole("button", { name: /theme/i });
        fireEvent.pointerDown(trigger, {
            button: 0,
            ctrlKey: false,
            pointerId: 1,
            pointerType: "mouse",
        });
        const lightItem = await screen.findByRole("option", {
            name: "Light",
            hidden: true,
        });
        fireEvent.pointerUp(lightItem, {
            button: 0,
            pointerId: 1,
            pointerType: "mouse",
        });
        expect(onThemeChange).toHaveBeenCalledWith("light");
    });
});
