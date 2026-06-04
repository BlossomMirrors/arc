import { db } from './db';
import { CONTENT_LANGS } from '$lib/content-langs';
import { CDN_BASE, uploadText } from './bunny';

export async function fetchCodeField(value: string): Promise<string> {
	if (!value || !value.startsWith(CDN_BASE)) return value;
	const res = await fetch(value);
	if (!res.ok) return value;
	return res.text();
}

export async function uploadCodeField(value: string, ext: 'css' | 'js'): Promise<string> {
	if (!value) return value;
	return uploadText(value, ext);
}

export function parseTranslations(data: FormData) {
	return CONTENT_LANGS.map(({ code }) => ({
		lang: code,
		name: ((data.get(`trans_${code}_name`) as string) ?? '').trim() || null,
		summary: ((data.get(`trans_${code}_summary`) as string) ?? '').trim() || null,
		description: ((data.get(`trans_${code}_description`) as string) ?? '').trim() || null
	}));
}

export async function saveTranslations(appId: string, data: FormData) {
	const translations = parseTranslations(data);
	for (const t of translations) {
		if (t.name || t.summary || t.description) {
			await db.pwaTranslation.upsert({
				where: { appId_lang: { appId, lang: t.lang } },
				update: { name: t.name, summary: t.summary, description: t.description },
				create: { appId, lang: t.lang, name: t.name, summary: t.summary, description: t.description }
			});
		} else {
			await db.pwaTranslation.deleteMany({ where: { appId, lang: t.lang } });
		}
	}
}
