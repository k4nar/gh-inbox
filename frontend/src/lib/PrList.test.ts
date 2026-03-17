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
        const statusbarCount = container.querySelector(".statusbar-count")!;
        expect(statusbarCount.textContent).toMatch(/2\s+PRs/);
        expect(statusbarCount.textContent).toContain("1 unread");
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

    it("renders pagination bar with page info", async () => {
        globalThis.fetch = mockFetch({
            items: MOCK_NOTIFICATIONS,
            total: 50,
            page: 1,
            per_page: 25,
        });

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        expect(screen.getByText(/Page 1 of 2/)).toBeInTheDocument();
        expect(
            screen.getByRole("button", { name: "Previous page" }),
        ).toBeDisabled();
        expect(screen.getByRole("button", { name: "Next page" })).toBeEnabled();
    });

    it("clicking Next page fetches page 2", async () => {
        globalThis.fetch = mockFetch({
            items: MOCK_NOTIFICATIONS,
            total: 50,
            page: 1,
            per_page: 25,
        });

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        globalThis.fetch = mockFetch({
            items: [makeItem({ id: "p2", title: "Page 2 PR" })],
            total: 50,
            page: 2,
            per_page: 25,
        });

        await fireEvent.click(
            screen.getByRole("button", { name: "Next page" }),
        );

        await waitFor(() => {
            expect(screen.getByText("Page 2 PR")).toBeInTheDocument();
        });
        expect(screen.getByText(/Page 2 of 2/)).toBeInTheDocument();
    });

    it("archive on last item of page navigates back", async () => {
        // Page 1 response
        globalThis.fetch = mockFetch({
            items: MOCK_NOTIFICATIONS,
            total: 27,
            page: 1,
            per_page: 25,
        });

        const { container } = render(PrList);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        // Navigate to page 2 (1 item)
        globalThis.fetch = mockFetch({
            items: [makeItem({ id: "last", title: "Last item" })],
            total: 27,
            page: 2,
            per_page: 25,
        });
        await fireEvent.click(
            screen.getByRole("button", { name: "Next page" }),
        );

        await waitFor(() => {
            expect(screen.getByText("Last item")).toBeInTheDocument();
        });

        // Archive the last item — after API call, refetch should go to page 1
        const archiveFetch = vi
            .fn()
            .mockResolvedValueOnce({
                ok: true,
                status: 204,
                json: () => Promise.resolve(undefined),
            })
            .mockResolvedValueOnce({
                ok: true,
                json: () =>
                    Promise.resolve({
                        items: MOCK_NOTIFICATIONS,
                        total: 26,
                        page: 1,
                        per_page: 25,
                    }),
            });
        globalThis.fetch = archiveFetch as unknown as typeof fetch;

        const archiveBtn = container.querySelector('button[title="Archive"]')!;
        await fireEvent.click(archiveBtn);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });
    });

    it("SSE refresh preserves current page", async () => {
        // Start on page 1
        globalThis.fetch = mockFetch({
            items: MOCK_NOTIFICATIONS,
            total: 50,
            page: 1,
            per_page: 25,
        });

        const { rerender } = render(PrList, {
            props: { refreshKey: 0 },
        });

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        // Navigate to page 2
        globalThis.fetch = mockFetch({
            items: [makeItem({ id: "p2", title: "Page 2 PR" })],
            total: 50,
            page: 2,
            per_page: 25,
        });
        await fireEvent.click(
            screen.getByRole("button", { name: "Next page" }),
        );

        await waitFor(() => {
            expect(screen.getByText("Page 2 PR")).toBeInTheDocument();
        });

        // SSE refresh (refreshKey changes) — should refetch page 2, not reset to 1
        const fetchSpy = mockFetch({
            items: [makeItem({ id: "p2-refreshed", title: "Refreshed P2" })],
            total: 50,
            page: 2,
            per_page: 25,
        });
        globalThis.fetch = fetchSpy;

        await rerender({ refreshKey: 1 });

        await waitFor(() => {
            expect(fetchSpy).toHaveBeenCalled();
        });

        // Verify the fetch URL included page=2
        const fetchUrl = (fetchSpy as ReturnType<typeof vi.fn>).mock
            .calls[0][0] as string;
        expect(fetchUrl).toContain("page=2");
    });

    it("hides pagination controls when total fits in one page", async () => {
        globalThis.fetch = mockFetch(paginatedResponse(MOCK_NOTIFICATIONS));

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        expect(
            screen.queryByRole("button", { name: "Previous page" }),
        ).not.toBeInTheDocument();
        expect(
            screen.queryByRole("button", { name: "Next page" }),
        ).not.toBeInTheDocument();
    });
});
