import {
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import type { PaginatedInbox, PrDetailResponse } from "./lib/types.ts";

vi.mock("./lib/sse.svelte.ts", () => ({
    connectSSE: vi.fn(),
    disconnectSSE: vi.fn(),
    getSyncStatus: vi.fn(() => "idle"),
    getSyncErrorMessage: vi.fn(() => null),
    onGithubSyncError: vi.fn(() => () => {}),
    onNewNotifications: vi.fn(() => () => {}),
    onPrInfoUpdated: vi.fn(() => () => {}),
}));

const BASE_INBOX: PaginatedInbox = {
    items: [
        {
            id: "notif-1",
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
            new_commits: 1,
            new_comments: [],
            teams: [],
            new_reviews: [],
        },
        {
            id: "notif-2",
            pr_id: 10,
            title: "Refactor auth module",
            repository: "org/api",
            reason: "mention",
            unread: false,
            archived: false,
            updated_at: "2025-06-01T11:00:00Z",
            author: "bob",
            author_avatar_url: null,
            pr_status: "draft",
            ci_status: null,
            new_commits: 0,
            new_comments: [],
            teams: [],
            new_reviews: [],
        },
    ],
    total: 2,
    page: 1,
    per_page: 20,
};

const DETAIL_BY_PR: Record<number, PrDetailResponse> = {
    42: {
        pull_request: {
            id: 42,
            title: "Fix bug in parser",
            repo: "owner/repo",
            author: "alice",
            author_avatar_url: null,
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
        threads: [],
        commits: [
            {
                sha: "abc123",
                pr_id: 42,
                message: "Initial commit",
                author: "alice",
                committed_at: "2025-06-01T08:00:00Z",
            },
        ],
        check_runs: [],
        previous_viewed_at: null,
        reviews: [],
        labels: [],
    },
    10: {
        pull_request: {
            id: 10,
            title: "Refactor auth module",
            repo: "org/api",
            author: "bob",
            author_avatar_url: null,
            url: "https://github.com/org/api/pull/10",
            ci_status: "pending",
            last_viewed_at: "2025-06-01T09:00:00Z",
            body: "This refactors the auth module.",
            body_html: "<p>This refactors the auth module.</p>",
            state: "open",
            head_sha: "def456",
            additions: 25,
            deletions: 9,
            changed_files: 4,
            draft: true,
            merged_at: null,
        },
        threads: [],
        commits: [
            {
                sha: "def456",
                pr_id: 10,
                message: "Follow-up cleanup",
                author: "bob",
                committed_at: "2025-06-01T07:00:00Z",
            },
        ],
        check_runs: [],
        previous_viewed_at: null,
        reviews: [],
        labels: [],
    },
};

function installFetchMock(): void {
    let archivedIds = new Set<string>();

    globalThis.fetch = vi.fn((input: string | URL | Request) => {
        const url = String(input);

        if (url.includes("/api/inbox/") && url.endsWith("/read")) {
            return Promise.resolve(
                Response.json({ ok: true }),
            ) as Promise<Response>;
        }

        if (url.includes("/api/inbox/") && url.endsWith("/archive")) {
            const archivedId = url.split("/").at(-2);
            if (archivedId) {
                archivedIds = new Set([...archivedIds, archivedId]);
            }
            return Promise.resolve(
                new Response(null, { status: 204 }),
            ) as Promise<Response>;
        }

        if (url.includes("/api/inbox?")) {
            const items = BASE_INBOX.items.filter(
                (item) => !archivedIds.has(item.id),
            );
            return Promise.resolve(
                Response.json({
                    ...BASE_INBOX,
                    items,
                    total: items.length,
                }),
            ) as Promise<Response>;
        }

        if (url.includes("/api/pull-requests/")) {
            const number = Number(url.split("/").at(-1));
            return Promise.resolve(
                Response.json(DETAIL_BY_PR[number]),
            ) as Promise<Response>;
        }

        if (url.endsWith("/api/preferences")) {
            return Promise.resolve(
                Response.json({ theme: "system" }),
            ) as Promise<Response>;
        }

        return Promise.reject(new Error(`Unhandled fetch URL: ${url}`));
    }) as typeof fetch;
}

function installLocalStorageMock(): void {
    const storage = new Map<string, string>();

    Object.defineProperty(window, "localStorage", {
        configurable: true,
        value: {
            getItem: (key: string) => storage.get(key) ?? null,
            setItem: (key: string, value: string) => {
                storage.set(key, value);
            },
            removeItem: (key: string) => {
                storage.delete(key);
            },
            clear: () => {
                storage.clear();
            },
        },
    });
}

beforeEach(() => {
    cleanup();
    vi.restoreAllMocks();
    installLocalStorageMock();
    window.localStorage.clear();
    installFetchMock();
});

afterEach(() => {
    cleanup();
});

describe("App", () => {
    it("opens the PR detail panel when selecting a notification", async () => {
        render(App);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        expect(
            screen.queryByLabelText("Resize PR detail panel"),
        ).not.toBeInTheDocument();

        screen.getByText("Fix bug in parser").click();

        await waitFor(() => {
            expect(
                screen.getByLabelText("Resize PR detail panel"),
            ).toBeInTheDocument();
        });
    });

    it("closes the PR detail panel when clicking the active notification again", async () => {
        render(App);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        const listRow = screen
            .getByText("Fix bug in parser")
            .closest(".pr-item") as HTMLDivElement;

        await fireEvent.click(listRow);

        await waitFor(() => {
            expect(
                screen.getByLabelText("Resize PR detail panel"),
            ).toBeInTheDocument();
        });

        await fireEvent.click(listRow);

        await waitFor(() => {
            expect(
                screen.queryByLabelText("Resize PR detail panel"),
            ).not.toBeInTheDocument();
        });
    });

    it("shows the next item after archiving the selected notification", async () => {
        const { container } = render(App);

        await waitFor(() => {
            expect(screen.getByText("Fix bug in parser")).toBeInTheDocument();
        });

        screen.getByText("Fix bug in parser").click();

        await waitFor(() => {
            expect(screen.getByText("Initial commit")).toBeInTheDocument();
        });

        const archiveButtons = container.querySelectorAll(
            'button[aria-label="Archive"]',
        );
        await fireEvent.click(archiveButtons[0] as HTMLButtonElement);

        await waitFor(() => {
            expect(screen.getAllByText("Refactor auth module")).toHaveLength(2);
        });
    });
});
