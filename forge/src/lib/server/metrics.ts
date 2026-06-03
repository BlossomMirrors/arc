import { db } from './db';
import { getFlathubStats } from './flathub';
import { applyLang, type AppWithTranslations } from './pwa';

export type AppWithMetrics = AppWithTranslations & {
	installs: number;
	flathub_installs: number;
};

const INCLUDE_TRANSLATIONS = { translations: true } as const;

export async function fetchApps() {
	return db.pwaApp.findMany({ include: INCLUDE_TRANSLATIONS });
}

async function enrichWithInstalls(
	apps: AppWithTranslations[],
	since?: Date
): Promise<AppWithMetrics[]> {
	const appids = apps.map((a) => a.appid);
	const counts = await db.appInstall.groupBy({
		by: ['appid'],
		where: { appid: { in: appids }, ...(since ? { createdAt: { gte: since } } : {}) },
		_count: { id: true }
	});
	const ourInstalls = new Map(counts.map((r) => [r.appid, r._count.id]));
	return apps.map((app) => ({ ...app, installs: ourInstalls.get(app.appid) ?? 0, flathub_installs: 0 }));
}

export async function appsWithMetrics(apps: AppWithTranslations[]): Promise<AppWithMetrics[]> {
	const flathub = await getFlathubStats();
	const enriched = await enrichWithInstalls(apps);
	return enriched.map((app) => ({
		...app,
		flathub_installs: flathub.get(app.appid) ?? 0
	}));
}

export async function appsWithTrendingMetrics(apps: AppWithTranslations[]): Promise<AppWithMetrics[]> {
	const since = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000);
	return enrichWithInstalls(apps, since);
}

export function toPublicWithMetrics(app: AppWithMetrics, lang = 'en', rank?: number) {
	const a = applyLang(app, lang);
	return {
		id: a.appid,
		appid: a.appid,
		name: a.name,
		summary: a.summary,
		description: a.description,
		icon_url: a.iconUrl,
		screenshots: a.screenshots,
		homepage_url: a.homepageUrl,
		content_rating: a.contentRating,
		developer_name: a.developerName,
		verified: true,
		url: a.url,
		color: a.color,
		css: a.css,
		js: a.js,
		useragent: a.useragent,
		widevine: a.widevine,
		tray: a.tray,
		installs: app.installs + app.flathub_installs,
		own_installs: app.installs,
		flathub_installs: app.flathub_installs,
		...(rank !== undefined ? { rank } : {})
	};
}
