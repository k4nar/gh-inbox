import { afterEach, describe, expect, it, vi } from "vitest";
import {
    formatKeys,
    listShortcuts,
    registerShortcuts,
    SEQUENCE_TIMEOUT_MS,
} from "./shortcuts.ts";

function press(key: string, init: KeyboardEventInit = {}, target?: Element) {
    const event = new KeyboardEvent("keydown", {
        key,
        bubbles: true,
        cancelable: true,
        ...init,
    });
    (target ?? document.body).dispatchEvent(event);
    return event;
}

const cleanups: (() => void)[] = [];
function register(...args: Parameters<typeof registerShortcuts>) {
    const unregister = registerShortcuts(...args);
    cleanups.push(unregister);
    return unregister;
}

afterEach(() => {
    for (const c of cleanups.splice(0)) c();
    document.body.innerHTML = "";
    vi.useRealTimers();
});

describe("shortcuts", () => {
    it("runs a single-key shortcut and prevents the default action", () => {
        const run = vi.fn();
        register([{ keys: ["j"], description: "Next", group: "List", run }]);

        const event = press("j");

        expect(run).toHaveBeenCalledOnce();
        expect(event.defaultPrevented).toBe(true);
    });

    it("supports alternative bindings", () => {
        const run = vi.fn();
        register([
            { keys: ["o", "Enter"], description: "Open", group: "List", run },
        ]);

        press("o");
        press("Enter");

        expect(run).toHaveBeenCalledTimes(2);
    });

    it("runs a two-key sequence", () => {
        const run = vi.fn();
        register([{ keys: ["g i"], description: "Inbox", group: "Go", run }]);

        press("g");
        expect(run).not.toHaveBeenCalled();
        press("i");
        expect(run).toHaveBeenCalledOnce();
    });

    it("drops a sequence prefix after the timeout", () => {
        vi.useFakeTimers();
        const run = vi.fn();
        register([{ keys: ["g i"], description: "Inbox", group: "Go", run }]);

        press("g");
        vi.advanceTimersByTime(SEQUENCE_TIMEOUT_MS + 1);
        press("i");

        expect(run).not.toHaveBeenCalled();
    });

    it("falls back to the single key when a sequence is abandoned", () => {
        const inbox = vi.fn();
        const next = vi.fn();
        register([
            { keys: ["g i"], description: "Inbox", group: "Go", run: inbox },
            { keys: ["j"], description: "Next", group: "List", run: next },
        ]);

        press("g");
        press("j");

        expect(inbox).not.toHaveBeenCalled();
        expect(next).toHaveBeenCalledOnce();
    });

    it("does not break a sequence on a bare Shift press", () => {
        const run = vi.fn();
        register([{ keys: ["g I"], description: "X", group: "Go", run }]);

        press("g");
        press("Shift", { shiftKey: true });
        press("I", { shiftKey: true });

        expect(run).toHaveBeenCalledOnce();
    });

    it("ignores keys typed into editable fields", () => {
        const run = vi.fn();
        register([{ keys: ["j"], description: "Next", group: "List", run }]);
        const input = document.createElement("input");
        document.body.append(input);

        press("j", {}, input);

        expect(run).not.toHaveBeenCalled();
    });

    it("ignores keys pressed with Cmd, Ctrl or Alt", () => {
        const run = vi.fn();
        register([{ keys: ["r"], description: "Sync", group: "App", run }]);

        press("r", { metaKey: true });
        press("r", { ctrlKey: true });
        press("r", { altKey: true });

        expect(run).not.toHaveBeenCalled();
    });

    it("ignores keys while focus is inside a dialog", () => {
        const run = vi.fn();
        register([{ keys: ["?"], description: "Help", group: "App", run }]);
        const dialog = document.createElement("div");
        dialog.setAttribute("role", "dialog");
        const button = document.createElement("button");
        dialog.append(button);
        document.body.append(dialog);

        press("?", {}, button);

        expect(run).not.toHaveBeenCalled();
    });

    it("ignores keys while a popup trigger is expanded", () => {
        const run = vi.fn();
        register([{ keys: ["j"], description: "Next", group: "List", run }]);
        const trigger = document.createElement("button");
        trigger.setAttribute("aria-haspopup", "listbox");
        trigger.setAttribute("aria-expanded", "true");
        document.body.append(trigger);

        press("j", {}, trigger);
        press("j");
        expect(run).not.toHaveBeenCalled();

        trigger.setAttribute("aria-expanded", "false");
        press("j");
        expect(run).toHaveBeenCalledOnce();
    });

    it("leaves Enter to a focused button", () => {
        const run = vi.fn();
        register([
            { keys: ["Enter"], description: "Open", group: "List", run },
        ]);
        const button = document.createElement("button");
        document.body.append(button);

        press("Enter", {}, button);
        expect(run).not.toHaveBeenCalled();

        press("Enter");
        expect(run).toHaveBeenCalledOnce();
    });

    it("skips a shortcut whose condition is false", () => {
        const run = vi.fn();
        register([
            {
                keys: ["Escape"],
                description: "Close",
                group: "Detail",
                when: () => false,
                run,
            },
        ]);

        const event = press("Escape");

        expect(run).not.toHaveBeenCalled();
        expect(event.defaultPrevented).toBe(false);
    });

    it("lets the most recently registered shortcut win", () => {
        const first = vi.fn();
        const second = vi.fn();
        register([{ keys: ["x"], description: "A", group: "G", run: first }]);
        register([{ keys: ["x"], description: "B", group: "G", run: second }]);

        press("x");

        expect(first).not.toHaveBeenCalled();
        expect(second).toHaveBeenCalledOnce();
    });

    it("stops handling a shortcut once unregistered", () => {
        const run = vi.fn();
        const unregister = register([
            { keys: ["j"], description: "Next", group: "List", run },
        ]);

        unregister();
        press("j");

        expect(run).not.toHaveBeenCalled();
        expect(listShortcuts()).toHaveLength(0);
    });

    it("ignores held-key repeats unless the shortcut opts in", () => {
        const archive = vi.fn();
        const next = vi.fn();
        register([
            { keys: ["e"], description: "Archive", group: "A", run: archive },
            {
                keys: ["j"],
                description: "Next",
                group: "N",
                repeat: true,
                run: next,
            },
        ]);

        press("e");
        const repeated = press("e", { repeat: true });
        press("j");
        press("j", { repeat: true });

        expect(archive).toHaveBeenCalledOnce();
        // The repeat is still swallowed so it does not reach the page.
        expect(repeated.defaultPrevented).toBe(true);
        expect(next).toHaveBeenCalledTimes(2);
    });

    it("ignores Caps Lock for letters", () => {
        const lower = vi.fn();
        const upper = vi.fn();
        register([
            { keys: ["o"], description: "Open", group: "N", run: lower },
            { keys: ["O"], description: "GitHub", group: "N", run: upper },
        ]);

        press("O", { modifierCapsLock: true });
        expect(lower).toHaveBeenCalledOnce();
        expect(upper).not.toHaveBeenCalled();

        press("o", { modifierCapsLock: true, shiftKey: true });
        expect(upper).toHaveBeenCalledOnce();
    });

    it("formats bindings for display", () => {
        expect(formatKeys("I")).toEqual(["Shift+I"]);
        expect(formatKeys("g i")).toEqual(["g", "i"]);
        expect(formatKeys("Escape")).toEqual(["Esc"]);
        expect(formatKeys("?")).toEqual(["?"]);
        expect(formatKeys("ArrowDown")).toEqual(["↓"]);
    });
});
