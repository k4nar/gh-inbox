export interface Notification {
    id: string;
    pr_id: number | null;
    title: string;
    repository: string;
    reason: string;
    unread: boolean;
    archived: boolean;
    updated_at: string;
}

export interface PullRequest {
    id: number;
    title: string;
    repo: string;
    author: string;
    url: string;
    ci_status: string;
    last_viewed_at: string | null;
    body: string;
    body_html: string;
    state: string;
    head_sha: string;
    additions: number;
    deletions: number;
    changed_files: number;
    draft: boolean;
    merged_at: string | null;
}

export interface Comment {
    id: number;
    pr_id: number;
    thread_id: string;
    author: string;
    body: string;
    body_html: string;
    created_at: string;
    comment_type: string;
    path: string | null;
    position: number | null;
    in_reply_to_id: number | null;
    html_url: string | null;
    diff_hunk: string | null;
}

export interface CheckRun {
    name: string;
    status: string;
    conclusion: string | null;
}

export interface Commit {
    sha: string;
    pr_id: number;
    message: string;
    author: string;
    committed_at: string;
}

export interface Thread {
    thread_id: string;
    path: string | null;
    comments: Comment[];
}

export interface PrDetailResponse {
    pull_request: PullRequest;
    comments: Comment[];
    commits: Commit[];
    check_runs: CheckRun[];
    previous_viewed_at: string | null;
}

export interface InboxItem {
    id: string;
    pr_id: number | null;
    title: string;
    repository: string;
    reason: string;
    unread: boolean;
    archived: boolean;
    updated_at: string;
    author: string | null;
    pr_status: "open" | "draft" | "merged" | "closed" | null;
    new_commits: number | null; // null = first visit (never opened)
    new_comments: { author: string; count: number }[] | null; // null = first visit
    teams: string[] | null; // null = loading (show shimmer)
}

export const DEFAULT_PER_PAGE = 20;

export interface PaginatedInbox {
    items: InboxItem[];
    total: number;
    page: number;
    per_page: number;
}
