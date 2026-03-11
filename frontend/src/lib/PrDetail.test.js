import { cleanup, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
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
	commits: [
		{
			sha: "abc123",
			pr_id: 42,
			message: "First commit",
			author: "alice",
			committed_at: "2025-06-01T08:00:00Z",
		},
		{
			sha: "def456",
			pr_id: 42,
			message: "Second commit",
			author: "alice",
			committed_at: "2025-06-01T11:00:00Z",
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

function renderPrDetail() {
	globalThis.fetch = mockDetailFetch();
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

describe("PrDetail", () => {
	afterEach(() => {
		cleanup();
		vi.restoreAllMocks();
	});

	it("renders loading state initially", () => {
		renderPrDetail();
		expect(screen.getByText("Loading...")).toBeInTheDocument();
	});

	it("renders PR metadata", async () => {
		const { container } = renderPrDetail();

		await waitFor(() => {
			// Check author in the PR meta section specifically
			const authorValue = container.querySelector(".pr-meta .meta-value");
			expect(authorValue).toBeInTheDocument();
			expect(authorValue.textContent).toBe("alice");
		});

		expect(screen.getByText("open")).toBeInTheDocument();
		expect(screen.getByText("+10")).toBeInTheDocument();
		expect(screen.getByText("-3")).toBeInTheDocument();
	});

	it("renders CI status badges with failures/pending first", async () => {
		const { container } = renderPrDetail();

		// Lint is in_progress (pending) — should be visible immediately
		await waitFor(() => {
			expect(screen.getByText("Lint")).toBeInTheDocument();
		});

		expect(screen.getByText("Running")).toBeInTheDocument();

		// CI is passing — should be behind toggle, not visible yet
		expect(screen.queryByText("CI")).not.toBeInTheDocument();

		// Click toggle to expand passing checks
		const toggle = container.querySelector(".ci-passing-toggle");
		expect(toggle).toBeInTheDocument();
		toggle.click();

		await waitFor(() => {
			expect(screen.getByText("CI")).toBeInTheDocument();
		});
		expect(screen.getByText("success")).toBeInTheDocument();
	});

	it("renders comment threads", async () => {
		renderPrDetail();

		await waitFor(() => {
			expect(screen.getByText("Looks good!")).toBeInTheDocument();
		});

		expect(screen.getByText("Nit: rename this")).toBeInTheDocument();
		expect(screen.getByText("src/main.rs")).toBeInTheDocument();
		expect(screen.getByText("bob")).toBeInTheDocument();
		expect(screen.getByText("carol")).toBeInTheDocument();
	});

	it("highlights new comments with badge", async () => {
		const { container } = renderPrDetail();

		await waitFor(() => {
			expect(screen.getByText("Nit: rename this")).toBeInTheDocument();
		});

		// Carol's comment (11:00) is after last_viewed_at (10:00) — should have "new" badge in threads
		const threadNewBadges = container.querySelectorAll(
			".threads-section .new-badge",
		);
		expect(threadNewBadges).toHaveLength(1);
	});

	it("renders description", async () => {
		renderPrDetail();

		await waitFor(() => {
			expect(
				screen.getByText("This fixes the parser bug."),
			).toBeInTheDocument();
		});
	});

	it("renders a link to the GitHub PR", async () => {
		renderPrDetail();

		await waitFor(() => {
			const link = screen.getByTitle("Open on GitHub");
			expect(link).toBeInTheDocument();
			expect(link.getAttribute("href")).toBe(
				"https://github.com/owner/repo/pull/42",
			);
			expect(link.getAttribute("target")).toBe("_blank");
		});
	});

	it("renders commits with new-commit highlighting", async () => {
		const { container } = renderPrDetail();

		await waitFor(() => {
			expect(screen.getByText("First commit")).toBeInTheDocument();
		});

		expect(screen.getByText("Second commit")).toBeInTheDocument();

		// Second commit (11:00) is after last_viewed_at (10:00) — should have "new" badge
		// First commit (08:00) is before — should not
		const commitNewBadges = container.querySelectorAll(
			".commits-section .new-badge",
		);
		expect(commitNewBadges).toHaveLength(1);
	});

	it("groups CI checks by status with failures first", async () => {
		const { container } = renderPrDetail();

		await waitFor(() => {
			// Lint is in_progress (pending), should be shown prominently
			expect(screen.getByText("Lint")).toBeInTheDocument();
		});

		// CI is passing — should be behind a collapsible toggle
		const passingToggle = container.querySelector(".ci-passing-toggle");
		expect(passingToggle).toBeInTheDocument();
		expect(passingToggle.textContent).toContain("1 passing");
	});
});
