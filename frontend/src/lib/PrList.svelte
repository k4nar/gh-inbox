<script>
import { reasonClass, reasonLabel } from "./reason.js";
import { timeAgo } from "./timeago.js";
import { showError } from "./toast.svelte.js";

/** @type {{ currentView?: string, onSelect?: (notification: any) => void, selectedId?: string|null, refreshKey?: number }} */
let {
	currentView = "inbox",
	onSelect = () => {},
	selectedId = null,
	refreshKey = 0,
} = $props();

let notifications = $state([]);

async function fetchNotifications(view) {
	const res = await fetch(`/api/inbox?status=${view}`);
	if (res.ok) {
		notifications = await res.json();
	}
}

$effect(() => {
	// Re-fetch when view changes or when SSE signals new notifications
	void refreshKey;
	fetchNotifications(currentView);
});

let count = $derived(notifications.length);
let unreadCount = $derived(notifications.filter((n) => n.unread).length);
let viewTitle = $derived(currentView === "archived" ? "Archived" : "Inbox");
let emptyMessage = $derived(
	currentView === "archived" ? "No archived notifications." : "All caught up!",
);

async function handleSelect(notif) {
	// Optimistically mark as read
	if (notif.unread) {
		notif.unread = false;
		notifications = [...notifications];
		try {
			const res = await fetch(`/api/inbox/${notif.id}/read`, {
				method: "POST",
			});
			if (!res.ok) {
				console.error(`Failed to mark read: ${res.status} ${res.statusText}`);
				showError("Failed to mark notification as read");
			}
		} catch (err) {
			console.error("Failed to mark read:", err);
			showError("Failed to mark notification as read");
		}
	}
	onSelect(notif);
}

async function handleArchive(e, notif) {
	e.stopPropagation();
	notifications = notifications.filter((n) => n.id !== notif.id);
	try {
		const res = await fetch(`/api/inbox/${notif.id}/archive`, {
			method: "POST",
		});
		if (!res.ok) {
			console.error(`Failed to archive: ${res.status} ${res.statusText}`);
			showError("Failed to archive notification");
			notifications = [...notifications, notif];
		}
	} catch (err) {
		console.error("Failed to archive:", err);
		showError("Failed to archive notification");
		notifications = [...notifications, notif];
	}
}

async function handleUnarchive(e, notif) {
	e.stopPropagation();
	notifications = notifications.filter((n) => n.id !== notif.id);
	try {
		const res = await fetch(`/api/inbox/${notif.id}/unarchive`, {
			method: "POST",
		});
		if (!res.ok) {
			console.error(`Failed to unarchive: ${res.status} ${res.statusText}`);
			showError("Failed to unarchive notification");
			notifications = [...notifications, notif];
		}
	} catch (err) {
		console.error("Failed to unarchive:", err);
		showError("Failed to unarchive notification");
		notifications = [...notifications, notif];
	}
}
</script>

<div class="main">
  <div class="list-header">
    <span class="list-title">{viewTitle}</span>
    <span class="list-count">{count}{#if currentView !== "archived"} · {unreadCount} unread{/if}</span>
    <div class="list-spacer"></div>
    <button class="filter-btn">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M.75 3h14.5a.75.75 0 0 1 0 1.5H.75a.75.75 0 0 1 0-1.5ZM3 7.75A.75.75 0 0 1 3.75 7h8.5a.75.75 0 0 1 0 1.5h-8.5A.75.75 0 0 1 3 7.75Zm3 4a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z"/></svg>
      Filter
    </button>
  </div>

  <div class="pr-list">
    {#if count === 0}
      <div class="empty-state">
        {emptyMessage}
      </div>
    {:else}
      {#each notifications as notif (notif.id)}
        <div class="pr-item" class:read={!notif.unread} class:selected={notif.id === selectedId} onclick={() => handleSelect(notif)} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && handleSelect(notif)}>
          <div class="unread-dot" class:read={!notif.unread}></div>
          <div class="pr-body">
            <div class="pr-meta-top">
              <span class="pr-repo">{notif.repository}</span>
            </div>
            <div class="pr-title">
              {#if notif.pr_id}<span class="pr-num">#{notif.pr_id}</span>{/if}{notif.title}
            </div>
          </div>
          <div class="pr-reason">
            <span class="label label-{reasonClass(notif.reason)}">{reasonLabel(notif.reason)}</span>
          </div>
          <div class="pr-date">{timeAgo(notif.updated_at)}</div>
          <div class="pr-actions">
            {#if currentView === "inbox"}
              <button class="action-btn" title="Archive" onclick={(e) => handleArchive(e, notif)}>
                <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M1.75 1h12.5c.966 0 1.75.784 1.75 1.75v2.5A1.75 1.75 0 0 1 14.25 7H1.75A1.75 1.75 0 0 1 0 5.25v-2.5C0 1.784.784 1 1.75 1Zm0 1.5a.25.25 0 0 0-.25.25v2.5c0 .138.112.25.25.25h12.5a.25.25 0 0 0 .25-.25v-2.5a.25.25 0 0 0-.25-.25ZM1 8.75v5.5c0 .966.784 1.75 1.75 1.75h10.5A1.75 1.75 0 0 0 15 14.25v-5.5a.75.75 0 0 0-1.5 0v5.5a.25.25 0 0 1-.25.25H2.75a.25.25 0 0 1-.25-.25v-5.5a.75.75 0 0 0-1.5 0ZM5 10.25a.75.75 0 0 1 .75-.75h4.5a.75.75 0 0 1 0 1.5h-4.5a.75.75 0 0 1-.75-.75Z"/></svg>
              </button>
            {:else}
              <button class="action-btn" title="Unarchive" onclick={(e) => handleUnarchive(e, notif)}>
                <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v9.5A1.75 1.75 0 0 1 14.25 13H8.06l-2.573 2.573A1.458 1.458 0 0 1 3 14.543V13H1.75A1.75 1.75 0 0 1 0 11.25Zm1.75-.25a.25.25 0 0 0-.25.25v9.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-9.5a.25.25 0 0 0-.25-.25Z"/></svg>
              </button>
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </div>

  <div class="statusbar">
    <span>{count} PRs{#if currentView !== "archived"} · {unreadCount} unread{/if}</span>
  </div>
</div>

<style>
  .main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .list-header {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-default);
    gap: 8px;
    flex-shrink: 0;
  }
  .list-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--fg-default);
  }
  .list-count {
    font-size: 12px;
    color: var(--fg-muted);
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 2em;
    padding: 0 8px;
    line-height: 20px;
  }
  .list-spacer { flex: 1; }
  .filter-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 500;
    color: var(--fg-default);
    cursor: pointer;
    font-family: inherit;
  }
  .filter-btn:hover { background: var(--border-muted); }
  .pr-list {
    flex: 1;
    overflow-y: auto;
  }
  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--fg-muted);
    font-size: 14px;
  }

  /* PR row */
  .pr-item {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border-muted);
    cursor: pointer;
    gap: 12px;
  }
  .pr-item:hover { background: var(--canvas-subtle); }
  .pr-item.selected { background: var(--accent-subtle); border-left: 2px solid var(--accent-fg); }

  .unread-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-fg);
    flex-shrink: 0;
  }
  .unread-dot.read { background: transparent; }

  .pr-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .pr-meta-top {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pr-repo {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
    white-space: nowrap;
  }
  .pr-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--fg-default);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pr-item.read .pr-title {
    font-weight: 400;
    color: var(--fg-muted);
  }
  .pr-num {
    font-size: 12px;
    font-weight: 400;
    color: var(--fg-subtle);
    margin-right: 4px;
  }

  /* Reason pill */
  .pr-reason {
    flex-shrink: 0;
    width: 152px;
    display: flex;
    justify-content: flex-end;
    padding-right: 16px;
  }
  .label {
    display: inline-flex;
    align-items: center;
    font-size: 12px;
    font-weight: 500;
    padding: 0 8px;
    border-radius: 2em;
    border: 1px solid;
    white-space: nowrap;
    line-height: 20px;
  }
  .label-review  { border-color: rgba(47,129,247,0.4); background: rgba(47,129,247,0.1); color: #79c0ff; }
  .label-mention { border-color: rgba(163,113,247,0.4); background: rgba(163,113,247,0.1); color: #c9b1f7; }
  .label-assign  { border-color: rgba(210,153,34,0.4); background: rgba(210,153,34,0.1); color: #e3b341; }
  .label-default { border-color: var(--border-default); background: var(--canvas-subtle); color: var(--fg-muted); }

  /* Date */
  .pr-date {
    flex-shrink: 0;
    width: 88px;
    display: flex;
    justify-content: flex-end;
    font-size: 12px;
    color: var(--fg-subtle);
    white-space: nowrap;
  }

  /* Action buttons */
  .pr-actions {
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }
  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: 1px solid var(--border-default);
    border-radius: 6px;
    background: var(--canvas-subtle);
    color: var(--fg-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.1s;
  }
  .pr-item:hover .action-btn {
    opacity: 1;
  }
  .action-btn:hover {
    background: var(--border-muted);
    color: var(--fg-default);
  }

  .statusbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 16px;
    height: 28px;
    border-top: 1px solid var(--border-default);
    background: var(--canvas-subtle);
    flex-shrink: 0;
    color: var(--fg-subtle);
    font-size: 12px;
  }
</style>
