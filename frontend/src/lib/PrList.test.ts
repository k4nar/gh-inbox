import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PrList from "./PrList.svelte";
import type { InboxItem } from "./types.ts";

function paginatedResponse(items: InboxItem[], total?: number): object {
    return { items, total: total ?? items.length, page: 1, per_page: 25 };
}

function makeItem(overrides: Partial<InboxItem> = {}): InboxItem {
    return {
        id: "n1",
        pr_id: 42,
        title: "Fix bug",
        repository: "owner/repo",
        reason: "review_requested",
        unread: true,
        archived: false,
        updated_at: "2025-01-01T00:00:00Z",
        author: "alice",
        pr_status: "open",
        new_commits: null,
        new_comments: null,
        teams: null,
        ...overrides,
    };
}

const MOCK_NOTIFICATIONS: InboxItem[] = [
    {
        id: "1",
        pr_id: 42,
        title: "Fix bug in parser",
        repository: "owner/repo",
        reason: "review_requested",
        unread: true,
        archived: false,
        updated_at: "2025-06-01T12:00:00Z",
        author: "alice",
        pr_status: "open",
        new_commits: null,
        new_comments: null,
        teams: null,
    },
    {
        id: "2",
        pr_id: 10,
        title: "Refactor auth module",
        repository: "org/api",
        reason: "mention",
        unread: false,
        archived: false,
        updated_at: "2025-05-30T08:00:00Z",
        author: "bob",
        pr_status: "draft",
        new_commits: 0,
        new_comments: [],
        teams: [],
    },
];

function mockFetch(data: unknown) {
    return vi.fn(() =>
        Promise.resolve({
            ok: true,
            json: () => Promise.resolve(data),
        }),
    ) as unknown as typeof fetch;
}

describe("PrList", () => {
    beforeEach(() => {
        vi.useFakeTimers({ now: new Date("2025-06-01T12:05:00Z") });
    });

    afterEach(() => {
        vi.useRealTimers();
        vi.restoreAllMocks();
    });

    it("renders empty state for inbox", async () => {
        globalThis.fetch = mockFetch(paginatedResponse([]));

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("All caught up!")).toBeInTheDocument();
        });
    });

    it("renders empty state for archived view", async () => {
        globalThis.fetch = mockFetch(paginatedResponse([]));

        render(PrList, { props: { currentView: "archived" } });

        await waitFor(() => {
            expect(
                screen.getByText("No archived notifications."),
            ).toBeInTheDocument();
        });
    });

    it("renders PR rows with repo, title, and PR number", async () => {
        globalThis.fetch = mockFetch(paginatedResponse(MOCK_NOTIFICATIONS));

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        expect(screen.getByText("#42")).toBeInTheDocument();
        expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        expect(screen.getByText("org/api")).toBeInTheDocument();
        expect(screen.getByText("#10")).toBeInTheDocument();
        expect(screen.getByText("Refactor auth module")).toBeInTheDocument();
    });

    it("shows unread dot for unread notifications", async () => {
        globalThis.fetch = mockFetch(paginatedResponse(MOCK_NOTIFICATIONS));

        const { container } = render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        const dots = container.querySelectorAll(".unread-dot");
        expect(dots).toHaveLength(2);
        // First notification is unread — dot should NOT have .read class
        expect(dots[0].classList.contains("read")).toBe(false);
        // Second notification is read — dot should have .read class
        expect(dots[1].classList.contains("read")).toBe(true);
    });

    it("displays correct count in header and statusbar", async () => {
        globalThis.fetch = mockFetch(paginatedResponse(MOCK_NOTIFICATIONS));

        const { container } = render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        // Header shows count with unread info
        const listCount = container.querySelector(".list-count")!;
        expect(listCount.textContent).toContain("2");
        expect(listCount.textContent).toContain("1 unread");

        // Statusbar shows count with unread info
        const statusbar = container.querySelector(".statusbar")!;
        expect(statusbar.textContent).toMatch(/2\s+PRs/);
        expect(statusbar.textContent).toContain("1 unread");
    });

    it("fetches with ?status= query param", async () => {
        globalThis.fetch = mockFetch(paginatedResponse([]));

        render(PrList, { props: { currentView: "archived" } });

        await waitFor(() => {
            expect(globalThis.fetch).toHaveBeenCalledWith(
                "/api/inbox?status=archived&page=1&per_page=25",
            );
        });
    });

    it("shows header title matching current view", async () => {
        globalThis.fetch = mockFetch(paginatedResponse([]));

        render(PrList, { props: { currentView: "archived" } });

        await waitFor(() => {
            expect(screen.getByText("Archived")).toBeInTheDocument();
        });
    });

    it("archive button removes notification from list", async () => {
        globalThis.fetch = mockFetch(paginatedResponse(MOCK_NOTIFICATIONS));

        const { container } = render(PrList);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        const archiveBtns = container.querySelectorAll(
            'button[title="Archive"]',
        );
        expect(archiveBtns).toHaveLength(2);

        await fireEvent.click(archiveBtns[0]);

        await waitFor(() => {
            expect(
                screen.queryByText("Fix bug in parser"),
            ).not.toBeInTheDocument();
        });
        expect(screen.getByText("Refactor auth module")).toBeInTheDocument();
    });

    it("refetches notifications when refreshKey changes", async () => {
        globalThis.fetch = mockFetch(paginatedResponse(MOCK_NOTIFICATIONS));

        const { rerender } = render(PrList, { props: { refreshKey: 0 } });

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        // Reset fetch mock to track new calls
        const fetchSpy = mockFetch(paginatedResponse(MOCK_NOTIFICATIONS));
        globalThis.fetch = fetchSpy;

        // Changing refreshKey should trigger a refetch
        await rerender({ refreshKey: 1 });

        await waitFor(() => {
            expect(fetchSpy).toHaveBeenCalled();
        });
    });

    it("clicking a PR marks it as read (optimistic UI)", async () => {
        globalThis.fetch = mockFetch(paginatedResponse(MOCK_NOTIFICATIONS));

        const onSelect = vi.fn();
        const { container } = render(PrList, { props: { onSelect } });

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        // First dot should be unread
        let dots = container.querySelectorAll(".unread-dot");
        expect(dots[0].classList.contains("read")).toBe(false);

        // Click the first PR row
        const firstRow = screen
            .getByText("Fix bug in parser")
            .closest(".pr-item")!;
        await fireEvent.click(firstRow);

        // Dot should now have .read class
        dots = container.querySelectorAll(".unread-dot");
        expect(dots[0].classList.contains("read")).toBe(true);

        // onSelect should have been called
        expect(onSelect).toHaveBeenCalled();
    });

    // New enriched data tests

    it("status icon shows open octicon for pr_status: open", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([makeItem({ pr_status: "open" })]),
        );

        render(PrList);

        await waitFor(() => {
            expect(
                screen.getByRole("img", { name: "open" }),
            ).toBeInTheDocument();
        });
    });

    it("status icon shows draft octicon for pr_status: draft", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([makeItem({ pr_status: "draft" })]),
        );

        render(PrList);

        await waitFor(() => {
            expect(
                screen.getByRole("img", { name: "draft" }),
            ).toBeInTheDocument();
        });
    });

    it("activity shows '✦ New pull request' when new_commits is null", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({ new_commits: null, new_comments: null }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("✦ New pull request")).toBeInTheDocument();
        });
    });

    it("activity shows quiet text when new_commits is 0 and new_comments is empty", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([makeItem({ new_commits: 0, new_comments: [] })]),
        );

        render(PrList);

        await waitFor(() => {
            expect(
                screen.getByText("No new activity since your last visit"),
            ).toBeInTheDocument();
        });
    });

    it("status icon shimmer is rendered when pr_status is null and pr_id is set", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([makeItem({ pr_status: null, pr_id: 42 })]),
        );

        const { container } = render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        const shimmer = container.querySelector(".status-icon-shimmer");
        expect(shimmer).toBeInTheDocument();
    });

    it("team badge renders @owner/team when teams is set", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([makeItem({ teams: ["owner/frontend"] })]),
        );

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("@owner/frontend")).toBeInTheDocument();
        });
    });
});
