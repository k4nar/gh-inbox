import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import PrDetail from "./PrDetail.svelte";
import type { PrDetailResponse } from "./types.ts";

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
    comments: [makeComment()],
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
};

const BASE_THREADS = [
    {
        thread_id: "conversation",
        path: null,
        comments: [makeComment()],
    },
];

function mockFetch(detail = BASE_DETAIL, threads = BASE_THREADS) {
    return vi.fn((url: string) => {
        if (url.includes("/threads")) {
            return Promise.resolve({
                ok: true,
                json: () => Promise.resolve(threads),
            });
        }
        return Promise.resolve({
            ok: true,
            json: () => Promise.resolve(detail),
        });
    }) as unknown as typeof fetch;
}

function renderDetail(detail = BASE_DETAIL, threads = BASE_THREADS) {
    globalThis.fetch = mockFetch(detail, threads);
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
            expect(
                container.querySelector(".ci-indicator")!.textContent,
            ).toMatch(/passing/i);
        });
    });

    it("shows failing count when some checks fail", async () => {
        const { container } = renderDetail();
        await waitFor(() => {
            expect(
                container.querySelector(".ci-indicator")!.textContent,
            ).toMatch(/1 (failing|running)/i);
        });
    });

    it("renders GitHub link in header", async () => {
        renderDetail();
        await waitFor(() => {
            const link = screen.getByTitle("Open on GitHub");
            expect(link.getAttribute("href")).toBe(
                "https://github.com/owner/repo/pull/42",
            );
        });
    });
});

describe("PrDetail — timeline", () => {
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
            expect(
                container.querySelector(".commits-list"),
            ).toBeInTheDocument();
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
