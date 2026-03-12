import { render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
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
