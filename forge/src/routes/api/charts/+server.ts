import { getTopIds } from '$lib/server/discovery';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ url }) => {
	const limit = Math.min(Number(url.searchParams.get('limit') ?? 20), 100);
	const ids = await getTopIds(limit);
	return Response.json(ids.map((id, i) => ({ rank: i + 1, id })));
};
