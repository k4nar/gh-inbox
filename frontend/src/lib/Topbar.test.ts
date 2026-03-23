import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
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
    it("renders a theme select with System/Light/Dark options", () => {
        render(Topbar, { props: { syncStatus: "idle", theme: "system" } });
        const select = screen.getByRole("combobox", { name: /theme/i });
        expect(select).toBeInTheDocument();
        expect(
            screen.getByRole("option", { name: "System" }),
        ).toBeInTheDocument();
        expect(
            screen.getByRole("option", { name: "Light" }),
        ).toBeInTheDocument();
        expect(
            screen.getByRole("option", { name: "Dark" }),
        ).toBeInTheDocument();
    });

    it("select reflects the current theme prop", () => {
        render(Topbar, { props: { syncStatus: "idle", theme: "dark" } });
        const select = screen.getByRole("combobox", {
            name: /theme/i,
        }) as HTMLSelectElement;
        expect(select.value).toBe("dark");
    });

    it("calls onThemeChange when selection changes", async () => {
        const user = userEvent.setup();
        const onThemeChange = vi.fn();
        render(Topbar, {
            props: { syncStatus: "idle", theme: "system", onThemeChange },
        });
        const select = screen.getByRole("combobox", { name: /theme/i });
        await user.selectOptions(select, "light");
        expect(onThemeChange).toHaveBeenCalledWith("light");
    });
});
