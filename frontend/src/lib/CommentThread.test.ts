import { render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import CommentThread from "./CommentThread.svelte";

describe("CommentThread", () => {
	beforeEach(() => {
		vi.useFakeTimers({ now: new Date("2025-06-01T12:05:00Z") });
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it("renders conversation thread with author and body", () => {
		const thread = {
			thread_id: "conversation",
			path: null,
			comments: [
				{
					id: 1,
					pr_id: 0,
					thread_id: "conversation",
					author: "bob",
					body: "Looks good!",
					created_at: "2025-06-01T11:00:00Z",
					comment_type: "issue_comment",
					path: null,
					position: null,
					in_reply_to_id: null,
				},
			],
		};

		render(CommentThread, { props: { thread } });

		expect(screen.getByText("Conversation")).toBeInTheDocument();
		expect(screen.getByText("bob")).toBeInTheDocument();
		expect(screen.getByText("Looks good!")).toBeInTheDocument();
	});

	it("renders file path for review thread", () => {
		const thread = {
			thread_id: "review:100",
			path: "src/main.rs",
			comments: [
				{
					id: 100,
					pr_id: 0,
					thread_id: "review:100",
					author: "carol",
					body: "Nit: rename this",
					created_at: "2025-06-01T11:00:00Z",
					comment_type: "review_comment",
					path: "src/main.rs",
					position: 1,
					in_reply_to_id: null,
				},
			],
		};

		render(CommentThread, { props: { thread } });

		expect(screen.getByText("src/main.rs")).toBeInTheDocument();
		expect(screen.getByText("carol")).toBeInTheDocument();
	});

	it("shows new badge for comments after lastViewedAt", () => {
		const thread = {
			thread_id: "conversation",
			path: null,
			comments: [
				{
					id: 1,
					pr_id: 0,
					thread_id: "conversation",
					author: "bob",
					body: "Old comment",
					created_at: "2025-06-01T09:00:00Z",
					comment_type: "issue_comment",
					path: null,
					position: null,
					in_reply_to_id: null,
				},
				{
					id: 2,
					pr_id: 0,
					thread_id: "conversation",
					author: "carol",
					body: "New comment",
					created_at: "2025-06-01T11:00:00Z",
					comment_type: "issue_comment",
					path: null,
					position: null,
					in_reply_to_id: null,
				},
			],
		};

		const { container } = render(CommentThread, {
			props: { thread, lastViewedAt: "2025-06-01T10:00:00Z" },
		});

		const newBadges = container.querySelectorAll(".new-badge");
		expect(newBadges).toHaveLength(1);
		expect(screen.getByText("new")).toBeInTheDocument();
	});

	it("shows no new badges when lastViewedAt is null", () => {
		const thread = {
			thread_id: "conversation",
			path: null,
			comments: [
				{
					id: 1,
					pr_id: 0,
					thread_id: "conversation",
					author: "bob",
					body: "A comment",
					created_at: "2025-06-01T11:00:00Z",
					comment_type: "issue_comment",
					path: null,
					position: null,
					in_reply_to_id: null,
				},
			],
		};

		const { container } = render(CommentThread, {
			props: { thread, lastViewedAt: null },
		});

		const newBadges = container.querySelectorAll(".new-badge");
		expect(newBadges).toHaveLength(0);
	});
});
