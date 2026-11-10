import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { tick } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PrList from "./PrList.test-helpers.svelte";
import { onPrInfoUpdated } from "./sse.svelte.ts";
import { DEFAULT_PER_PAGE, type InboxItem } from "./types.ts";

vi.mock("./sse.svelte.ts", async (importOriginal) => {
    const actual = await importOriginal<typeof import("./sse.svelte.ts")>();
    return {
        ...actual,
        onPrInfoUpdated: vi.fn(actual.onPrInfoUpdated),
    };
});

function paginatedResponse(items: InboxItem[], total?: number): object {
    return {
        items,
        total: total ?? items.length,
        page: 1,
        per_page: DEFAULT_PER_PAGE,
    };
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
        author_avatar_url: null,
        pr_status: "open",
        ci_status: null,
        new_commits: null,
        new_comments: null,
        teams: null,
        new_reviews: null,
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
        author_avatar_url: null,
        pr_status: "open",
        ci_status: null,
        new_commits: null,
        new_comments: null,
        teams: null,
        new_reviews: null,
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
        author_avatar_url: null,
        pr_status: "draft",
        ci_status: null,
        new_commits: 0,
        new_comments: [],
        teams: [],
        new_reviews: null,
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

    it("shows a filter-aware empty state with a clear button when filters are active", async () => {
        globalThis.fetch = mockFetch(paginatedResponse([]));
        const onClearFilters = vi.fn();

        render(PrList, {
            props: { activeFilters: { repo: "acme/api" }, onClearFilters },
        });

        await waitFor(() => {
            expect(
                screen.getByText("No pull requests match your filters."),
            ).toBeInTheDocument();
        });
        await fireEvent.click(
            screen.getByRole("button", { name: /clear filters/i }),
        );
        expect(onClearFilters).toHaveBeenCalled();
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
                `/api/inbox?status=archived&page=1&per_page=${DEFAULT_PER_PAGE}`,
                expect.any(Object),
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
            'button[aria-label="Archive"]',
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

    it("fetches exactly once per filter change (no self-retriggering effect)", async () => {
        globalThis.fetch = mockFetch(paginatedResponse([]));

        const { rerender } = render(PrList, { props: { activeFilters: {} } });
        await waitFor(() => {
            expect(globalThis.fetch).toHaveBeenCalledTimes(1);
        });

        await rerender({ activeFilters: { repo: "acme/api" } });
        await waitFor(() => {
            expect(globalThis.fetch).toHaveBeenCalledTimes(2);
        });

        // Let any spurious re-run of the refetch effect fire before asserting.
        await vi.advanceTimersByTimeAsync(50);
        expect(globalThis.fetch).toHaveBeenCalledTimes(2);
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

    it("activity renders nothing while not yet enriched (fields absent from inbox API)", async () => {
        // The inbox API never includes activity fields — they arrive as
        // undefined and are only filled in by the pr:info_updated SSE event.
        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    new_commits: undefined,
                    new_comments: undefined,
                    new_reviews: undefined,
                }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });
        expect(
            screen.queryByText("✦ New pull request"),
        ).not.toBeInTheDocument();
        expect(
            screen.queryByText("No new activity since your last visit"),
        ).not.toBeInTheDocument();
    });

    it("SSE event with null activity marks the item as a new pull request", async () => {
        let capturedInfoCallback: ((data: unknown) => void) | null = null;
        vi.mocked(onPrInfoUpdated).mockImplementation((cb) => {
            capturedInfoCallback = cb as (data: unknown) => void;
            return () => {};
        });

        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    new_commits: undefined,
                    new_comments: undefined,
                    new_reviews: undefined,
                }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        // Backend sends null for all three = PR never viewed (first visit).
        capturedInfoCallback!({
            pr_id: 42,
            repository: "owner/repo",
            author: "alice",
            pr_status: "open",
            ci_status: null,
            new_commits: null,
            new_comments: null,
            new_reviews: null,
            teams: null,
        });

        await waitFor(() => {
            expect(screen.getByText("✦ New pull request")).toBeInTheDocument();
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
            total: DEFAULT_PER_PAGE * 2,
            page: 1,
            per_page: DEFAULT_PER_PAGE,
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
            total: DEFAULT_PER_PAGE * 2,
            page: 1,
            per_page: DEFAULT_PER_PAGE,
        });

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        globalThis.fetch = mockFetch({
            items: [makeItem({ id: "p2", title: "Page 2 PR" })],
            total: DEFAULT_PER_PAGE * 2,
            page: 2,
            per_page: DEFAULT_PER_PAGE,
        });

        await fireEvent.click(
            screen.getByRole("button", { name: "Next page" }),
        );

        await waitFor(() => {
            expect(screen.getByText("Page 2 PR")).toBeInTheDocument();
        });
        expect(screen.getByText(/Page 2 of 2/)).toBeInTheDocument();
        expect(
            screen.getByRole("button", { name: "Next page" }),
        ).toBeDisabled();
        expect(
            screen.getByRole("button", { name: "Previous page" }),
        ).toBeEnabled();
    });

    it("archive on last item of page navigates back", async () => {
        // Page 1 response
        globalThis.fetch = mockFetch({
            items: MOCK_NOTIFICATIONS,
            total: DEFAULT_PER_PAGE + 1,
            page: 1,
            per_page: DEFAULT_PER_PAGE,
        });

        const { container } = render(PrList);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        // Navigate to page 2 (1 item)
        globalThis.fetch = mockFetch({
            items: [makeItem({ id: "last", title: "Last item" })],
            total: DEFAULT_PER_PAGE + 1,
            page: 2,
            per_page: DEFAULT_PER_PAGE,
        });
        await fireEvent.click(
            screen.getByRole("button", { name: "Next page" }),
        );

        await waitFor(() => {
            expect(screen.getByText("Last item")).toBeInTheDocument();
        });

        // Archive the last item — after API call, refetch should go to page 1
        const archiveFetch = vi.fn((input: string | URL | Request) => {
            const url = String(input);

            if (url.includes("/archive")) {
                return Promise.resolve({
                    ok: true,
                    status: 204,
                    json: () => Promise.resolve(undefined),
                });
            }

            return Promise.resolve({
                ok: true,
                status: 200,
                json: () =>
                    Promise.resolve({
                        items: MOCK_NOTIFICATIONS,
                        total: DEFAULT_PER_PAGE,
                        page: 1,
                        per_page: DEFAULT_PER_PAGE,
                    }),
            });
        });
        globalThis.fetch = archiveFetch as unknown as typeof fetch;

        const archiveBtn = container.querySelector(
            'button[aria-label="Archive"]',
        )!;
        await fireEvent.click(archiveBtn);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });
    });

    it("SSE refresh preserves current page", async () => {
        // Start on page 1
        globalThis.fetch = mockFetch({
            items: MOCK_NOTIFICATIONS,
            total: DEFAULT_PER_PAGE * 2,
            page: 1,
            per_page: DEFAULT_PER_PAGE,
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
            total: DEFAULT_PER_PAGE * 2,
            page: 2,
            per_page: DEFAULT_PER_PAGE,
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
            total: DEFAULT_PER_PAGE * 2,
            page: 2,
            per_page: DEFAULT_PER_PAGE,
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

    it("activitySentence includes approval text for APPROVED review", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    new_commits: 0,
                    new_comments: [],
                    new_reviews: [{ reviewer: "alice", state: "APPROVED" }],
                }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("alice approved")).toBeInTheDocument();
        });
    });

    it("activitySentence includes changes requested text for CHANGES_REQUESTED review", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    new_commits: 0,
                    new_comments: [],
                    new_reviews: [
                        { reviewer: "bob", state: "CHANGES_REQUESTED" },
                    ],
                }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            expect(
                screen.getByText("bob requested changes"),
            ).toBeInTheDocument();
        });
    });

    it("activitySentence includes both commits and approval when both present", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    new_commits: 2,
                    new_comments: [],
                    new_reviews: [{ reviewer: "alice", state: "APPROVED" }],
                }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            const activityEl = screen.getByText(
                /alice pushed 2 commits · alice approved/,
            );
            expect(activityEl).toBeInTheDocument();
        });
    });

    it("activitySentence shows dismissed for DISMISSED review", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    new_commits: 0,
                    new_comments: [],
                    new_reviews: [{ reviewer: "carol", state: "DISMISSED" }],
                }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            expect(
                screen.getByText("carol's review dismissed"),
            ).toBeInTheDocument();
        });
    });

    it("onPrInfoUpdated SSE handler sets ci_status to a non-null value", async () => {
        let capturedInfoCallback: ((data: unknown) => void) | null = null;
        vi.mocked(onPrInfoUpdated).mockImplementation((cb) => {
            capturedInfoCallback = cb as (data: unknown) => void;
            return () => {};
        });

        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    id: "n1",
                    pr_id: 42,
                    repository: "owner/repo",
                    ci_status: null,
                }),
            ]),
        );

        const { container } = render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        const row = container.querySelector("[data-notif-id='n1']")!;
        expect(row.getAttribute("data-ci-status")).toBe("");

        expect(capturedInfoCallback).not.toBeNull();
        capturedInfoCallback!({
            pr_id: 42,
            repository: "owner/repo",
            author: "alice",
            pr_status: "open",
            ci_status: "success",
            new_commits: null,
            new_comments: null,
            new_reviews: null,
            teams: null,
        });

        await waitFor(() => {
            expect(row.getAttribute("data-ci-status")).toBe("success");
        });
    });

    it("onPrInfoUpdated SSE handler clears ci_status when null is received", async () => {
        // Regression: the old guard `if (data.ci_status !== null)` prevented null
        // from clearing a previously-set ci_status value.
        let capturedInfoCallback: ((data: unknown) => void) | null = null;
        vi.mocked(onPrInfoUpdated).mockImplementation((cb) => {
            capturedInfoCallback = cb as (data: unknown) => void;
            return () => {};
        });

        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    id: "n1",
                    pr_id: 42,
                    repository: "owner/repo",
                    ci_status: "failure",
                }),
            ]),
        );

        const { container } = render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        const row = container.querySelector("[data-notif-id='n1']")!;
        expect(row.getAttribute("data-ci-status")).toBe("failure");

        expect(capturedInfoCallback).not.toBeNull();
        capturedInfoCallback!({
            pr_id: 42,
            repository: "owner/repo",
            author: "alice",
            pr_status: "open",
            ci_status: null,
            new_commits: null,
            new_comments: null,
            new_reviews: null,
            teams: null,
        });

        await waitFor(() => {
            expect(row.getAttribute("data-ci-status")).toBe("");
        });
    });

    it("activitySentence does not include reviews when new_reviews is null", async () => {
        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    new_commits: 0,
                    new_comments: [],
                    new_reviews: null,
                }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            expect(
                screen.getByText("No new activity since your last visit"),
            ).toBeInTheDocument();
        });

        expect(screen.queryByText(/approved/)).not.toBeInTheDocument();
        expect(screen.queryByText(/requested changes/)).not.toBeInTheDocument();
    });

    it("onPrInfoUpdated SSE handler updates new_reviews on matching item", async () => {
        // Capture the callback registered by PrList via a mock
        let capturedInfoCallback: ((data: unknown) => void) | null = null;
        vi.mocked(onPrInfoUpdated).mockImplementation((cb) => {
            capturedInfoCallback = cb as (data: unknown) => void;
            return () => {};
        });

        globalThis.fetch = mockFetch(
            paginatedResponse([
                makeItem({
                    id: "n1",
                    pr_id: 42,
                    repository: "owner/repo",
                    author: "alice",
                    new_commits: 0,
                    new_comments: [],
                    new_reviews: null,
                }),
            ]),
        );

        render(PrList);

        await waitFor(() => {
            expect(screen.getByText("owner/repo")).toBeInTheDocument();
        });

        // Initially no review activity (new_reviews: null means first visit, but
        // new_commits is 0 so it shows "No new activity")
        expect(
            screen.getByText("No new activity since your last visit"),
        ).toBeInTheDocument();

        // Fire the SSE payload with new_reviews
        expect(capturedInfoCallback).not.toBeNull();
        capturedInfoCallback!({
            pr_id: 42,
            repository: "owner/repo",
            author: "alice",
            pr_status: "open",
            new_commits: 0,
            new_comments: [],
            new_reviews: [{ reviewer: "alice", state: "APPROVED" }],
        });

        await waitFor(() => {
            expect(screen.getByText("alice approved")).toBeInTheDocument();
        });
    });
});

describe("PrList filter params", () => {
    it("fetch URL includes repo filter param when activeFilters.repo is set", async () => {
        const fetchSpy = vi.fn(() =>
            Promise.resolve(Response.json(paginatedResponse([]))),
        );
        globalThis.fetch = fetchSpy as unknown as typeof fetch;
        render(PrList, {
            props: { activeFilters: { repo: "acme/api" } },
        });
        await waitFor(() => {
            const urls = (fetchSpy.mock.calls as unknown as [unknown][]).map(
                ([url]) => String(url),
            );
            expect(urls.find((u) => u.includes("/api/inbox"))).toContain(
                "repo=acme%2Fapi",
            );
        });
    });

    it("fetch URL includes author filter param", async () => {
        const fetchSpy = vi.fn(() =>
            Promise.resolve(Response.json(paginatedResponse([]))),
        );
        globalThis.fetch = fetchSpy as unknown as typeof fetch;
        render(PrList, {
            props: { activeFilters: { author: "alice" } },
        });
        await waitFor(() => {
            const urls = (fetchSpy.mock.calls as unknown as [unknown][]).map(
                ([url]) => String(url),
            );
            expect(urls.find((u) => u.includes("/api/inbox"))).toContain(
                "author=alice",
            );
        });
    });

    it("fetch URL includes state_include param for included statuses", async () => {
        const fetchSpy = vi.fn(() =>
            Promise.resolve(Response.json(paginatedResponse([]))),
        );
        globalThis.fetch = fetchSpy as unknown as typeof fetch;
        render(PrList, {
            props: {
                activeFilters: {
                    states: { open: "include", draft: "include" },
                },
            },
        });
        await waitFor(() => {
            const url = (fetchSpy.mock.calls as unknown as [unknown][])
                .map(([u]) => String(u))
                .find((u) => u.includes("/api/inbox"));
            expect(url).toContain("state_include=open%2Cdraft");
        });
    });

    it("fetch URL includes state_exclude param for hidden statuses", async () => {
        const fetchSpy = vi.fn(() =>
            Promise.resolve(Response.json(paginatedResponse([]))),
        );
        globalThis.fetch = fetchSpy as unknown as typeof fetch;
        render(PrList, {
            props: { activeFilters: { states: { merged: "exclude" } } },
        });
        await waitFor(() => {
            const url = (fetchSpy.mock.calls as unknown as [unknown][])
                .map(([u]) => String(u))
                .find((u) => u.includes("/api/inbox"));
            expect(url).toContain("state_exclude=merged");
        });
    });
});

describe("PrList keyboard shortcuts", () => {
    afterEach(() => {
        vi.restoreAllMocks();
    });

    function rowFor(title: string): Element {
        return screen.getByText(title).closest(".pr-item")!;
    }

    function fetchCalls(spy: ReturnType<typeof vi.fn>): string[] {
        return (spy.mock.calls as unknown as [unknown, RequestInit?][])
            .filter(([, init]) => init?.method === "POST")
            .map(([u]) => String(u));
    }

    // A fake /api/inbox that honours archives and pages; with holdGets,
    // list GETs wait until release() so tests can interleave requests.
    function statefulInbox(
        pages: InboxItem[][],
        opts: { holdGets?: boolean; total?: number } = {},
    ) {
        const archived = new Set<string>();
        const calls: { url: string; method: string }[] = [];
        let held: (() => void)[] = [];
        const fetchSpy = vi.fn((input: unknown, init?: RequestInit) => {
            const url = String(input);
            const method = init?.method ?? "GET";
            calls.push({ url, method });
            const archiveMatch = url.match(/\/api\/inbox\/([^/]+)\/archive$/);
            if (archiveMatch) archived.add(archiveMatch[1]);
            if (method === "POST") {
                return Promise.resolve(new Response(null, { status: 204 }));
            }
            const page = Number(
                new URL(url, "http://x").searchParams.get("page"),
            );
            const respond = () => {
                const items = (pages[page - 1] ?? []).filter(
                    (n) => !archived.has(n.id),
                );
                return Response.json({
                    items,
                    total: opts.total ?? items.length,
                    page,
                    per_page: DEFAULT_PER_PAGE,
                });
            };
            if (!opts.holdGets) return Promise.resolve(respond());
            return new Promise<Response>((resolve) => {
                held.push(() => resolve(respond()));
            });
        });
        return {
            fetchSpy,
            release() {
                const pending = held;
                held = [];
                for (const r of pending) r();
            },
            posts: () =>
                calls.filter((c) => c.method === "POST").map((c) => c.url),
            inboxGets: () =>
                calls.filter(
                    (c) => c.method === "GET" && c.url.includes("/api/inbox?"),
                ).length,
            pageGets: (n: number) =>
                calls.filter(
                    (c) => c.method === "GET" && c.url.includes(`page=${n}&`),
                ).length,
        };
    }

    async function renderStateful(
        server: ReturnType<typeof statefulInbox>,
        props: Record<string, unknown> = {},
    ) {
        globalThis.fetch = server.fetchSpy as unknown as typeof fetch;
        const result = render(PrList, { props });
        // With held GETs the initial load needs releasing until rows show.
        await waitFor(() => {
            server.release();
            expect(document.querySelector(".pr-item")).not.toBeNull();
        });
        return result;
    }

    async function renderList(props: Record<string, unknown> = {}) {
        const fetchSpy = vi.fn(() =>
            Promise.resolve(
                Response.json(paginatedResponse(MOCK_NOTIFICATIONS)),
            ),
        );
        globalThis.fetch = fetchSpy as unknown as typeof fetch;
        const result = render(PrList, { props });
        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });
        return { ...result, fetchSpy };
    }

    it("j and k move the cursor without opening the PR", async () => {
        const onSelect = vi.fn();
        await renderList({ onSelect });

        await fireEvent.keyDown(document.body, { key: "j" });
        expect(rowFor("Fix bug in parser")).toHaveClass("cursor");

        await fireEvent.keyDown(document.body, { key: "j" });
        expect(rowFor("Refactor auth module")).toHaveClass("cursor");
        expect(rowFor("Fix bug in parser")).not.toHaveClass("cursor");

        await fireEvent.keyDown(document.body, { key: "k" });
        expect(rowFor("Fix bug in parser")).toHaveClass("cursor");

        expect(onSelect).not.toHaveBeenCalled();
    });

    it("arrow keys move the cursor like j and k", async () => {
        await renderList();

        await fireEvent.keyDown(document.body, { key: "ArrowDown" });
        await fireEvent.keyDown(document.body, { key: "ArrowDown" });
        expect(rowFor("Refactor auth module")).toHaveClass("cursor");

        await fireEvent.keyDown(document.body, { key: "ArrowUp" });
        expect(rowFor("Fix bug in parser")).toHaveClass("cursor");
    });

    it("arrow keys open the adjacent PR when the detail panel is open", async () => {
        const onSelect = vi.fn();
        await renderList({ onSelect, selectedId: "1" });

        await fireEvent.keyDown(document.body, { key: "ArrowDown" });

        expect(onSelect).toHaveBeenCalledWith(
            expect.objectContaining({ id: "2" }),
        );
    });

    it("o opens the cursor row and marks it read", async () => {
        const onSelect = vi.fn();
        const { fetchSpy } = await renderList({ onSelect });

        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "o" });

        expect(onSelect).toHaveBeenCalledWith(
            expect.objectContaining({ id: "1" }),
        );
        expect(fetchCalls(fetchSpy)).toContain("/api/inbox/1/read");
    });

    it("o opens the already-open PR on GitHub instead of closing it", async () => {
        const openSpy = vi.spyOn(window, "open").mockReturnValue(null);
        const onSelect = vi.fn();
        await renderList({ onSelect, selectedId: "1" });

        await fireEvent.keyDown(document.body, { key: "o" });

        expect(onSelect).not.toHaveBeenCalled();
        expect(openSpy).toHaveBeenCalledWith(
            "https://github.com/owner/repo/pull/42",
            "_blank",
            "noopener",
        );
    });

    it("j opens the next PR when the detail panel is open", async () => {
        const onSelect = vi.fn();
        await renderList({ onSelect, selectedId: "1" });

        await fireEvent.keyDown(document.body, { key: "j" });

        expect(onSelect).toHaveBeenCalledWith(
            expect.objectContaining({ id: "2" }),
        );
    });

    it("e archives the cursor row and moves the cursor to the next one", async () => {
        const server = statefulInbox([MOCK_NOTIFICATIONS]);
        await renderStateful(server);

        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "e" });

        // Wait for the refetch (which no longer returns the row), not just
        // the optimistic removal.
        await waitFor(() => {
            expect(server.inboxGets()).toBe(2);
        });
        expect(server.posts()).toContain("/api/inbox/1/archive");
        expect(screen.queryByText("Fix bug in parser")).not.toBeInTheDocument();
        expect(rowFor("Refactor auth module")).toHaveClass("cursor");
    });

    it("rapid archives never select a row that was archived meanwhile", async () => {
        const rows = [
            makeItem({ id: "1", title: "First" }),
            makeItem({ id: "2", title: "Second" }),
            makeItem({ id: "3", title: "Third" }),
        ];
        const server = statefulInbox([rows], { holdGets: true });
        const onSelectionChange = vi.fn();
        server.release();
        await renderStateful(server, { selectedId: "1", onSelectionChange });

        await fireEvent.keyDown(document.body, { key: "e" });
        await fireEvent.keyDown(document.body, { key: "e" });
        await waitFor(() => {
            expect(server.posts()).toEqual([
                "/api/inbox/1/archive",
                "/api/inbox/2/archive",
            ]);
        });
        server.release();

        await waitFor(() => {
            expect(onSelectionChange).toHaveBeenCalled();
        });
        const picked = onSelectionChange.mock.calls.map(([n]) => n?.id);
        expect(picked).not.toContain("2");
        expect(picked.at(-1)).toBe("3");
    });

    it("Enter on the focused row acts on the cursor row", async () => {
        const onSelect = vi.fn();
        await renderList({ onSelect });

        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "j" });
        await waitFor(() => {
            expect(document.activeElement).toBe(rowFor("Refactor auth module"));
        });

        await fireEvent.keyDown(document.activeElement!, { key: "Enter" });

        expect(onSelect).toHaveBeenCalledOnce();
        expect(onSelect).toHaveBeenCalledWith(
            expect.objectContaining({ id: "2" }),
        );
    });

    it("focusing a row with Tab moves the cursor to it", async () => {
        await renderList();

        (rowFor("Refactor auth module") as HTMLElement).focus();
        await tick();

        expect(rowFor("Refactor auth module")).toHaveClass("cursor");
    });

    it("j presses during a page load fetch the next page once", async () => {
        const page1 = [
            makeItem({ id: "1", title: "First" }),
            makeItem({ id: "2", title: "Second" }),
        ];
        const page2 = [makeItem({ id: "3", title: "Third" })];
        const server = statefulInbox([page1, page2], {
            holdGets: true,
            total: DEFAULT_PER_PAGE + 1,
        });
        server.release();
        await renderStateful(server);

        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "k" });
        server.release();

        await waitFor(() => {
            expect(rowFor("Third")).toHaveClass("cursor");
        });
        expect(server.pageGets(2)).toBe(1);
    });

    it("e unarchives in the Archived view", async () => {
        const { fetchSpy } = await renderList({ currentView: "archived" });

        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "e" });

        await waitFor(() => {
            expect(fetchCalls(fetchSpy)).toContain("/api/inbox/1/unarchive");
        });
    });

    it("Shift+I marks the cursor row read without opening it", async () => {
        const onSelect = vi.fn();
        const { fetchSpy } = await renderList({ onSelect });

        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "I", shiftKey: true });

        expect(fetchCalls(fetchSpy)).toContain("/api/inbox/1/read");
        expect(onSelect).not.toHaveBeenCalled();
    });

    it("v and Shift+O open the cursor row on GitHub", async () => {
        const openSpy = vi.spyOn(window, "open").mockReturnValue(null);
        await renderList();

        await fireEvent.keyDown(document.body, { key: "j" });
        await fireEvent.keyDown(document.body, { key: "v" });
        await fireEvent.keyDown(document.body, { key: "O", shiftKey: true });

        expect(openSpy).toHaveBeenCalledTimes(2);
        expect(openSpy).toHaveBeenCalledWith(
            "https://github.com/owner/repo/pull/42",
            "_blank",
            "noopener",
        );
    });

    it("e does nothing before the cursor is placed", async () => {
        const { fetchSpy } = await renderList();

        await fireEvent.keyDown(document.body, { key: "e" });

        expect(fetchCalls(fetchSpy)).toEqual([]);
    });
});
