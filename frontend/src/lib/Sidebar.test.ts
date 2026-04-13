import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import type { ActiveFilters, FilterOptions } from "./types.ts";

describe("Sidebar", () => {
    it("renders the Inbox nav item", () => {
        render(Sidebar);
        expect(screen.getByText("Inbox")).toBeInTheDocument();
    });

    it("renders the Archived nav item", () => {
        render(Sidebar);
        expect(screen.getByText("Archived")).toBeInTheDocument();
    });

    it("renders the Repositories section label", () => {
        render(Sidebar, {
            props: {
                options: {
                    repos: ["acme/api"],
                    orgs: [],
                    teams: [],
                    authors: [],
                },
            },
        });
        expect(screen.getByText("Repositories")).toBeInTheDocument();
    });

    it("renders the Codeowner Teams section label", () => {
        render(Sidebar, {
            props: {
                options: {
                    repos: [],
                    orgs: [],
                    teams: ["acme/platform"],
                    authors: [],
                },
            },
        });
        expect(screen.getByText("Codeowner Teams")).toBeInTheDocument();
    });

    it("shows active state on Inbox when currentView is inbox", () => {
        render(Sidebar, { props: { currentView: "inbox" } });
        const inboxBtn = screen.getByText("Inbox").closest("button")!;
        const archivedBtn = screen.getByText("Archived").closest("button")!;
        expect(inboxBtn.getAttribute("data-state")).toBe("active");
        expect(archivedBtn.getAttribute("data-state")).toBe("inactive");
    });

    it("shows active state on Archived when currentView is archived", () => {
        render(Sidebar, { props: { currentView: "archived" } });
        const inboxBtn = screen.getByText("Inbox").closest("button")!;
        const archivedBtn = screen.getByText("Archived").closest("button")!;
        expect(inboxBtn.getAttribute("data-state")).toBe("inactive");
        expect(archivedBtn.getAttribute("data-state")).toBe("active");
    });

    it("calls onViewChange with 'archived' when Archived is clicked", async () => {
        const onViewChange = vi.fn();
        render(Sidebar, { props: { currentView: "inbox", onViewChange } });
        await fireEvent.click(screen.getByText("Archived"));
        expect(onViewChange).toHaveBeenCalledWith("archived");
    });

    it("calls onViewChange with 'inbox' when Inbox is clicked", async () => {
        const onViewChange = vi.fn();
        render(Sidebar, { props: { currentView: "archived", onViewChange } });
        await fireEvent.click(screen.getByText("Inbox"));
        expect(onViewChange).toHaveBeenCalledWith("inbox");
    });
});

const OPTS: FilterOptions = {
    repos: ["acme/api", "acme/web"],
    orgs: ["acme"],
    teams: ["acme/platform"],
    authors: ["alice"],
};

describe("Sidebar filter lists", () => {
    it("renders repo items from options", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("acme/api")).toBeInTheDocument();
        expect(screen.getByTitle("acme/web")).toBeInTheDocument();
    });

    it("shows only the repo part as label", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("acme/api").textContent?.trim()).toBe("api");
    });

    it("renders team items from options", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("acme/platform")).toBeInTheDocument();
    });

    it("shows only the slug part as team label", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("acme/platform").textContent?.trim()).toBe(
            "platform",
        );
    });

    it("hides Repositories section when repos is empty", () => {
        render(Sidebar, {
            props: { options: { ...OPTS, repos: [] }, activeFilters: {} },
        });
        expect(screen.queryByText("Repositories")).not.toBeInTheDocument();
    });

    it("hides Codeowner Teams section when teams is empty", () => {
        render(Sidebar, {
            props: { options: { ...OPTS, teams: [] }, activeFilters: {} },
        });
        expect(screen.queryByText("Codeowner Teams")).not.toBeInTheDocument();
    });

    it("active repo item has data-state=active", () => {
        render(Sidebar, {
            props: { options: OPTS, activeFilters: { repo: "acme/api" } },
        });
        expect(screen.getByTitle("acme/api").getAttribute("data-state")).toBe(
            "active",
        );
    });

    it("inactive repo item has data-state=inactive", () => {
        render(Sidebar, {
            props: { options: OPTS, activeFilters: { repo: "acme/api" } },
        });
        expect(screen.getByTitle("acme/web").getAttribute("data-state")).toBe(
            "inactive",
        );
    });

    it("clicking a repo item calls onFiltersChange with that repo", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: { options: OPTS, activeFilters: {}, onFiltersChange },
        });
        await fireEvent.click(screen.getByTitle("acme/api"));
        expect(onFiltersChange).toHaveBeenCalledWith(
            expect.objectContaining({ repo: "acme/api" }),
        );
    });

    it("clicking the active repo item clears it (toggle off)", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: { repo: "acme/api" },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByTitle("acme/api"));
        const called = onFiltersChange.mock.calls[0][0] as ActiveFilters;
        expect(called.repo).toBeUndefined();
    });

    it("clicking a team item calls onFiltersChange with that team", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: { options: OPTS, activeFilters: {}, onFiltersChange },
        });
        await fireEvent.click(screen.getByTitle("acme/platform"));
        expect(onFiltersChange).toHaveBeenCalledWith(
            expect.objectContaining({ team: "acme/platform" }),
        );
    });

    it("clicking the active team item clears it (toggle off)", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: { team: "acme/platform" },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByTitle("acme/platform"));
        const called = onFiltersChange.mock.calls[0][0] as ActiveFilters;
        expect(called.team).toBeUndefined();
    });
});
