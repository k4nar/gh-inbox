/**
 * Maps a GitHub notification reason string to a human-readable label.
 */
const REASON_LABELS = {
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

/**
 * @param {string} reason - GitHub notification reason
 * @returns {string} Human-readable label
 */
export function reasonLabel(reason) {
	return REASON_LABELS[reason] ?? reason;
}

/**
 * Maps a reason to a CSS class suffix for pill styling.
 * @param {string} reason
 * @returns {string}
 */
export function reasonClass(reason) {
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
