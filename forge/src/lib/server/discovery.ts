import { db } from './db';
import * as flathub from './flathub';
import { getCustomRepoIds } from './custom-repo';

function mergeDedup(primary: string[], secondary: string[], limit: number): string[] {
	const seen = new Set(primary);
	for (const id of secondary) {
		if (!seen.has(id)) { seen.add(id); primary.push(id); }
	}
	return primary.slice(0, limit);
}

async function getOurIds(since?: Date): Promise<string[]> {
	const [pwaApps, repoIds, counts] = await Promise.all([
		db.pwaApp.findMany({ select: { appid: true } }),
		getCustomRepoIds(),
		db.appInstall.groupBy({
			by: ['appid'],
			where: since ? { createdAt: { gte: since } } : undefined,
			_count: { id: true },
			orderBy: { _count: { id: 'desc' } }
		})
	]);

	const allIds = new Set([...pwaApps.map((a) => a.appid), ...repoIds]);
	const tracked = counts.map((r) => r.appid).filter((id) => allIds.has(id));
	const untracked = [...allIds].filter((id) => !tracked.includes(id));
	return [...tracked, ...untracked];
}

export async function getTopIds(limit: number): Promise<string[]> {
	const [flathubIds, ourIds] = await Promise.all([
		flathub.getPopular(limit),
		getOurIds()
	]);
	return mergeDedup(flathubIds, ourIds, limit);
}

export async function getTrendingIds(limit: number): Promise<string[]> {
	const since = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000);
	const [flathubIds, ourIds] = await Promise.all([
		flathub.getTrending(limit),
		getOurIds(since)
	]);
	return mergeDedup(flathubIds, ourIds, limit);
}

export async function getNewIds(limit: number): Promise<string[]> {
	const [flathubIds, pwaApps, repoIds] = await Promise.all([
		flathub.getRecentlyAdded(limit),
		db.pwaApp.findMany({ select: { appid: true }, orderBy: { createdAt: 'desc' } }),
		getCustomRepoIds()
	]);
	const ourIds = [...pwaApps.map((a) => a.appid), ...repoIds];
	return mergeDedup(flathubIds, ourIds, limit);
}
