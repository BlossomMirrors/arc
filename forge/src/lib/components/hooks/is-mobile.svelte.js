const MOBILE_BREAKPOINT = 768;

export class IsMobile {
	current = $state(false);

	constructor() {
		if (typeof window === 'undefined') return;
		const mql = window.matchMedia(`(max-width: ${MOBILE_BREAKPOINT - 1}px)`);
		this.current = mql.matches;
		const handler = (/** @type {MediaQueryListEvent} */ e) => { this.current = e.matches; };
		mql.addEventListener('change', handler);
	}
}
