/** @type {{ id: number, message: string }[]} */
let toasts = $state([]);
let nextId = 0;

const DURATION_MS = 5000;

export function getToasts() {
	return toasts;
}

export function showError(message) {
	const id = nextId++;
	toasts.push({ id, message });
	setTimeout(() => {
		toasts = toasts.filter((t) => t.id !== id);
	}, DURATION_MS);
}
