use super::home::{Card, HeroItem, HomeSection};
use libarc::{ArcDaemonProxy, ImageKind, Package};

fn wanted(url: &String) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

async fn resolve(
    proxy: Option<&ArcDaemonProxy<'static>>,
    urls: Vec<&mut String>,
    kind: ImageKind,
    width: u32,
) {
    let Some(proxy) = proxy else { return };
    let mut urls: Vec<&mut String> = urls.into_iter().filter(|u| wanted(u)).collect();
    proxy.resolve_image_urls(&mut urls, kind, width).await;
}

fn card_urls(cards: &mut [Card]) -> impl Iterator<Item = &mut String> {
    cards.iter_mut().map(|c| &mut c.icon_url)
}

fn hero_urls(items: &mut [HeroItem]) -> Vec<&mut String> {
    let mut out = Vec::new();
    for item in items {
        out.push(&mut item.banner_url);
        out.push(&mut item.icon_url);
        out.extend(card_urls(&mut item.apps));
    }
    out
}

pub async fn resolve_sections(
    proxy: Option<&ArcDaemonProxy<'static>>,
    sections: &mut [HomeSection],
) {
    let mut urls = Vec::new();
    for section in sections {
        urls.extend(card_urls(&mut section.cards));
        urls.extend(hero_urls(&mut section.hero_items));
        urls.extend(hero_urls(&mut section.editorial_items));
    }
    resolve(proxy, urls, ImageKind::Image, 0).await;
}

pub async fn resolve_cards(proxy: Option<&ArcDaemonProxy<'static>>, cards: &mut [Card]) {
    resolve(proxy, card_urls(cards).collect(), ImageKind::Icon, 0).await;
}

pub async fn resolve_urls(
    proxy: Option<&ArcDaemonProxy<'static>>,
    urls: Vec<&mut String>,
    kind: ImageKind,
    width: u32,
) {
    resolve(proxy, urls, kind, width).await;
}

pub async fn resolve_packages(packages: &mut [Package]) {
    let proxy = crate::runtime::proxy().await;
    let urls = packages
        .iter_mut()
        .filter_map(|p| p.icon_url.as_mut())
        .collect();
    resolve(proxy.as_ref(), urls, ImageKind::Icon, 0).await;
}
