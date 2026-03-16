import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import CommentThread from "./CommentThread.svelte";

function makeComment(overrides: Record<string, unknown> = {}) {
    const body = (overrides.body as string) ?? "Looks good!";
    return {
        id: 1,
        pr_id: 0,
        thread_id: "conversation",
        author: "bob",
        body,
        body_html: `<p>${body}</p>`,
        created_at: "2025-06-01T09:00:00Z",
        comment_type: "issue_comment",
        path: null,
        position: null,
        in_reply_to_id: null,
        html_url: "https://github.com/owner/repo/pull/1#issuecomment-1",
        ...overrides,
    };
}

afterEach(cleanup);

describe("CommentThread — collapsed (default)", () => {
    it("shows thread header label for conversation thread", () => {
        const thread = {
            thread_id: "conversation",
            path: null,
            comments: [makeComment()],
        };
        render(CommentThread, { props: { thread } });
        expect(screen.getByText("Conversation")).toBeInTheDocument();
    });

    it("shows file path for review thread", () => {
        const thread = {
            thread_id: "review:100",
            path: "src/main.rs",
            comments: [
                makeComment({
                    thread_id: "review:100",
                    comment_type: "review_comment",
                    path: "src/main.rs",
                }),
            ],
        };
        render(CommentThread, { props: { thread } });
        expect(screen.getByText("src/main.rs")).toBeInTheDocument();
    });

    it("shows preview of first and last comment", () => {
        const thread = {
            thread_id: "conversation",
            path: null,
            comments: [
                makeComment({
                    id: 1,
                    author: "alice",
                    body: "First comment here",
                }),
                makeComment({
                    id: 2,
                    author: "bob",
                    body: "Last comment here",
                    created_at: "2025-06-01T10:00:00Z",
                }),
            ],
        };
        const { container } = render(CommentThread, { props: { thread } });
        const previews = container.querySelectorAll(".comment-preview");
        expect(previews).toHaveLength(2);
        // First preview shows truncated first comment
        expect(previews[0].textContent).toContain("alice");
        // Last preview shows truncated last comment
        expect(previews[1].textContent).toContain("bob");
    });

    it("shows 'new' badge on thread header when there are new comments", () => {
        const thread = {
            thread_id: "conversation",
            path: null,
            comments: [
                makeComment({ id: 1, created_at: "2025-06-01T09:00:00Z" }),
                makeComment({
                    id: 2,
                    created_at: "2025-06-01T11:00:00Z",
                    author: "carol",
                }),
            ],
        };
        const { container } = render(CommentThread, {
            props: { thread, previousViewedAt: "2025-06-01T10:00:00Z" },
        });
        expect(container.querySelector(".new-count-badge")).toBeInTheDocument();
        expect(
            container.querySelector(".new-count-badge")!.textContent,
        ).toContain("1");
    });
});

describe("CommentThread — expanded", () => {
    it("renders all comments as links when initiallyExpanded is true", async () => {
        const thread = {
            thread_id: "conversation",
            path: null,
            comments: [
                makeComment({ id: 1, author: "alice", body: "First comment" }),
                makeComment({
                    id: 2,
                    author: "bob",
                    body: "Second comment",
                    created_at: "2025-06-01T10:00:00Z",
                }),
            ],
        };
        render(CommentThread, { props: { thread, initiallyExpanded: true } });
        expect(screen.getByText("First comment")).toBeInTheDocument();
        expect(screen.getByText("Second comment")).toBeInTheDocument();
        // Comments are anchor tags
        const links = screen.getAllByRole("link");
        expect(links.length).toBeGreaterThanOrEqual(2);
        links.forEach((link) => {
            expect(link.getAttribute("target")).toBe("_blank");
        });
    });

    it("clicking a collapsed thread expands it", async () => {
        const thread = {
            thread_id: "conversation",
            path: null,
            comments: [makeComment({ body: "Expandable comment" })],
        };
        const { container } = render(CommentThread, { props: { thread } });

        // Not expanded initially: .thread-comments container should be absent
        expect(
            container.querySelector(".thread-comments"),
        ).not.toBeInTheDocument();

        // Click to expand
        const header = container.querySelector(".thread-header")!;
        await fireEvent.click(header);

        expect(container.querySelector(".thread-comments")).toBeInTheDocument();
        expect(screen.getByText("Expandable comment")).toBeInTheDocument();
    });

    it("shows avatar for each comment author", async () => {
        const thread = {
            thread_id: "conversation",
            path: null,
            comments: [makeComment({ author: "octocat" })],
        };
        const { container } = render(CommentThread, {
            props: { thread, initiallyExpanded: true },
        });
        const avatar = container.querySelector(
            ".comment-avatar",
        ) as HTMLImageElement;
        expect(avatar).toBeInTheDocument();
        expect(avatar.src).toContain("octocat");
    });

    it("new comments have blue left border class", async () => {
        const thread = {
            thread_id: "conversation",
            path: null,
            comments: [
                makeComment({ id: 1, created_at: "2025-06-01T09:00:00Z" }),
                makeComment({ id: 2, created_at: "2025-06-01T11:00:00Z" }),
            ],
        };
        const { container } = render(CommentThread, {
            props: {
                thread,
                previousViewedAt: "2025-06-01T10:00:00Z",
                initiallyExpanded: true,
            },
        });
        const newComments = container.querySelectorAll(".new-comment");
        expect(newComments).toHaveLength(1);
    });
});
