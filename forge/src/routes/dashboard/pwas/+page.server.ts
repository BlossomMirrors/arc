import { fail } from '@sveltejs/kit';
import { db } from '$lib/server/db';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async () => {
	const apps = await db.pwaApp.findMany({ orderBy: { createdAt: 'asc' } });
	return { apps };
};

export const actions: Actions = {
	delete: async ({ request }) => {
		const data = await request.formData();
		const id = data.get('id') as string;
		if (!id) return fail(400);
		await db.pwaApp.delete({ where: { id } });
	}
};
