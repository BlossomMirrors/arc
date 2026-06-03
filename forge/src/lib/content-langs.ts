export const CONTENT_LANGS = [
	{ code: 'de', label: 'Deutsch' }
] as const satisfies readonly { code: string; label: string }[];

export type ContentLangCode = (typeof CONTENT_LANGS)[number]['code'];
