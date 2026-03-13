export async function apiFetch<T>(url: string, init?: RequestInit): Promise<T> {
    const res = await (init ? fetch(url, init) : fetch(url));
    if (!res.ok) {
        throw new Error(`${res.status} ${res.statusText}`);
    }
    if (res.status === 204) {
        return undefined as T;
    }
    return res.json() as Promise<T>;
}
