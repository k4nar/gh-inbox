<script lang="ts">
import { timeAgo } from "./timeago.ts";
import type { Commit } from "./types.ts";

let {
    commit,
    repo,
    isNew,
}: {
    commit: Commit;
    repo: string;
    isNew: boolean;
} = $props();

function commitUrl(repo: string, sha: string): string {
    return `https://github.com/${repo}/commit/${sha}`;
}
</script>

<a
    class="commit-row {isNew ? 'commit-row-new' : 'commit-row-old'}"
    href={commitUrl(repo, commit.sha)}
    target="_blank"
    rel="noopener noreferrer"
>
    <span class="commit-icon" class:commit-icon-old={!isNew}>⬆</span>
    <span class="commit-sha" class:commit-sha-old={!isNew}
        >{commit.sha.slice(0, 7)}</span
    >
    <span class="commit-message" class:commit-message-old={!isNew}
        >{commit.message}</span
    >
    <span class="commit-date">{timeAgo(commit.committed_at)}</span>
</a>

<style>
.commit-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 12px;
    border: 1px solid var(--border-default);
    text-decoration: none;
    color: inherit;
}

.commit-row:hover {
    background: var(--canvas-subtle);
    border-color: var(--border-muted);
}

.commit-row-new {
    background: rgba(47, 129, 247, 0.06);
    border-color: rgba(47, 129, 247, 0.2);
}

.commit-icon {
    color: var(--accent-fg);
    flex-shrink: 0;
    font-size: 13px;
}

.commit-icon-old {
    color: var(--fg-muted);
}

.commit-sha {
    font-family:
        ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
    font-size: 11px;
    color: var(--accent-fg);
    flex-shrink: 0;
}

.commit-sha-old {
    color: var(--fg-muted);
}

.commit-message {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg-default);
}

.commit-message-old {
    color: var(--fg-muted);
}

.commit-date {
    font-size: 11px;
    color: var(--fg-subtle);
    flex-shrink: 0;
}
</style>
