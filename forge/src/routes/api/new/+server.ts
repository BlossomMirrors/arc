import { db } from '$lib/server/db';
import { appsWithMetrics, toPublicWithMetrics } from '$lib/server/metrics';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	const limit = Math.min(Number(url.searchParams.get('limit') ?? 20), 100);
	const apps = await db.pwaApp.findMany({ orderBy: { createdAt: 'desc' }, take: limit });
	const enriched = await appsWithMetrics(apps);
	return Response.json(enriched.map((a) => toPublicWithMetrics(a)));
};
