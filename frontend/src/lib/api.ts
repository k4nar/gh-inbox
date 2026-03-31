// Read the per-session token injected by the Rust server into index.html.
// In dev mode (Vite serves HTML), the meta tag won't exist and this is null,
// which is fine since the session-token check is disabled in debug builds.
const SESSION_TOKEN =
    document.querySelector<HTMLMetaElement>('meta[name="x-session-token"]')
        ?.content ?? null;

export async function apiFetch<T>(url: string, init?: RequestInit): Promise<T> {
    const headers: Record<string, string> = SESSION_TOKEN
        ? { "x-session-token": SESSION_TOKEN }
        : {};
    const mergedInit: RequestInit = {
        ...init,
        headers: { ...headers, ...(init?.headers as Record<string, string>) },
    };
    const res = await fetch(url, mergedInit);
    if (!res.ok) {
        throw new Error(`${res.status} ${res.statusText}`);
    }
    if (res.status === 202 || res.status === 204) {
        return undefined as T;
    }
    return res.json() as Promise<T>;
}
