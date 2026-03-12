const REASON_LABELS: Record<string, string> = {
    review_requested: "Review requested",
    mention: "Mentioned",
    assign: "Assigned",
    team_mention: "Team mentioned",
    subscribed: "Subscribed",
    author: "Author",
    comment: "Commented",
    ci_activity: "CI activity",
    manual: "Manual",
    state_change: "State changed",
};

export function reasonLabel(reason: string): string {
    return REASON_LABELS[reason] ?? reason;
}

export function reasonClass(reason: string): string {
    switch (reason) {
        case "review_requested":
            return "review";
        case "mention":
        case "team_mention":
            return "mention";
        case "assign":
            return "assign";
        default:
            return "default";
    }
}
