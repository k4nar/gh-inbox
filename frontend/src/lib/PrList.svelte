<div class="main">
  <div class="list-header">
    <span class="list-title">Inbox</span>
    <span class="list-count">0 PRs</span>
    <div class="list-spacer"></div>
    <button class="filter-btn">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M.75 3h14.5a.75.75 0 0 1 0 1.5H.75a.75.75 0 0 1 0-1.5ZM3 7.75A.75.75 0 0 1 3.75 7h8.5a.75.75 0 0 1 0 1.5h-8.5A.75.75 0 0 1 3 7.75Zm3 4a.75.75 0 0 1 .75-.75h2.5a.75.75 0 0 1 0 1.5h-2.5a.75.75 0 0 1-.75-.75Z"/></svg>
      Filter
    </button>
  </div>

  <div class="pr-list">
    <div class="empty-state">
      No notifications yet.
    </div>
  </div>

  <div class="statusbar">
    <span>0 PRs</span>
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
