import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';

async function gravatar(email: string, size = 80): Promise<string> {
	const normalized = email.trim().toLowerCase();
	const encoded = new TextEncoder().encode(normalized);
	const hashBuffer = await crypto.subtle.digest('SHA-256', encoded);
	const hash = Array.from(new Uint8Array(hashBuffer))
		.map((b) => b.toString(16).padStart(2, '0'))
		.join('');
	return `https://gravatar.com/avatar/${hash}?s=${size}&d=mp`;
}

export const load: LayoutServerLoad = async ({ locals }) => {
	if (!locals.user) throw redirect(302, '/auth/login');
	const avatarUrl = locals.user.image ?? (await gravatar(locals.user.email));
	return { user: locals.user, avatarUrl };
};
