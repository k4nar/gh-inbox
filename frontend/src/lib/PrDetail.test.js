import { render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PrDetail from "./PrDetail.svelte";

const MOCK_DETAIL = {
	pull_request: {
		id: 42,
		title: "Fix bug in parser",
		repo: "owner/repo",
		author: "alice",
		url: "https://github.com/owner/repo/pull/42",
		ci_status: "success",
		last_viewed_at: "2025-06-01T10:00:00Z",
		body: "This fixes the parser bug.",
		state: "open",
		head_sha: "abc123",
		additions: 10,
		deletions: 3,
		changed_files: 2,
	},
	comments: [
		{
			id: 100,
			pr_id: 42,
			thread_id: "conversation",
			author: "bob",
			body: "Looks good!",
			created_at: "2025-06-01T09:00:00Z",
			comment_type: "issue_comment",
			path: null,
			position: null,
			in_reply_to_id: null,
		},
		{
			id: 200,
			pr_id: 42,
			thread_id: "review:200",
			author: "carol",
			body: "Nit: rename this",
			created_at: "2025-06-01T11:00:00Z",
			comment_type: "review_comment",
			path: "src/main.rs",
			position: 10,
			in_reply_to_id: null,
		},
	],
	check_runs: [
		{ name: "CI", status: "completed", conclusion: "success" },
		{ name: "Lint", status: "in_progress", conclusion: null },
	],
};

const MOCK_THREADS = [
	{
		thread_id: "conversation",
		path: null,
		comments: [MOCK_DETAIL.comments[0]],
	},
	{
		thread_id: "review:200",
		path: "src/main.rs",
		comments: [MOCK_DETAIL.comments[1]],
	},
];

function mockDetailFetch() {
	return vi.fn((url) => {
		if (url.includes("/threads")) {
			return Promise.resolve({
				ok: true,
				json: () => Promise.resolve(MOCK_THREADS),
			});
		}
		return Promise.resolve({
			ok: true,
			json: () => Promise.resolve(MOCK_DETAIL),
		});
	});
}

describe("PrDetail", () => {
	beforeEach(() => {
		vi.useFakeTimers({ now: new Date("2025-06-01T12:05:00Z") });
	});

	afterEach(() => {
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it("renders PR metadata", async () => {
		globalThis.fetch = mockDetailFetch();
		const onClose = vi.fn();

		render(PrDetail, {
			props: {
				notification: {
					repository: "owner/repo",
					pr_id: 42,
					title: "Fix bug in parser",
				},
				onClose,
			},
		});

		await waitFor(() => {
			expect(screen.getByText("alice")).toBeInTheDocument();
		});

		expect(screen.getByText("open")).toBeInTheDocument();
		expect(screen.getByText("+10")).toBeInTheDocument();
		expect(screen.getByText("-3")).toBeInTheDocument();
	});

	it("renders CI status badges", async () => {
		globalThis.fetch = mockDetailFetch();

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

		await waitFor(() => {
			expect(screen.getByText("CI")).toBeInTheDocument();
		});

		expect(screen.getByText("Lint")).toBeInTheDocument();
		expect(screen.getByText("success")).toBeInTheDocument();
		expect(screen.getByText("Running")).toBeInTheDocument();
	});

	it("renders comment threads", async () => {
		globalThis.fetch = mockDetailFetch();

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

		await waitFor(() => {
			expect(screen.getByText("Looks good!")).toBeInTheDocument();
		});

		expect(screen.getByText("Nit: rename this")).toBeInTheDocument();
		expect(screen.getByText("src/main.rs")).toBeInTheDocument();
		expect(screen.getByText("bob")).toBeInTheDocument();
		expect(screen.getByText("carol")).toBeInTheDocument();
	});

	it("highlights new comments with badge", async () => {
		globalThis.fetch = mockDetailFetch();

		const { container } = render(PrDetail, {
			props: {
				notification: {
					repository: "owner/repo",
					pr_id: 42,
					title: "Fix bug in parser",
				},
				onClose: vi.fn(),
			},
		});

		await waitFor(() => {
			expect(screen.getByText("Nit: rename this")).toBeInTheDocument();
		});

		// Carol's comment (11:00) is after last_viewed_at (10:00) — should have "new" badge
		const newBadges = container.querySelectorAll(".new-badge");
		expect(newBadges.length).toBeGreaterThanOrEqual(1);

		// Bob's comment (09:00) is before last_viewed_at — should NOT have "new" badge
		// The "new" text should appear exactly once (for Carol's comment)
		expect(screen.getAllByText("new")).toHaveLength(1);
	});

	it("renders description", async () => {
		globalThis.fetch = mockDetailFetch();

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

		await waitFor(() => {
			expect(
				screen.getByText("This fixes the parser bug."),
			).toBeInTheDocument();
		});
	});
});
