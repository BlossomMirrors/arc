export type LangString = { lang: string; text: string };

export type CarouselApp = { type: 'app'; id: string };
export type CarouselStory = { type: 'story'; banner: string; titles: LangString[]; body: string };
export type CarouselItem = CarouselApp | CarouselStory;

export type Section =
	| { type: 'h1'; text: string }
	| { type: 'h2'; text: string }
	| { type: 'h3'; text: string }
	| { type: 'p'; text: string }
	| { type: 'ul'; items: string[] }
	| { type: 'br' }
	| { type: 'carousel'; breakpoint: number; items: CarouselItem[] }
	| { type: 'top' }
	| { type: 'new' }
	| { type: 'trending' }
	| { type: 'categories' }
	| { type: 'category'; value: string }
	| { type: 'custom'; titles: LangString[]; apps: string[] }
	| { type: 'charts'; cards: boolean };

export const HTML_TYPES = ['h1', 'h2', 'h3', 'p', 'ul', 'br'] as const;
export const APP_TYPES = [
	'carousel',
	'top',
	'new',
	'trending',
	'categories',
	'category',
	'custom',
	'charts'
] as const;

function esc(s: string) {
	return s
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;');
}

function langStrings(items: LangString[], indent: string) {
	return items
		.map((t) => `${indent}<title lang="${esc(t.lang)}">${esc(t.text)}</title>`)
		.join('\n');
}

function carouselItemToXml(item: CarouselItem): string {
	if (item.type === 'app') return `    <app id="${esc(item.id)}" />`;
	const titles = langStrings(item.titles, '        ');
	return `    <story banner="${esc(item.banner)}">\n${titles}\n        <body>\n            ${item.body}\n        </body>\n    </story>`;
}

function sectionToXml(s: Section): string {
	switch (s.type) {
		case 'h1':
			return `<h1>${esc(s.text)}</h1>`;
		case 'h2':
			return `<h2>${esc(s.text)}</h2>`;
		case 'h3':
			return `<h3>${esc(s.text)}</h3>`;
		case 'p':
			return `<p>${esc(s.text)}</p>`;
		case 'ul':
			return `<ul>\n${s.items.map((i) => `  <li>${esc(i)}</li>`).join('\n')}\n</ul>`;
		case 'br':
			return `<br />`;
		case 'carousel':
			return `<carousel breakpoint="${s.breakpoint}">\n${s.items.map(carouselItemToXml).join('\n')}\n</carousel>`;
		case 'top':
			return '<top />';
		case 'new':
			return '<new />';
		case 'trending':
			return '<trending />';
		case 'categories':
			return '<categories />';
		case 'category':
			return `<category>${esc(s.value)}</category>`;
		case 'custom': {
			const titles = langStrings(s.titles, '    ');
			const apps = s.apps.map((id) => `    <app id="${esc(id)}" />`).join('\n');
			return `<custom>\n${titles}\n${apps}\n</custom>`;
		}
		case 'charts':
			return `<charts cards="${s.cards}" />`;
	}
}

export function sectionsToXml(sections: Section[]): string {
	return `<?xml version="1.0" encoding="UTF-8" ?>\n${sections.map(sectionToXml).join('\n')}`;
}

export function newSection(type: Section['type']): Section {
	switch (type) {
		case 'h1':
		case 'h2':
		case 'h3':
		case 'p':
			return { type, text: '' };
		case 'ul':
			return { type, items: [''] };
		case 'br':
			return { type };
		case 'carousel':
			return { type, breakpoint: 5, items: [] };
		case 'category':
			return { type, value: '' };
		case 'custom':
			return { type, titles: [{ lang: 'en', text: '' }], apps: [] };
		case 'charts':
			return { type, cards: false };
		default:
			return { type } as Section;
	}
}
