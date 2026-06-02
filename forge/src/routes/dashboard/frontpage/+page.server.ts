import { db } from '$lib/server/db';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async () => {
	const row = await db.frontPage.findUnique({ where: { id: 'singleton' } });
	return { sections: (row?.sections ?? []) as unknown[] };
};

export const actions: Actions = {
	save: async ({ request }) => {
		const data = await request.formData();
		const raw = data.get('sections') as string;
		const sections = JSON.parse(raw);
		await db.frontPage.upsert({
			where: { id: 'singleton' },
			update: { sections },
			create: { id: 'singleton', sections }
		});
	}
};
