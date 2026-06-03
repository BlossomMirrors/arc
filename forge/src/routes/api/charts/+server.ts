import { db } from '$lib/server/db';
import { appsWithMetrics, toPublicWithMetrics } from '$lib/server/metrics';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	const limit = Math.min(Number(url.searchParams.get('limit') ?? 20), 100);
	const apps = await db.pwaApp.findMany();
	const enriched = await appsWithMetrics(apps);
	enriched.sort((a, b) => (b.installs + b.flathub_installs) - (a.installs + a.flathub_installs));
	return Response.json(
		enriched.slice(0, limit).map((a, i) => toPublicWithMetrics(a, i + 1))
	);
};
