const REPO_URL = 'https://repo.blossomos.org/flatpak/refs/heads/app/';
const TTL_MS = 60 * 60 * 1000; // 1 hour

// Flatpak app IDs are reverse-domain names: two or more dot-separated segments
const APP_ID_RE = /href="([a-zA-Z][a-zA-Z0-9_-]*(?:\.[a-zA-Z][a-zA-Z0-9_.-]+)+)\/?"/g;

let cached: string[] = [];
let cachedAt = 0;

export async function getCustomRepoIds(): Promise<string[]> {
	if (cached.length && Date.now() - cachedAt < TTL_MS) return cached;

	try {
		const res = await fetch(REPO_URL);
		if (!res.ok) throw new Error(`Custom repo: HTTP ${res.status}`);
		const html = await res.text();
		const ids: string[] = [];
		for (const m of html.matchAll(APP_ID_RE)) ids.push(m[1]);
		cached = [...new Set(ids)];
		cachedAt = Date.now();
	} catch {
		// keep stale cache on error
	}

	return cached;
}
