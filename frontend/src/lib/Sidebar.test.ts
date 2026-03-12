import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";

describe("Sidebar", () => {
	it("renders the Inbox nav item", () => {
		render(Sidebar);
		expect(screen.getByText("Inbox")).toBeInTheDocument();
	});

	it("renders the Archived nav item", () => {
		render(Sidebar);
		expect(screen.getByText("Archived")).toBeInTheDocument();
	});

	it("renders the Repositories section label", () => {
		render(Sidebar);
		expect(screen.getByText("Repositories")).toBeInTheDocument();
	});

	it("renders the Codeowner Teams section label", () => {
		render(Sidebar);
		expect(screen.getByText("Codeowner Teams")).toBeInTheDocument();
	});

	it("shows active class on Inbox when currentView is inbox", () => {
		render(Sidebar, { props: { currentView: "inbox" } });
		const inboxBtn = screen.getByText("Inbox").closest("button");
		const archivedBtn = screen.getByText("Archived").closest("button");
		expect(inboxBtn.classList.contains("active")).toBe(true);
		expect(archivedBtn.classList.contains("active")).toBe(false);
	});

	it("shows active class on Archived when currentView is archived", () => {
		render(Sidebar, { props: { currentView: "archived" } });
		const inboxBtn = screen.getByText("Inbox").closest("button");
		const archivedBtn = screen.getByText("Archived").closest("button");
		expect(inboxBtn.classList.contains("active")).toBe(false);
		expect(archivedBtn.classList.contains("active")).toBe(true);
	});

	it("calls onViewChange with 'archived' when Archived is clicked", async () => {
		const onViewChange = vi.fn();
		render(Sidebar, { props: { currentView: "inbox", onViewChange } });
		await fireEvent.click(screen.getByText("Archived"));
		expect(onViewChange).toHaveBeenCalledWith("archived");
	});

	it("calls onViewChange with 'inbox' when Inbox is clicked", async () => {
		const onViewChange = vi.fn();
		render(Sidebar, { props: { currentView: "archived", onViewChange } });
		await fireEvent.click(screen.getByText("Inbox"));
		expect(onViewChange).toHaveBeenCalledWith("inbox");
	});
});
