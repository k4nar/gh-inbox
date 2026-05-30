<script lang="ts">
import { Collapsible, Tabs } from "bits-ui";
import type { ActiveFilters, FilterOptions } from "./types.ts";

let {
    currentView = "inbox",
    onViewChange = (_view: string) => {},
    options = {
        repos: [],
        orgs: [],
        teams: [],
        authors: [],
        repo_counts: {},
        team_counts: {},
    } as FilterOptions,
    activeFilters = {} as ActiveFilters,
    onFiltersChange = (_f: ActiveFilters) => {},
}: {
    currentView?: string;
    onViewChange?: (view: string) => void;
    options?: FilterOptions;
    activeFilters?: ActiveFilters;
    onFiltersChange?: (f: ActiveFilters) => void;
} = $props();

const FILTER_STATES = ["open", "draft", "merged", "closed"] as const;

// SVG path data for GitHub Octicons (16px) — PR state icons.
const STATUS_ICONS: Record<string, string> = {
    open: "M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354ZM3.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm8.25.75a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Z",
    draft: "M3.25 1A2.25 2.25 0 0 1 4 5.372v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 3.25 1Zm9.5 14a2.25 2.25 0 1 1 0-4.5 2.25 2.25 0 0 1 0 4.5ZM3.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm9.5 0a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z",
    merged: "M5.45 5.154A4.25 4.25 0 0 0 9.25 7.5h1.378a2.251 2.251 0 1 1 0 1.5H9.25A5.734 5.734 0 0 1 5 7.123v3.505a2.25 2.25 0 1 1-1.5 0V5.372A2.25 2.25 0 1 1 5.45 5.154ZM4.25 13.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm8.5-4.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5ZM5 3.25a.75.75 0 1 0 0 .005V3.25Z",
    closed: "M3.25 1A2.25 2.25 0 0 1 4 5.372v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 3.25 1Zm9.96 5.016a.75.75 0 1 0-1.06-1.06L10.5 6.61 8.84 4.94a.75.75 0 0 0-1.061 1.06l1.661 1.661-1.661 1.661a.75.75 0 1 0 1.06 1.06L10.5 8.72l1.661 1.661a.75.75 0 1 0 1.06-1.06L11.56 7.66l1.65-1.644ZM3.25 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z",
};

const hasActiveFilters = $derived(
    Object.values(activeFilters).some((v) => v !== undefined && v !== ""),
);

function handleRepoClick(repo: string) {
    if (activeFilters.repo === repo) {
        onFiltersChange({ ...activeFilters, repo: undefined, org: undefined });
    } else {
        onFiltersChange({ ...activeFilters, repo, org: undefined });
    }
}

function handleTeamClick(team: string) {
    if (activeFilters.team === team) {
        onFiltersChange({ ...activeFilters, team: undefined });
    } else {
        onFiltersChange({ ...activeFilters, team });
    }
}

function handleAuthorClick(author: string) {
    if (activeFilters.author === author) {
        onFiltersChange({ ...activeFilters, author: undefined });
    } else {
        onFiltersChange({ ...activeFilters, author });
    }
}

// Toggle a status between the given mode and neutral. Each status can be in at
// most one mode ("include" = show only, "exclude" = hide), so setting one mode
// replaces the other.
function toggleStateMode(state: string, mode: "include" | "exclude") {
    const states = { ...(activeFilters.states ?? {}) };
    if (states[state] === mode) {
        delete states[state];
    } else {
        states[state] = mode;
    }
    onFiltersChange({
        ...activeFilters,
        states: Object.keys(states).length > 0 ? states : undefined,
    });
}

function clearFilters() {
    onFiltersChange({});
}

// Collapsible list sections are collapsed by default. When collapsed, a section
// with an active filter shows only the active item (handled in CSS) and its
// label is highlighted; expanding reveals the full list.
let reposOpen = $state(false);
let teamsOpen = $state(false);
let authorsOpen = $state(false);
</script>

{#snippet sectionHeader(title: string, count: number, active: boolean)}
    <span class="sidebar-section-title" class:active>{title}</span>
    <span class="sidebar-section-badge">{count}</span>
    <svg
        aria-hidden="true"
        class="sidebar-chevron"
        width="12"
        height="12"
        viewBox="0 0 16 16"
        fill="currentColor"
    >
        <path
            d="M6.22 3.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L9.94 8 6.22 4.28a.75.75 0 0 1 0-1.06Z"
        />
    </svg>
{/snippet}

<nav class="sidebar">
    <Tabs.Root value={currentView} onValueChange={onViewChange}>
        <Tabs.List class="sidebar-section">
            <Tabs.Trigger value="inbox" class="sidebar-item">
                <svg
                    aria-hidden="true"
                    width="16"
                    height="16"
                    viewBox="0 0 16 16"
                    fill="currentColor"
                >
                    <path
                        d="M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v9.5A1.75 1.75 0 0 1 14.25 13H8.06l-2.573 2.573A1.458 1.458 0 0 1 3 14.543V13H1.75A1.75 1.75 0 0 1 0 11.25Zm1.75-.25a.25.25 0 0 0-.25.25v9.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h6.5a.25.25 0 0 0 .25-.25v-9.5a.25.25 0 0 0-.25-.25Z"
                    />
                </svg>
                Inbox
            </Tabs.Trigger>
            <Tabs.Trigger value="archived" class="sidebar-item">
                <svg
                    aria-hidden="true"
                    width="16"
                    height="16"
                    viewBox="0 0 16 16"
                    fill="currentColor"
                >
                    <path
                        d="M1.75 2.5h12.5a.25.25 0 0 1 .25.25v7.5a.25.25 0 0 1-.25.25H7.5a.75.75 0 0 0-.53.22L4.5 12.94V11.25a.75.75 0 0 0-.75-.75h-2a.25.25 0 0 1-.25-.25v-7.5a.25.25 0 0 1 .25-.25ZM14.25 1H1.75A1.75 1.75 0 0 0 0 2.75v7.5C0 11.216.784 12 1.75 12H3v1.543a1.457 1.457 0 0 0 2.487 1.03L8.06 12h6.19A1.75 1.75 0 0 0 16 10.25v-7.5A1.75 1.75 0 0 0 14.25 1Z"
                    />
                </svg>
                Archived
            </Tabs.Trigger>
        </Tabs.List>
    </Tabs.Root>

    {#if hasActiveFilters}
        <div class="sidebar-clear">
            <button
                type="button"
                class="sidebar-clear-btn"
                onclick={clearFilters}
            >
                Clear filters
            </button>
        </div>
    {/if}

    <div class="sidebar-section">
        <div class="sidebar-label">Status</div>
        <div class="sidebar-status">
            {#each FILTER_STATES as state}
                {@const mode = activeFilters.states?.[state]}
                <div class="status-pill" data-mode={mode ?? "neutral"}>
                    <button
                        type="button"
                        class="status-pill-main"
                        aria-pressed={mode === "include"}
                        title="Show only {state}"
                        onclick={() => toggleStateMode(state, "include")}
                    >
                        <svg
                            aria-hidden="true"
                            class="status-pill-icon status-pill-icon-{state}"
                            width="14"
                            height="14"
                            viewBox="0 0 16 16"
                            fill="currentColor"
                        >
                            <path d={STATUS_ICONS[state]} />
                        </svg>
                        {state}
                    </button>
                    <button
                        type="button"
                        class="status-pill-hide"
                        aria-label="Hide {state}"
                        aria-pressed={mode === "exclude"}
                        title="Hide {state}"
                        onclick={() => toggleStateMode(state, "exclude")}
                    >
                        <svg
                            aria-hidden="true"
                            width="13"
                            height="13"
                            viewBox="0 0 16 16"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.4"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        >
                            <path
                                d="M.5 8C2 5 4.5 3.5 8 3.5S14 5 15.5 8C14 11 11.5 12.5 8 12.5S2 11 .5 8Z"
                            />
                            <circle cx="8" cy="8" r="1.9" />
                            <line x1="2.5" y1="13.5" x2="13.5" y2="2.5" />
                        </svg>
                    </button>
                </div>
            {/each}
        </div>
    </div>

    {#if options.repos.length > 0}
        <Collapsible.Root
            open={reposOpen}
            onOpenChange={(v) => (reposOpen = v)}
        >
            <Collapsible.Trigger class="sidebar-section-header">
                {@render sectionHeader(
                    "Repositories",
                    options.repos.length,
                    activeFilters.repo !== undefined,
                )}
            </Collapsible.Trigger>
            <Collapsible.Content class="sidebar-section-content" forceMount>
                {#each options.repos as repo}
                    {@const isActive = activeFilters.repo === repo}
                    {@const org = repo.split("/")[0]}
                    <button
                        type="button"
                        class="sidebar-item"
                        data-state={isActive ? "active" : "inactive"}
                        title={repo}
                        onclick={() => handleRepoClick(repo)}
                    >
                        <img
                            class="sidebar-avatar"
                            src="https://github.com/{org}.png?size=32"
                            alt=""
                            width="16"
                            height="16"
                        >
                        <span class="sidebar-item-label">{repo}</span>
                        {#if options.repo_counts[repo]}
                            <span class="sidebar-count"
                                >{options.repo_counts[repo]}</span
                            >
                        {/if}
                    </button>
                {/each}
            </Collapsible.Content>
        </Collapsible.Root>
    {/if}

    {#if options.teams.length > 0}
        <Collapsible.Root
            open={teamsOpen}
            onOpenChange={(v) => (teamsOpen = v)}
        >
            <Collapsible.Trigger class="sidebar-section-header">
                {@render sectionHeader(
                    "Codeowner Teams",
                    options.teams.length,
                    activeFilters.team !== undefined,
                )}
            </Collapsible.Trigger>
            <Collapsible.Content class="sidebar-section-content" forceMount>
                {#each options.teams as team}
                    {@const isActive = activeFilters.team === team}
                    {@const org = team.split("/")[0]}
                    <button
                        type="button"
                        class="sidebar-item"
                        data-state={isActive ? "active" : "inactive"}
                        title={team}
                        onclick={() => handleTeamClick(team)}
                    >
                        <img
                            class="sidebar-avatar"
                            src="https://github.com/{org}.png?size=32"
                            alt=""
                            width="16"
                            height="16"
                        >
                        <span class="sidebar-item-label">{team}</span>
                        {#if options.team_counts[team]}
                            <span class="sidebar-count"
                                >{options.team_counts[team]}</span
                            >
                        {/if}
                    </button>
                {/each}
            </Collapsible.Content>
        </Collapsible.Root>
    {/if}

    {#if options.authors.length > 0}
        <Collapsible.Root
            open={authorsOpen}
            onOpenChange={(v) => (authorsOpen = v)}
        >
            <Collapsible.Trigger class="sidebar-section-header">
                {@render sectionHeader(
                    "Authors",
                    options.authors.length,
                    activeFilters.author !== undefined,
                )}
            </Collapsible.Trigger>
            <Collapsible.Content class="sidebar-section-content" forceMount>
                {#each options.authors as author}
                    {@const isActive = activeFilters.author === author}
                    <button
                        type="button"
                        class="sidebar-item"
                        data-state={isActive ? "active" : "inactive"}
                        title={author}
                        onclick={() => handleAuthorClick(author)}
                    >
                        <img
                            class="sidebar-avatar"
                            src="https://github.com/{author}.png?size=32"
                            alt=""
                            width="16"
                            height="16"
                        >
                        <span class="sidebar-item-label">{author}</span>
                    </button>
                {/each}
            </Collapsible.Content>
        </Collapsible.Root>
    {/if}
</nav>

<style>
.sidebar {
    width: var(--sidebar-w);
    background: var(--canvas-default);
    border-right: 1px solid var(--border-default);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    overflow-y: auto;
    padding: 16px 0;
    gap: 24px;
}
.sidebar-section {
    display: flex;
    flex-direction: column;
    gap: 1px;
}
.sidebar-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--fg-muted);
    padding: 0 16px;
    margin-bottom: 4px;
}
.sidebar-avatar {
    border-radius: 3px;
    flex-shrink: 0;
}
.sidebar-item-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.sidebar-count {
    font-size: 11px;
    color: var(--fg-muted);
    font-weight: 400;
    flex-shrink: 0;
}
/* Collapsible list sections — header (Bits Trigger) + content (Bits Content)
   are styled via :global because the class lands on elements inside Bits UI. */
:global(.sidebar-section-header) {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 4px 16px;
    background: none;
    border: none;
    cursor: pointer;
    font-family: inherit;
    color: var(--fg-muted);
    font-size: 12px;
    font-weight: 600;
    text-align: left;
}
:global(.sidebar-section-header:hover) {
    color: var(--fg-default);
}
:global(.sidebar-section-content) {
    display: flex;
    flex-direction: column;
    gap: 1px;
}
/* When a section is collapsed, hide every item except the active one, so the
   collapsed header still shows the current selection. */
:global(
        .sidebar-section-content[data-state="closed"]
        .sidebar-item[data-state="inactive"]
    ) {
    display: none;
}
.sidebar-section-title {
    flex-shrink: 0;
}
.sidebar-section-title.active {
    color: var(--accent-fg);
}
.sidebar-section-badge {
    font-size: 11px;
    font-weight: 400;
    color: var(--fg-muted);
    background: var(--canvas-subtle);
    border: 1px solid var(--border-default);
    border-radius: 2em;
    padding: 0 6px;
    line-height: 16px;
}
.sidebar-chevron {
    margin-left: auto;
    flex-shrink: 0;
    color: var(--fg-muted);
    transition: transform 0.15s ease;
}
:global(.sidebar-section-header[data-state="open"]) .sidebar-chevron {
    transform: rotate(90deg);
}
.sidebar-clear {
    padding: 0 16px;
    margin-top: -12px;
}
.sidebar-clear-btn {
    font-size: 12px;
    color: var(--accent-fg);
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    font-family: inherit;
}
.sidebar-clear-btn:hover {
    text-decoration: underline;
}
.sidebar-status {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 0 16px;
}
.status-pill {
    display: inline-flex;
    align-items: stretch;
    font-size: 12px;
    border: 1px solid var(--border-default);
    border-radius: 2em;
    background: var(--canvas-subtle);
    overflow: hidden;
}
.status-pill-main {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 4px 3px 8px;
    background: none;
    border: none;
    color: var(--fg-muted);
    cursor: pointer;
    font-family: inherit;
    font-size: inherit;
    text-transform: capitalize;
}
.status-pill-hide {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0 7px 0 2px;
    background: none;
    border: none;
    color: var(--fg-subtle);
    cursor: pointer;
}
/* The eye-off icon is revealed only on hover or when the status is hidden. */
.status-pill-hide svg {
    opacity: 0;
    transition: opacity 0.12s ease;
}
.status-pill:hover .status-pill-hide svg,
.status-pill[data-mode="exclude"] .status-pill-hide svg {
    opacity: 1;
}
.status-pill-main:hover {
    color: var(--fg-default);
}
.status-pill-hide:hover {
    color: var(--fg-default);
}
/* Include: only these statuses are shown. */
.status-pill[data-mode="include"] {
    border-color: var(--accent-fg);
    background: var(--accent-subtle);
}
.status-pill[data-mode="include"] .status-pill-main {
    color: var(--fg-default);
}
/* Exclude: this status is hidden — dim and strike through the label. */
.status-pill[data-mode="exclude"] .status-pill-main {
    color: var(--fg-subtle);
    text-decoration: line-through;
}
.status-pill[data-mode="exclude"] .status-pill-hide {
    color: #f85149;
}
.status-pill-icon {
    flex-shrink: 0;
}
.status-pill-icon-open {
    color: #3fb950;
}
.status-pill-icon-draft {
    color: var(--fg-muted);
}
.status-pill-icon-merged {
    color: #a371f7;
}
.status-pill-icon-closed {
    color: #f85149;
}
/* Dim the status icon when the status is hidden (declared after the base/color
   rules above to keep specificity non-descending). */
.status-pill[data-mode="exclude"] .status-pill-icon {
    opacity: 0.5;
}
:global(.sidebar-item) {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px;
    cursor: pointer;
    color: var(--fg-muted);
    font-size: 14px;
    user-select: none;
    position: relative;
    border-radius: 0;
    text-decoration: none;
    background: none;
    border: none;
    font-family: inherit;
    text-align: left;
    width: 100%;
}
:global(.sidebar-item:hover) {
    background: var(--canvas-subtle);
    color: var(--fg-default);
}
:global(.sidebar-item[data-state="active"]) {
    color: var(--fg-default);
    font-weight: 600;
    background: var(--canvas-subtle);
}
:global(.sidebar-item[data-state="active"]::before) {
    content: "";
    position: absolute;
    left: 0;
    top: 4px;
    bottom: 4px;
    width: 2px;
    background: var(--accent-fg);
    border-radius: 0 2px 2px 0;
}
</style>
