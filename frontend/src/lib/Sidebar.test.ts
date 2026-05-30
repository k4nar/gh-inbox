import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.test-helpers.svelte";
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

    it("clicking a status pill includes that state", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: { options: OPTS, activeFilters: {}, onFiltersChange },
        });
        await fireEvent.click(screen.getByText("open"));
        expect(onFiltersChange).toHaveBeenCalledWith(
            expect.objectContaining({ states: { open: "include" } }),
        );
    });

    it("clicking an included status pill clears it (toggle off)", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: { states: { open: "include" } },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByText("open"));
        const called = onFiltersChange.mock.calls[0][0] as ActiveFilters;
        expect(called.states).toBeUndefined();
    });

    it("clicking the hide button excludes that state", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: { options: OPTS, activeFilters: {}, onFiltersChange },
        });
        await fireEvent.click(screen.getByLabelText("Hide merged"));
        expect(onFiltersChange).toHaveBeenCalledWith(
            expect.objectContaining({ states: { merged: "exclude" } }),
        );
    });

    it("clicking hide on an included status switches it to excluded", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: { states: { open: "include" } },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByLabelText("Hide open"));
        const called = onFiltersChange.mock.calls[0][0] as ActiveFilters;
        expect(called.states).toEqual({ open: "exclude" });
    });

    it("clicking an excluded status's hide button clears it (toggle off)", async () => {
        const onFiltersChange = vi.fn();
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: { states: { merged: "exclude" } },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByLabelText("Hide merged"));
        const called = onFiltersChange.mock.calls[0][0] as ActiveFilters;
        expect(called.states).toBeUndefined();
    });

    it("reflects include/exclude modes via data-mode on the pill", () => {
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: {
                    states: { open: "include", merged: "exclude" },
                },
            },
        });
        expect(
            screen
                .getByText("open")
                .closest(".status-pill")
                ?.getAttribute("data-mode"),
        ).toBe("include");
        expect(
            screen
                .getByText("merged")
                .closest(".status-pill")
                ?.getAttribute("data-mode"),
        ).toBe("exclude");
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

describe("Sidebar collapsible sections", () => {
    it("collapses Repositories by default", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        const header = screen.getByText("Repositories").closest("button");
        expect(header?.getAttribute("data-state")).toBe("closed");
    });

    it("stays collapsed when a repo filter is active", () => {
        render(Sidebar, {
            props: { options: OPTS, activeFilters: { repo: "acme/api" } },
        });
        const header = screen.getByText("Repositories").closest("button");
        expect(header?.getAttribute("data-state")).toBe("closed");
    });

    it("highlights the section label when its filter is active", () => {
        render(Sidebar, {
            props: { options: OPTS, activeFilters: { repo: "acme/api" } },
        });
        expect(screen.getByText("Repositories")).toHaveClass("active");
    });

    it("does not highlight the section label when its filter is inactive", () => {
        render(Sidebar, {
            props: { options: OPTS, activeFilters: { repo: "acme/api" } },
        });
        // Repositories is active, but Codeowner Teams / Authors are not.
        expect(screen.getByText("Codeowner Teams")).not.toHaveClass("active");
        expect(screen.getByText("Authors")).not.toHaveClass("active");
    });

    it("keeps the active item mounted while the section is collapsed", () => {
        render(Sidebar, {
            props: { options: OPTS, activeFilters: { repo: "acme/api" } },
        });
        const header = screen.getByText("Repositories").closest("button");
        expect(header?.getAttribute("data-state")).toBe("closed");
        // The active item is still rendered (CSS shows only it while collapsed).
        expect(screen.getByTitle("acme/api").getAttribute("data-state")).toBe(
            "active",
        );
    });

    it("clicking a collapsed section header expands it", async () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        const header = screen.getByText("Repositories").closest("button")!;
        expect(header.getAttribute("data-state")).toBe("closed");
        await fireEvent.click(header);
        expect(header.getAttribute("data-state")).toBe("open");
    });

    it("shows a count badge with the number of items in each section", () => {
        render(Sidebar, { props: { options: OPTS, activeFilters: {} } });
        // Repositories has 2 items (acme/api, acme/web)
        expect(
            screen.getByText("Repositories").closest("button")?.textContent,
        ).toContain("2");
        // Authors has 1 item (alice)
        expect(
            screen.getByText("Authors").closest("button")?.textContent,
        ).toContain("1");
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
                activeFilters: {
                    repo: "acme/api",
                    states: { open: "include" },
                },
                onFiltersChange,
            },
        });
        await fireEvent.click(screen.getByText("Clear filters"));
        expect(onFiltersChange).toHaveBeenCalledWith({});
    });

    it("shows the number of active filters as a count", () => {
        render(Sidebar, {
            props: {
                options: OPTS,
                activeFilters: {
                    repo: "acme/api",
                    author: "alice",
                    states: { open: "include", merged: "exclude" },
                },
            },
        });
        // repo + author + status (grouped) = 3
        const btn = screen.getByText("Clear filters").closest("button");
        expect(btn?.textContent).toContain("3");
    });
});
