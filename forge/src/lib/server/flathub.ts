const BASE = 'https://flathub.org/api/v2';
const COLLECTION_TTL_MS = 60 * 60 * 1000; // 1 hour

type CollectionEntry = { app_id: string };
type CollectionResponse = { hits: CollectionEntry[] };

interface CollectionCache {
	ids: string[];
	at: number;
}

const cache: Record<string, CollectionCache> = {};

async function fetchCollection(path: string, limit: number): Promise<string[]> {
	const hit = cache[path];
	if (hit && Date.now() - hit.at < COLLECTION_TTL_MS) return hit.ids.slice(0, limit);

	try {
		const res = await fetch(`${BASE}${path}?locale=en`);
		if (!res.ok) throw new Error(`Flathub ${path}: HTTP ${res.status}`);
		const data: CollectionResponse = await res.json();
		const ids = (data.hits ?? []).map((a) => a.app_id).filter(Boolean);
		cache[path] = { ids, at: Date.now() };
		return ids.slice(0, limit);
	} catch {
		return hit?.ids.slice(0, limit) ?? [];
	}
}

export const getPopular = (limit: number) => fetchCollection('/collection/popular', limit);
export const getTrending = (limit: number) => fetchCollection('/collection/trending', limit);
export const getRecentlyAdded = (limit: number) => fetchCollection('/collection/recently-added', limit);
