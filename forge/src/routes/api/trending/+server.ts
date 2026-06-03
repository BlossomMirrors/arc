import { getTrendingIds } from '$lib/server/discovery';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	const limit = Math.min(Number(url.searchParams.get('limit') ?? 20), 100);
	return Response.json(await getTrendingIds(limit));
};
