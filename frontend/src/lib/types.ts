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
    state: string;
    head_sha: string;
    additions: number;
    deletions: number;
    changed_files: number;
}

export interface Comment {
    id: number;
    pr_id: number;
    thread_id: string;
    author: string;
    body: string;
    created_at: string;
    comment_type: string;
    path: string | null;
    position: number | null;
    in_reply_to_id: number | null;
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
}
