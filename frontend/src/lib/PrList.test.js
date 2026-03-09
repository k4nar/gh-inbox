import { render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PrList from "./PrList.svelte";

const MOCK_NOTIFICATIONS = [
	{
		id: "1",
		pr_id: 42,
		title: "Fix bug in parser",
		repository: "owner/repo",
		reason: "review_requested",
		unread: true,
		archived: false,
		updated_at: "2025-06-01T12:00:00Z",
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
	},
];

function mockFetch(data) {
	return vi.fn(() =>
		Promise.resolve({
			ok: true,
			json: () => Promise.resolve(data),
		}),
	);
}

describe("PrList", () => {
	beforeEach(() => {
		vi.useFakeTimers({ now: new Date("2025-06-01T12:05:00Z") });
	});

	afterEach(() => {
		vi.useRealTimers();
		vi.restoreAllMocks();
	});

	it("renders empty state when inbox is empty", async () => {
		globalThis.fetch = mockFetch([]);

		render(PrList);

		await waitFor(() => {
			expect(screen.getByText("No notifications yet.")).toBeInTheDocument();
		});
	});

	it("renders PR rows with repo, title, and PR number", async () => {
		globalThis.fetch = mockFetch(MOCK_NOTIFICATIONS);

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

	it("renders reason pills with correct labels", async () => {
		globalThis.fetch = mockFetch(MOCK_NOTIFICATIONS);

		render(PrList);

		await waitFor(() => {
			expect(screen.getByText("Review requested")).toBeInTheDocument();
		});

		expect(screen.getByText("Mentioned")).toBeInTheDocument();
	});

	it("shows unread dot for unread notifications", async () => {
		globalThis.fetch = mockFetch(MOCK_NOTIFICATIONS);

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
		globalThis.fetch = mockFetch(MOCK_NOTIFICATIONS);

		render(PrList);

		await waitFor(() => {
			expect(screen.getByText("owner/repo")).toBeInTheDocument();
		});

		// Header shows "2 · 1 unread", statusbar shows "2 PRs · 1 unread"
		expect(screen.getByText("2 · 1 unread")).toBeInTheDocument();
		expect(screen.getByText("2 PRs · 1 unread")).toBeInTheDocument();
	});
});
