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
                    repo_counts: {},
                    team_counts: {},
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
                    repo_counts: {},
                    team_counts: {},
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
    repo_counts: { "acme/api": 3, "acme/web": 1 },
    team_counts: { "acme/platform": 2 },
};

describe("Sidebar filter lists", () => {
    it("renders repo items from options", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("acme/api")).toBeInTheDocument();
        expect(screen.getByTitle("acme/web")).toBeInTheDocument();
    });

    it("shows the full org/repo as label with count", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("acme/api").textContent).toContain("acme/api");
        expect(screen.getByTitle("acme/api").textContent).toContain("3");
    });

    it("renders team items from options", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("acme/platform")).toBeInTheDocument();
    });

    it("shows the full org/team as label with count", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("acme/platform").textContent).toContain(
            "acme/platform",
        );
        expect(screen.getByTitle("acme/platform").textContent).toContain("2");
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

describe("Sidebar status filter", () => {
    it("renders the Status section label", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByText("Status")).toBeInTheDocument();
    });

    it("renders a pill for each PR state", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        for (const state of ["open", "draft", "merged", "closed"]) {
            expect(screen.getByText(state)).toBeInTheDocument();
        }
    });

    it("clicking a status pill calls onFiltersChange with that state", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: { options: OPTS, activeFilters: {}, onFiltersChange },
        });
        await fireEvent.click(screen.getByText("open"));
        expect(onFiltersChange).toHaveBeenCalledWith(
            expect.objectContaining({ state: "open" }),
        );
    });

    it("clicking the active status pill clears it (toggle off)", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: { state: "open" },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByText("open"));
        const called = onFiltersChange.mock.calls[0][0] as ActiveFilters;
        expect(called.state).toBeUndefined();
    });

    it("active status pill has data-state=active", () => {
        render(Sidebar, {
            props: { options: OPTS, activeFilters: { state: "merged" } },
        });
        expect(
            screen
                .getByText("merged")
                .closest("button")
                ?.getAttribute("data-state"),
        ).toBe("active");
    });
});

describe("Sidebar authors filter", () => {
    it("renders the Authors section label", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByText("Authors")).toBeInTheDocument();
    });

    it("renders author items from options", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.getByTitle("alice")).toBeInTheDocument();
    });

    it("hides Authors section when authors is empty", () => {
        render(Sidebar, {
            props: { options: { ...OPTS, authors: [] }, activeFilters: {} },
        });
        expect(screen.queryByText("Authors")).not.toBeInTheDocument();
    });

    it("clicking an author item calls onFiltersChange with that author", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: { options: OPTS, activeFilters: {}, onFiltersChange },
        });
        await fireEvent.click(screen.getByTitle("alice"));
        expect(onFiltersChange).toHaveBeenCalledWith(
            expect.objectContaining({ author: "alice" }),
        );
    });

    it("clicking the active author item clears it (toggle off)", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: { author: "alice" },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByTitle("alice"));
        const called = onFiltersChange.mock.calls[0][0] as ActiveFilters;
        expect(called.author).toBeUndefined();
    });
});

describe("Sidebar clear filters", () => {
    it("hides the Clear filters link when no filter is active", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        expect(screen.queryByText("Clear filters")).not.toBeInTheDocument();
    });

    it("shows the Clear filters link when a filter is active", () => {
        render(Sidebar, {
            props: { options: OPTS, activeFilters: { repo: "acme/api" } },
        });
        expect(screen.getByText("Clear filters")).toBeInTheDocument();
    });

    it("clicking Clear filters calls onFiltersChange with empty filters", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: { repo: "acme/api", state: "open" },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByText("Clear filters"));
        expect(onFiltersChange).toHaveBeenCalledWith({});
    });
});
