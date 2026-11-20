// Global keyboard shortcuts (Gmail / GitHub style).
//
// Components register the shortcuts they own from an `$effect` and get back
// an unregister function, so a shortcut only exists while its component is
// mounted. A single `keydown` listener on `window` is installed while at
// least one shortcut is registered.
//
// A key is matched on `event.key`, so shortcuts follow the keyboard layout
// ("?" works wherever "?" is). Shift+letter is written as the uppercase
// letter ("I"). Caps Lock is ignored for letters, so it never turns "o"
// into "O". Sequences are space-separated ("g i").

export interface Shortcut {
    /** Alternative bindings for the same action, e.g. ["o", "Enter"]. */
    keys: string[];
    description: string;
    group: string;
    /** The shortcut is skipped (and the key falls through) when this is false. */
    when?: () => boolean;
    /** Keep firing while the key is held (auto-repeat). Off by default, so
     * holding "e" archives one row, not the whole page. */
    repeat?: boolean;
    run: () => void;
}

export const SEQUENCE_TIMEOUT_MS = 1000;

// Registration order matters: when two active shortcuts share a key, the
// most recently registered one wins.
let registry: Shortcut[] = [];
let pending: string[] = [];
let pendingTimer: ReturnType<typeof setTimeout> | null = null;
let listening = false;

export function registerShortcuts(shortcuts: Shortcut[]): () => void {
    registry = [...registry, ...shortcuts];
    if (!listening) {
        window.addEventListener("keydown", handleKeydown);
        listening = true;
    }
    return () => {
        registry = registry.filter((s) => !shortcuts.includes(s));
        if (registry.length === 0 && listening) {
            window.removeEventListener("keydown", handleKeydown);
            listening = false;
            resetPending();
        }
    };
}

/** Every registered shortcut, for the help dialog. */
export function listShortcuts(): Shortcut[] {
    return [...registry];
}

const KEY_LABELS: Record<string, string> = {
    Escape: "Esc",
    ArrowUp: "↑",
    ArrowDown: "↓",
};

/** Display labels for one binding's keys: "I" -> ["Shift+I"], "g i" -> ["g", "i"]. */
export function formatKeys(binding: string): string[] {
    return binding
        .split(" ")
        .map((k) => (/^[A-Z]$/.test(k) ? `Shift+${k}` : (KEY_LABELS[k] ?? k)));
}

function resetPending(): void {
    pending = [];
    if (pendingTimer) {
        clearTimeout(pendingTimer);
        pendingTimer = null;
    }
}

function isEditable(el: Element): boolean {
    return (
        el instanceof HTMLInputElement ||
        el instanceof HTMLTextAreaElement ||
        el instanceof HTMLSelectElement ||
        (el instanceof HTMLElement && el.isContentEditable)
    );
}

// Enter and Space already activate a focused button or link; a shortcut on
// the same key would fire twice.
function isActivatable(el: Element): boolean {
    return el.closest("button, a[href], [role='button'], summary") !== null;
}

// Open dialogs and menus own the keyboard (Escape closes them, letters do
// type-ahead), so global shortcuts stand down while one is open. Focus is
// either inside the popup (dialogs, menus) or stays on its expanded trigger
// (Bits UI Select keeps it on the combobox), so check both.
function isOverlayOpen(el: Element): boolean {
    return (
        el.closest(
            "[role='dialog'], [role='alertdialog'], [role='menu'], [role='listbox']",
        ) !== null ||
        document.querySelector("[aria-haspopup][aria-expanded='true']") !== null
    );
}

// Letters follow Shift only: with Caps Lock on, browsers report "O" for a
// plain o press.
function normalizeKey(event: KeyboardEvent): string {
    const { key } = event;
    if (/^[a-zA-Z]$/.test(key) && event.getModifierState?.("CapsLock")) {
        return event.shiftKey ? key.toUpperCase() : key.toLowerCase();
    }
    return key;
}

function findActive(sequence: string): Shortcut | undefined {
    for (let i = registry.length - 1; i >= 0; i--) {
        const s = registry[i];
        if (s.keys.includes(sequence) && (s.when?.() ?? true)) return s;
    }
    return undefined;
}

function isPrefix(sequence: string): boolean {
    const prefix = `${sequence} `;
    return registry.some(
        (s) => s.keys.some((k) => k.startsWith(prefix)) && (s.when?.() ?? true),
    );
}

function handleKeydown(event: KeyboardEvent): void {
    if (event.defaultPrevented || event.isComposing) return;
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    // Bare modifier presses (the Shift of Shift+I) must not break a sequence.
    if (["Shift", "Control", "Alt", "Meta"].includes(event.key)) return;

    const target = event.target;
    if (target instanceof Element) {
        if (isEditable(target) || isOverlayOpen(target)) return;
        if (
            (event.key === "Enter" || event.key === " ") &&
            isActivatable(target)
        )
            return;
    }

    const key = normalizeKey(event);
    const candidates =
        pending.length > 0 ? [[...pending, key].join(" "), key] : [key];
    resetPending();

    // Try the sequence continuation first, then the key on its own, so an
    // abandoned "g" followed by "j" still moves down.
    for (const sequence of candidates) {
        const shortcut = findActive(sequence);
        if (shortcut) {
            event.preventDefault();
            if (!event.repeat || shortcut.repeat) shortcut.run();
            return;
        }
        if (isPrefix(sequence)) {
            event.preventDefault();
            pending = sequence.split(" ");
            pendingTimer = setTimeout(resetPending, SEQUENCE_TIMEOUT_MS);
            return;
        }
    }
}
