/**
 * GitHub avatar URL, falling back to the login-based endpoint when the API
 * didn't provide one (e.g. a row not yet enriched).
 */
export function avatarUrl(
    login: string,
    apiUrl: string | null,
    size = 40,
): string {
    return apiUrl ?? `https://github.com/${login}.png?size=${size}`;
}
