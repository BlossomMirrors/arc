import type { PwaApp, PwaTranslation } from '$lib/generated/prisma/client';

export type AppWithTranslations = PwaApp & { translations: PwaTranslation[] };

export function applyLang<T extends AppWithTranslations>(app: T, lang: string): T {
	if (!lang || lang === 'en') return app;
	const t = app.translations.find((tr) => tr.lang === lang);
	if (!t) return app;
	return {
		...app,
		name: t.name || app.name,
		summary: t.summary || app.summary,
		description: t.description || app.description
	};
}

export function parseLang(url: URL): string {
	return (url.searchParams.get('lang') ?? 'en').slice(0, 10);
}
