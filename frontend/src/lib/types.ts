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
    author_avatar_url: string | null;
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
    author_avatar_url: string | null;
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

export interface Review {
    id: number;
    reviewer: string;
    reviewer_avatar_url: string | null;
    state: string; // "APPROVED" | "CHANGES_REQUESTED"
    body: string;
    submitted_at: string;
    html_url: string;
}

export interface Label {
    name: string;
    color: string;
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
    resolved: boolean;
    comments: Comment[];
}

export interface PrDetailResponse {
    pull_request: PullRequest;
    threads: Thread[];
    commits: Commit[];
    check_runs: CheckRun[];
    previous_viewed_at: string | null;
    reviews: Review[];
    labels: Label[];
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
    author_avatar_url: string | null;
    pr_status: "open" | "draft" | "merged" | "closed" | null;
    ci_status: string | null;
    teams: string[] | null; // null = loading (show shimmer)
    // Activity fields — populated via SSE pr:info_updated, not from the inbox API.
    new_commits: number | null; // null = not yet enriched or first visit
    new_comments: { author: string; count: number }[] | null;
    new_reviews: { reviewer: string; state: string }[] | null;
}

export const DEFAULT_PER_PAGE = 20;

export interface PaginatedInbox {
    items: InboxItem[];
    total: number;
    page: number;
    per_page: number;
}

export type Theme =
    | "system"
    | "light"
    | "dark"
    | "catppuccin-latte"
    | "catppuccin-frappe"
    | "catppuccin-macchiato"
    | "catppuccin-mocha";

export interface Preferences {
    theme: Theme;
}
