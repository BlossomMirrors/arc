// Flathub stats API: https://flathub.org/api/v2/stats
// Response: { by_app: { "com.example.App": { installs_total: number } } }

const FLATHUB_STATS = 'https://flathub.org/api/v2/stats';
const TTL_MS = 6 * 60 * 60 * 1000; // 6 hours

let cache: Map<string, number> | null = null;
let cachedAt = 0;

async function refresh(): Promise<Map<string, number>> {
	const res = await fetch(FLATHUB_STATS);
	if (!res.ok) throw new Error(`Flathub stats: HTTP ${res.status}`);
	const data = await res.json();

	const map = new Map<string, number>();
	const byApp = data?.by_app ?? {};
	for (const [appid, stats] of Object.entries(byApp)) {
		const count = (stats as { installs_total?: number })?.installs_total ?? 0;
		map.set(appid, count);
	}
	return map;
}

export async function getFlathubStats(): Promise<Map<string, number>> {
	if (cache && Date.now() - cachedAt < TTL_MS) return cache;
	try {
		cache = await refresh();
		cachedAt = Date.now();
	} catch {
		cache ??= new Map();
	}
	return cache;
}

export async function getFlathubInstalls(appid: string): Promise<number> {
	const stats = await getFlathubStats();
	return stats.get(appid) ?? 0;
}
