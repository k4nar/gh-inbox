import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import type { PaginatedInbox, PrDetailResponse } from "./lib/types.ts";

vi.mock("./lib/sse.svelte.ts", () => ({
    connectSSE: vi.fn(),
    disconnectSSE: vi.fn(),
    getSyncStatus: vi.fn(() => "idle"),
    onGithubSyncError: vi.fn(() => () => {}),
    onNewNotifications: vi.fn(() => () => {}),
    onPrInfoUpdated: vi.fn(() => () => {}),
    onPrTeamsUpdated: vi.fn(() => () => {}),
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
            pr_status: "open",
            new_commits: 1,
            new_comments: [],
            teams: [],
            new_reviews: [],
        },
    ],
    total: 1,
    page: 1,
    per_page: 20,
};

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
    comments: [],
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
};

function installFetchMock(): void {
    globalThis.fetch = vi.fn((input: string | URL | Request) => {
        const url = String(input);

        if (url.includes("/api/inbox/") && url.endsWith("/read")) {
            return Promise.resolve(
                Response.json({ ok: true }),
            ) as Promise<Response>;
        }

        if (url.includes("/api/inbox?")) {
            return Promise.resolve(
                Response.json(BASE_INBOX),
            ) as Promise<Response>;
        }

        if (url.includes("/threads")) {
            return Promise.resolve(Response.json([])) as Promise<Response>;
        }

        if (url.includes("/api/pull-requests/")) {
            return Promise.resolve(
                Response.json(BASE_DETAIL),
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
});
