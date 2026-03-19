<script lang="ts">
import type { CheckRun } from "./types.ts";

let { checkRuns, size = 16 }: { checkRuns: CheckRun[]; size?: number } =
    $props();

function isPassing(cr: CheckRun): boolean {
    return (
        cr.status === "completed" &&
        (cr.conclusion === "success" ||
            cr.conclusion === "skipped" ||
            cr.conclusion === "neutral")
    );
}

let label = $derived.by(() => {
    if (checkRuns.length === 0) return "";
    const failing = checkRuns.filter(
        (cr) =>
            cr.status === "completed" &&
            cr.conclusion &&
            !["success", "skipped", "neutral"].includes(cr.conclusion),
    ).length;
    const pending = checkRuns.filter((cr) => cr.status !== "completed").length;
    if (failing > 0) return `${failing} failing`;
    if (pending > 0) return `${pending} running`;
    return "CI passing";
});

let segments = $derived.by(() => {
    if (checkRuns.length === 0) return [];
    const total = checkRuns.length;
    const r = 6;
    const circ = 2 * Math.PI * r;
    const counts = {
        failing: checkRuns.filter(
            (cr) =>
                cr.status === "completed" &&
                cr.conclusion &&
                !["success", "skipped", "neutral"].includes(cr.conclusion),
        ).length,
        pending: checkRuns.filter((cr) => cr.status !== "completed").length,
        success: checkRuns.filter(
            (cr) => cr.status === "completed" && cr.conclusion === "success",
        ).length,
        skipped: checkRuns.filter(
            (cr) =>
                cr.status === "completed" &&
                (cr.conclusion === "skipped" || cr.conclusion === "neutral"),
        ).length,
    };
    const colors: Record<string, string> = {
        failing: "var(--danger-fg, #cf222e)",
        pending: "var(--attention-fg, #9a6700)",
        success: "var(--success-fg, #1a7f37)",
        skipped: "var(--border-default, #444c56)",
    };
    let offset = 0;
    return (["failing", "pending", "success", "skipped"] as const)
        .filter((k) => counts[k] > 0)
        .map((k) => {
            const length = (counts[k] / total) * circ;
            const seg = {
                color: colors[k],
                dasharray: `${length} ${circ}`,
                rotate: (offset / circ) * 360 - 90,
            };
            offset += length;
            return seg;
        });
});
</script>

<svg
    role="img"
    aria-label={label}
    width={size}
    height={size}
    viewBox="0 0 16 16"
>
    <circle
        cx="8"
        cy="8"
        r="6"
        fill="none"
        stroke="var(--border-default)"
        stroke-width="2.5"
    />
    {#each segments as seg}
        <circle
            cx="8"
            cy="8"
            r="6"
            fill="none"
            stroke={seg.color}
            stroke-width="2.5"
            stroke-dasharray={seg.dasharray}
            transform="rotate({seg.rotate} 8 8)"
        />
    {/each}
</svg>
