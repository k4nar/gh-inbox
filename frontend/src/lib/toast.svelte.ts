interface Toast {
    id: number;
    message: string;
}

let toasts: Toast[] = $state([]);
let nextId = 0;

const DURATION_MS = 5000;

export function getToasts(): Toast[] {
    return toasts;
}

export function showError(message: string): void {
    const id = nextId++;
    toasts.push({ id, message });
    setTimeout(() => {
        toasts = toasts.filter((t) => t.id !== id);
    }, DURATION_MS);
}
