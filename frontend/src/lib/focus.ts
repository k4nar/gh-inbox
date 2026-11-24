// Focus handoff between the inbox list and the PR detail panel.
//
// Both move keyboard focus on their own (the list to the cursor row, the
// panel into the PR timeline), but only when the user has not put focus on
// some other control (a sidebar button, a link in the panel).

/** Marks an inbox row element; PrListItem sets it, the list queries it. */
export const ROW_ATTR = "data-notif-id";

export function rowSelector(id?: string): string {
    return id === undefined ? `[${ROW_ATTR}]` : `[${ROW_ATTR}="${id}"]`;
}

/** True when focus may be moved: it is on the page itself, or on an inbox
 * row or a control inside one. */
export function focusIsFree(): boolean {
    const active = document.activeElement;
    return (
        active === null ||
        active === document.body ||
        active.closest(rowSelector()) !== null
    );
}
