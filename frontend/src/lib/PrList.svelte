<script>
import { onMount } from "svelte";
import { reasonClass, reasonLabel } from "./reason.js";
import { timeAgo } from "./timeago.js";

let notifications = $state([]);

onMount(async () => {
	const res = await fetch("/api/inbox");
	if (res.ok) {
		notifications = await res.json();
	}
});

let count = $derived(notifications.length);
let unreadCount = $derived(notifications.filter((n) => n.unread).length);
</script>

<div class="main">
  <div class="list-header">
    <span class="list-title">Inbox</span>
    <span class="list-count">{count} · {unreadCount} unread</span>
    <div class="list-spacer"></div>
    <button class="filter-btn">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M.75 3h14.5a.75.75 0 0 1 0 1.5H.75a.75.75 0 0 1 0-1.5ZM3 7.75A.75.75 0 0 1 3.75 7h8.5a.75.75 0 0 1 0 1.5h-8.5A.75.75 0 0 1 3 7.75Zm3 4a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z"/></svg>
      Filter
    </button>
  </div>

  <div class="pr-list">
    {#if count === 0}
      <div class="empty-state">
        No notifications yet.
      </div>
    {:else}
      {#each notifications as notif (notif.id)}
        <div class="pr-item" class:read={!notif.unread}>
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
        </div>
      {/each}
    {/if}
  </div>

  <div class="statusbar">
    <span>{count} PRs · {unreadCount} unread</span>
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
