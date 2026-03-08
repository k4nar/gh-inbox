<header class="topbar">
  <div class="topbar-logo">
    <svg width="20" height="20" viewBox="0 0 16 16" fill="none">
      <rect x="1" y="1" width="14" height="14" rx="3" stroke="var(--fg-muted)" stroke-width="1.3"/>
      <path d="M4 5h8M4 8h5M4 11h6" stroke="var(--accent-fg)" stroke-width="1.5" stroke-linecap="round"/>
    </svg>
    gh-inbox
  </div>
  <div class="topbar-spacer"></div>
  <div class="topbar-sync">
    <div class="sync-dot"></div>
    synced just now
  </div>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 16px;
    height: 48px;
    border-bottom: 1px solid var(--border-default);
    background: var(--canvas-default);
    flex-shrink: 0;
  }
  .topbar-logo {
    font-size: 14px;
    font-weight: 600;
    color: var(--fg-default);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .topbar-spacer { flex: 1; }
  .topbar-sync {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .sync-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--success-fg);
  }
</style>
