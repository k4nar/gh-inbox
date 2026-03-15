import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/svelte";
import { afterEach } from "vitest";

afterEach(() => {
    cleanup();
});

// jsdom does not implement IntersectionObserver; stub it so components render in tests.
if (typeof IntersectionObserver === "undefined") {
    (globalThis as unknown as Record<string, unknown>).IntersectionObserver =
        class {
            observe() {}
            unobserve() {}
            disconnect() {}
        };
}
