import { auth } from '$lib/auth';
import { AUTHENTIK_URL } from '$env/static/private';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ request }) => {
	const signOutResponse = await auth.api.signOut({
		headers: request.headers,
		asResponse: true
	});

	const headers = new Headers({
		Location: `${AUTHENTIK_URL}/application/o/arc-forge/end-session/`
	});
	for (const cookie of signOutResponse.headers.getSetCookie?.() ?? []) {
		headers.append('set-cookie', cookie);
	}
	return new Response(null, { status: 302, headers });
};
