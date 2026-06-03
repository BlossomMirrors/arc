import { db } from './db';
import { getFlathubStats } from './flathub';

export type AppWithMetrics = Awaited<ReturnType<typeof db.pwaApp.findMany>>[number] & {
	installs: number;
	flathub_installs: number;
};

export async function appsWithMetrics(
	apps: Awaited<ReturnType<typeof db.pwaApp.findMany>>
): Promise<AppWithMetrics[]> {
	const flathub = await getFlathubStats();

	const appids = apps.map((a) => a.appid);
	const counts = await db.appInstall.groupBy({
		by: ['appid'],
		where: { appid: { in: appids } },
		_count: { id: true }
	});
	const ourInstalls = new Map(counts.map((r) => [r.appid, r._count.id]));

	return apps.map((app) => ({
		...app,
		installs: ourInstalls.get(app.appid) ?? 0,
		flathub_installs: flathub.get(app.appid) ?? 0
	}));
}

export async function appsWithTrendingMetrics(
	apps: Awaited<ReturnType<typeof db.pwaApp.findMany>>
): Promise<AppWithMetrics[]> {
	const since = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000);
	const appids = apps.map((a) => a.appid);

	const counts = await db.appInstall.groupBy({
		by: ['appid'],
		where: { appid: { in: appids }, createdAt: { gte: since } },
		_count: { id: true }
	});
	const recentInstalls = new Map(counts.map((r) => [r.appid, r._count.id]));

	return apps.map((app) => ({
		...app,
		installs: recentInstalls.get(app.appid) ?? 0,
		flathub_installs: 0
	}));
}

export function toPublicWithMetrics(app: AppWithMetrics, rank?: number) {
	return {
		id: app.appid,
		appid: app.appid,
		name: app.name,
		summary: app.summary,
		description: app.description,
		icon_url: app.iconUrl,
		screenshots: app.screenshots,
		homepage_url: app.homepageUrl,
		content_rating: app.contentRating,
		developer_name: app.developerName,
		verified: true,
		url: app.url,
		color: app.color,
		css: app.css,
		js: app.js,
		useragent: app.useragent,
		widevine: app.widevine,
		tray: app.tray,
		installs: app.installs + app.flathub_installs,
		own_installs: app.installs,
		flathub_installs: app.flathub_installs,
		...(rank !== undefined ? { rank } : {})
	};
}
