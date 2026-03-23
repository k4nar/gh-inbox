import {
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import PrDetail from "./PrDetail.test-helpers.svelte";
import { onPrInfoUpdated } from "./sse.svelte.ts";
import type { PrDetailResponse } from "./types.ts";

vi.mock("./sse.svelte.ts", async (importOriginal) => {
    const actual = await importOriginal<typeof import("./sse.svelte.ts")>();
    return {
        ...actual,
        onPrInfoUpdated: vi.fn(actual.onPrInfoUpdated),
    };
});

function makeComment(overrides: object = {}) {
    return {
        id: 1,
        pr_id: 42,
        thread_id: "conversation",
        author: "bob",
        body: "Looks good!",
        body_html: "<p>Looks good!</p>",
        created_at: "2025-06-01T09:00:00Z",
        comment_type: "issue_comment",
        path: null,
        position: null,
        in_reply_to_id: null,
        html_url: "https://github.com/owner/repo/pull/42#issuecomment-1",
        diff_hunk: null,
        ...overrides,
    };
}

const BASE_DETAIL: PrDetailResponse = {
    pull_request: {
        id: 42,
        title: "Fix bug in parser",
        repo: "owner/repo",
        author: "alice",
        url: "https://github.com/owner/repo/pull/42",
        ci_status: "success",
        last_viewed_at: "2025-06-01T10:00:00Z",
        body: "This fixes the parser bug.",
        body_html: "<p>This fixes the parser bug.</p>",
        state: "open",
        head_sha: "abc123",
        additions: 10,
        deletions: 3,
        changed_files: 2,
        draft: false,
        merged_at: null,
    },
    threads: [
        {
            thread_id: "conversation",
            path: null,
            resolved: false,
            comments: [makeComment()],
        },
    ],
    commits: [
        {
            sha: "abc123",
            pr_id: 42,
            message: "Old commit",
            author: "alice",
            committed_at: "2025-06-01T08:00:00Z",
        },
        {
            sha: "def456",
            pr_id: 42,
            message: "New commit",
            author: "alice",
            committed_at: "2025-06-01T11:00:00Z",
        },
    ],
    check_runs: [
        { name: "CI / test", status: "completed", conclusion: "success" },
        { name: "CI / lint", status: "in_progress", conclusion: null },
    ],
    previous_viewed_at: "2025-06-01T10:00:00Z",
    reviews: [],
    labels: [],
};

function mockFetch(detail = BASE_DETAIL) {
    return vi.fn((_url: string) =>
        Promise.resolve({
            ok: true,
            json: () => Promise.resolve(detail),
        }),
    ) as unknown as typeof fetch;
}

function renderDetail(detail = BASE_DETAIL) {
    globalThis.fetch = mockFetch(detail);
    return render(PrDetail, {
        props: {
            notification: {
                repository: "owner/repo",
                pr_id: 42,
                title: "Fix bug in parser",
            },
            onClose: vi.fn(),
        },
    });
}

afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
});

describe("PrDetail — status bar", () => {
    it("renders loading state initially", () => {
        renderDetail();
        expect(screen.getByText("Loading...")).toBeInTheDocument();
    });

    it("shows state pill with correct label", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            expect(container.querySelector(".state-pill")).toBeInTheDocument();
        });
        expect(container.querySelector(".state-pill")!.textContent).toBe(
            "Open",
        );
    });

    it("shows Draft pill for draft PRs", async () => {
        const detail = {
            ...BASE_DETAIL,
            pull_request: { ...BASE_DETAIL.pull_request, draft: true },
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(container.querySelector(".state-pill")!.textContent).toBe(
                "Draft",
            );
        });
    });

    it("shows Merged pill for merged PRs", async () => {
        const detail = {
            ...BASE_DETAIL,
            pull_request: {
                ...BASE_DETAIL.pull_request,
                merged_at: "2025-06-02T00:00:00Z",
                state: "closed",
            },
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(container.querySelector(".state-pill")!.textContent).toBe(
                "Merged",
            );
        });
    });

    it("shows author name in status bar", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            expect(container.querySelector(".status-author")!.textContent).toBe(
                "alice",
            );
        });
    });

    it("shows author avatar in status bar", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            const avatar = container.querySelector(
                ".status-avatar",
            ) as HTMLImageElement;
            expect(avatar).toBeInTheDocument();
            expect(avatar.src).toContain("alice");
        });
    });

    it("shows diff stats in status bar", async () => {
        renderDetail();
        await waitFor(() => {
            expect(screen.getByText("+10")).toBeInTheDocument();
            expect(screen.getByText("−3")).toBeInTheDocument();
        });
    });

    it("shows CI passing when all checks pass", async () => {
        const detail = {
            ...BASE_DETAIL,
            check_runs: [
                { name: "CI", status: "completed", conclusion: "success" },
            ],
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            const svg = container.querySelector(".ci-wrapper svg");
            expect(svg?.getAttribute("aria-label")).toMatch(/passing/i);
        });
    });

    it("shows failing count when some checks fail", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            const svg = container.querySelector(".ci-wrapper svg");
            expect(svg?.getAttribute("aria-label")).toMatch(
                /1 (failing|running)/i,
            );
        });
    });

    it("renders GitHub link in header", async () => {
        renderDetail();
        await waitFor(() => {
            const link = screen.getByRole("link", { name: "Open on GitHub" });
            expect(link.getAttribute("href")).toBe(
                "https://github.com/owner/repo/pull/42",
            );
        });
    });
});

describe("PrDetail — timeline", () => {
    it("shows description expanded on first visit", async () => {
        const detail = { ...BASE_DETAIL, previous_viewed_at: null };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(
                container.querySelector(".description-content"),
            ).toBeInTheDocument();
        });
        expect(container.textContent).toContain("This fixes the parser bug.");
    });

    it("shows description collapsed when previously viewed", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            expect(
                container.querySelector(".description-header"),
            ).toBeInTheDocument();
        });
        // Content stays in DOM but inside a closed collapsible (Bits UI uses data-state)
        expect(
            container.querySelector(
                "[data-state='closed'] .description-content",
            ),
        ).toBeInTheDocument();
    });

    it("shows fallback text when PR has no description", async () => {
        const detail = {
            ...BASE_DETAIL,
            previous_viewed_at: null,
            pull_request: {
                ...BASE_DETAIL.pull_request,
                body: "",
                body_html: "",
            },
        };
        renderDetail(detail);
        await waitFor(() => {
            expect(
                screen.getByText("No description provided."),
            ).toBeInTheDocument();
        });
    });

    it("shows Since last visit divider when there are new items", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            expect(container.querySelector(".divider-new")).toBeInTheDocument();
        });
        expect(container.querySelector(".divider-new")!.textContent).toMatch(
            /since your last visit/i,
        );
    });

    it("does not show dividers when previous_viewed_at is null (first visit)", async () => {
        const detail = { ...BASE_DETAIL, previous_viewed_at: null };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(container.querySelector(".zone")).toBeInTheDocument();
        });
        expect(container.querySelector(".divider-new")).not.toBeInTheDocument();
    });

    it("new commit appears in the new zone", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            expect(container.querySelector(".divider-new")).toBeInTheDocument();
        });
        const newZone = container.querySelector(".zone-new")!;
        expect(newZone).toBeInTheDocument();
        expect(newZone.textContent).toContain("New commit");
    });

    it("old commit appears in the earlier zone", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            expect(container.querySelector(".zone-old")).toBeInTheDocument();
        });
        const oldZone = container.querySelector(".zone-old")!;
        expect(oldZone.textContent).toContain("Old commit");
    });

    it("renders comment threads", async () => {
        renderDetail();
        await waitFor(() => {
            expect(screen.getByText("Conversation")).toBeInTheDocument();
        });
    });
});

describe("PrDetail — labels", () => {
    it("renders label chips in the status bar with correct color and name", async () => {
        const detail = {
            ...BASE_DETAIL,
            labels: [
                { name: "bug", color: "d73a4a" },
                { name: "enhancement", color: "a2eeef" },
            ],
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(
                container.querySelector(".labels-wrapper"),
            ).toBeInTheDocument();
        });
        fireEvent.pointerEnter(container.querySelector(".labels-wrapper")!);
        await waitFor(() => {
            const chips = document.querySelectorAll(".label-chip");
            expect(chips).toHaveLength(2);
        });
        const chips = document.querySelectorAll(".label-chip");
        expect(chips[0].textContent?.trim()).toBe("bug");
        expect(chips[1].textContent?.trim()).toBe("enhancement");
    });

    it("renders no label chips when labels array is empty", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            expect(container.querySelector(".state-pill")).toBeInTheDocument();
        });
        expect(document.querySelectorAll(".label-chip")).toHaveLength(0);
    });
});

describe("PrDetail — reviews", () => {
    it("renders a review with no body compactly (no body paragraph)", async () => {
        const detail = {
            ...BASE_DETAIL,
            reviews: [
                {
                    id: 1,
                    reviewer: "charlie",
                    state: "APPROVED",
                    body: "",
                    submitted_at: "2025-06-01T08:00:00Z",
                    html_url:
                        "https://github.com/owner/repo/pull/42#pullrequestreview-1",
                },
            ],
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(container.querySelector(".review-item")).toBeInTheDocument();
        });
        const reviewItem = container.querySelector(".review-item")!;
        expect(reviewItem.textContent).toContain("charlie");
        expect(reviewItem.textContent).toContain("Approved");
        expect(
            reviewItem.querySelector(".review-comment"),
        ).not.toBeInTheDocument();
        expect(
            reviewItem.querySelector(".thread-chevron"),
        ).not.toBeInTheDocument();
    });

    it("renders a review with body showing a toggle but not the body by default", async () => {
        const detail = {
            ...BASE_DETAIL,
            reviews: [
                {
                    id: 2,
                    reviewer: "dave",
                    state: "CHANGES_REQUESTED",
                    body: "Please fix the typo on line 42.",
                    submitted_at: "2025-06-01T08:00:00Z",
                    html_url:
                        "https://github.com/owner/repo/pull/42#pullrequestreview-2",
                },
            ],
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(container.querySelector(".review-item")).toBeInTheDocument();
        });
        const reviewItem = container.querySelector(".review-item")!;
        expect(reviewItem.textContent).toContain("dave");
        expect(reviewItem.textContent).toContain("Changes requested");
        expect(reviewItem.querySelector(".thread-chevron")).toBeInTheDocument();
        // Body is collapsed by default
        expect(
            reviewItem.querySelector(".review-comment"),
        ).not.toBeInTheDocument();
    });

    it("shows New badge for a review submitted after previous_viewed_at", async () => {
        const detail = {
            ...BASE_DETAIL,
            previous_viewed_at: "2025-06-01T10:00:00Z",
            reviews: [
                {
                    id: 3,
                    reviewer: "eve",
                    state: "APPROVED",
                    body: "",
                    // after previous_viewed_at
                    submitted_at: "2025-06-01T11:00:00Z",
                    html_url:
                        "https://github.com/owner/repo/pull/42#pullrequestreview-3",
                },
            ],
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(container.querySelector(".review-item")).toBeInTheDocument();
        });
        const reviewItem = container.querySelector(".review-item")!;
        expect(
            reviewItem.querySelector(".new-count-badge"),
        ).toBeInTheDocument();
    });

    it("renders a dismissed review with Dismissed pill", async () => {
        const detail = {
            ...BASE_DETAIL,
            reviews: [
                {
                    id: 5,
                    reviewer: "grace",
                    state: "DISMISSED",
                    body: "",
                    submitted_at: "2025-06-01T08:00:00Z",
                    html_url:
                        "https://github.com/owner/repo/pull/42#pullrequestreview-5",
                },
            ],
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(container.querySelector(".review-item")).toBeInTheDocument();
        });
        const pill = container.querySelector(".pill-dismissed");
        expect(pill).toBeInTheDocument();
        expect(pill!.textContent).toBe("Dismissed");
    });

    it("does not show New badge for a review submitted before previous_viewed_at", async () => {
        const detail = {
            ...BASE_DETAIL,
            previous_viewed_at: "2025-06-01T10:00:00Z",
            reviews: [
                {
                    id: 4,
                    reviewer: "frank",
                    state: "APPROVED",
                    body: "",
                    // before previous_viewed_at
                    submitted_at: "2025-06-01T09:00:00Z",
                    html_url:
                        "https://github.com/owner/repo/pull/42#pullrequestreview-4",
                },
            ],
        };
        const { container } = renderDetail(detail);
        await waitFor(() => {
            expect(container.querySelector(".review-item")).toBeInTheDocument();
        });
        const reviewItem = container.querySelector(".review-item")!;
        expect(
            reviewItem.querySelector(".new-count-badge"),
        ).not.toBeInTheDocument();
    });
});

describe("PrDetail — SSE reload", () => {
    it("reloads when pr:info_updated fires for the current PR", async () => {
        let capturedCallback: ((data: object) => void) | null = null;
        vi.mocked(onPrInfoUpdated).mockImplementation((cb) => {
            capturedCallback = cb as (data: object) => void;
            return () => {};
        });

        const fetchMock = mockFetch();
        globalThis.fetch = fetchMock;
        render(PrDetail, {
            props: {
                notification: {
                    repository: "owner/repo",
                    pr_id: 42,
                    title: "Fix bug in parser",
                },
                onClose: vi.fn(),
            },
        });

        await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));

        capturedCallback!({
            pr_id: 42,
            repository: "owner/repo",
            author: "alice",
            pr_status: "open",
            new_commits: 1,
            new_comments: null,
            new_reviews: null,
        });

        await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
    });

    it("does not reload when pr:info_updated carries no new data (view-acknowledgement event)", async () => {
        let capturedCallback: ((data: object) => void) | null = null;
        vi.mocked(onPrInfoUpdated).mockImplementation((cb) => {
            capturedCallback = cb as (data: object) => void;
            return () => {};
        });

        const fetchMock = mockFetch();
        globalThis.fetch = fetchMock;
        render(PrDetail, {
            props: {
                notification: {
                    repository: "owner/repo",
                    pr_id: 42,
                    title: "Fix bug in parser",
                },
                onClose: vi.fn(),
            },
        });

        await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));

        // Simulate the all-zeros event that get_pr emits after a view
        capturedCallback!({
            pr_id: 42,
            repository: "owner/repo",
            author: "alice",
            pr_status: "open",
            new_commits: 0,
            new_comments: [],
            new_reviews: [],
        });

        await new Promise((r) => setTimeout(r, 50));
        expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    it("does not reload when pr:info_updated fires for a different PR", async () => {
        let capturedCallback: ((data: object) => void) | null = null;
        vi.mocked(onPrInfoUpdated).mockImplementation((cb) => {
            capturedCallback = cb as (data: object) => void;
            return () => {};
        });

        const fetchMock = mockFetch();
        globalThis.fetch = fetchMock;
        render(PrDetail, {
            props: {
                notification: {
                    repository: "owner/repo",
                    pr_id: 42,
                    title: "Fix bug in parser",
                },
                onClose: vi.fn(),
            },
        });

        await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));

        capturedCallback!({
            pr_id: 99,
            repository: "owner/repo",
            author: "bob",
            pr_status: "open",
            new_commits: 1,
            new_comments: null,
            new_reviews: null,
        });

        await new Promise((r) => setTimeout(r, 50));
        expect(fetchMock).toHaveBeenCalledTimes(1);
    });
});
