import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/svelte";
import { afterEach } from "vitest";

afterEach(() => {
    cleanup();
});

// jsdom does not implement pointer capture; stub it so Bits UI components work in tests.
if (!Element.prototype.hasPointerCapture) {
    Element.prototype.hasPointerCapture = () => false;
    Element.prototype.setPointerCapture = () => {};
    Element.prototype.releasePointerCapture = () => {};
}

// jsdom does not implement scrollIntoView; stub it so Bits UI item highlighting works in tests.
if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = () => {};
}

// jsdom does not implement ResizeObserver; stub it so Bits UI floating content renders in tests.
if (typeof ResizeObserver === "undefined") {
    (globalThis as unknown as Record<string, unknown>).ResizeObserver = class {
        observe() {}
        unobserve() {}
        disconnect() {}
    };
}

// jsdom does not implement IntersectionObserver; stub it so components render in tests.
if (typeof IntersectionObserver === "undefined") {
    (globalThis as unknown as Record<string, unknown>).IntersectionObserver =
        class {
            observe() {}
            unobserve() {}
            disconnect() {}
        };
}
