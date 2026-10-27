use futures_util::future::join_all;
use libarc::ArcDaemonProxy;

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct DescBlock {
    pub text: String,
    pub is_list_item: bool,
    pub is_heading: bool,
    pub is_bold: bool,
    pub is_image: bool,
    pub image_url: String,
    pub is_app: bool,
    pub app_id: String,
    pub app_name: String,
    pub app_summary: String,
    pub app_icon_url: String,
}

fn text_block(text: String, is_list_item: bool, is_heading: bool, is_bold: bool) -> DescBlock {
    DescBlock {
        text,
        is_list_item,
        is_heading,
        is_bold,
        ..Default::default()
    }
}

fn split_bold(text: String, is_list_item: bool, is_heading: bool) -> Vec<DescBlock> {
    if is_heading || !text.contains("**") {
        return vec![text_block(text, is_list_item, is_heading, false)];
    }
    let mut result = Vec::new();
    let mut remaining = text.as_str();
    while !remaining.is_empty() {
        match remaining.find("**") {
            None => {
                result.push(text_block(
                    remaining.to_string(),
                    is_list_item,
                    is_heading,
                    false,
                ));
                break;
            }
            Some(start) => {
                if start > 0 {
                    result.push(text_block(
                        remaining[..start].to_string(),
                        is_list_item,
                        is_heading,
                        false,
                    ));
                }
                remaining = &remaining[start + 2..];
                match remaining.find("**") {
                    None => {
                        result.push(text_block(
                            remaining.to_string(),
                            is_list_item,
                            is_heading,
                            false,
                        ));
                        break;
                    }
                    Some(end) => {
                        let bold = &remaining[..end];
                        if !bold.is_empty() {
                            result.push(text_block(
                                bold.to_string(),
                                is_list_item,
                                is_heading,
                                true,
                            ));
                        }
                        remaining = &remaining[end + 2..];
                    }
                }
            }
        }
    }
    result
}

fn parse_heading(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with("===") && s.ends_with("===") && s.len() > 6 {
        Some(s.trim_matches('=').trim().to_string())
    } else {
        None
    }
}

#[derive(Clone, Copy)]
enum Flush {
    Plain,
    ListItem,
    Paragraph,
    Heading,
}

fn flush(current: &mut String, blocks: &mut Vec<DescBlock>, how: Flush) {
    let text = current.trim().to_string();
    current.clear();
    if text.is_empty() {
        return;
    }
    match how {
        Flush::Plain => blocks.extend(split_bold(text, false, false)),
        Flush::ListItem => blocks.extend(split_bold(text, true, false)),
        Flush::Paragraph => match parse_heading(&text) {
            Some(heading) => blocks.push(text_block(heading, false, true, false)),
            None => blocks.extend(split_bold(text, false, false)),
        },
        Flush::Heading => blocks.push(text_block(text, false, true, false)),
    }
}

fn walk(node: roxmltree::Node, current: &mut String, blocks: &mut Vec<DescBlock>) {
    for child in node.children() {
        if let Some(text) = child.text().filter(|_| child.is_text()) {
            current.push_str(text);
            continue;
        }
        if !child.is_element() {
            continue;
        }
        match child.tag_name().name().to_ascii_lowercase().as_str() {
            "app" => {
                flush(current, blocks, Flush::Plain);
                if let Some(id) = child
                    .attribute("id")
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                {
                    blocks.push(DescBlock {
                        is_app: true,
                        app_id: id.to_string(),
                        ..Default::default()
                    });
                }
            }
            "img" => {
                flush(current, blocks, Flush::Plain);
                if let Some(src) = child.attribute("src").filter(|src| !src.is_empty()) {
                    blocks.push(DescBlock {
                        text: child.attribute("alt").unwrap_or_default().to_string(),
                        is_image: true,
                        image_url: libarc::media::proxied(src),
                        ..Default::default()
                    });
                }
            }
            "li" => {
                flush(current, blocks, Flush::Plain);
                walk(child, current, blocks);
                flush(current, blocks, Flush::ListItem);
            }
            "p" | "figcaption" => {
                walk(child, current, blocks);
                flush(current, blocks, Flush::Paragraph);
            }
            "h1" | "h2" | "h3" => {
                walk(child, current, blocks);
                flush(current, blocks, Flush::Heading);
            }
            "figure" => {
                walk(child, current, blocks);
                flush(current, blocks, Flush::Plain);
            }
            _ => walk(child, current, blocks),
        }
    }
}

// story markup is a piece of the forge frontpage so it gets a root element and is read with the same xml parser
pub fn html_to_blocks(html: &str) -> Vec<DescBlock> {
    let wrapped = format!("<story>{html}</story>");
    let doc = match roxmltree::Document::parse(&wrapped) {
        Ok(doc) => doc,
        Err(e) => {
            tracing::warn!("story text is not valid xml: {e}");
            return Vec::new();
        }
    };
    let mut blocks = Vec::new();
    let mut current = String::new();
    walk(doc.root_element(), &mut current, &mut blocks);
    flush(&mut current, &mut blocks, Flush::Plain);
    blocks
}

pub fn blocks_worth_caching(blocks: &[DescBlock]) -> bool {
    let mut apps = blocks
        .iter()
        .filter(|b| b.is_app && !b.app_id.is_empty())
        .peekable();
    if apps.peek().is_none() {
        return true;
    }
    apps.any(|b| !b.app_name.is_empty())
}

pub async fn resolve_app_blocks(blocks: &mut [DescBlock], proxy: Option<&ArcDaemonProxy<'static>>) {
    let ids: Vec<String> = blocks
        .iter()
        .filter(|b| b.is_app && !b.app_id.is_empty())
        .map(|b| b.app_id.clone())
        .collect();
    if ids.is_empty() {
        return;
    }

    let futs = ids.iter().map(|id| {
        let id = id.clone();
        let proxy = proxy.cloned();
        async move {
            let pkg = match proxy {
                Some(p) => {
                    tokio::time::timeout(std::time::Duration::from_secs(15), p.app_info(&id))
                        .await
                        .ok()
                        .and_then(|r| r.ok())
                        .flatten()
                }
                None => None,
            };
            (id, pkg)
        }
    });

    let resolved: std::collections::HashMap<String, libarc::Package> = join_all(futs)
        .await
        .into_iter()
        .filter_map(|(id, pkg)| pkg.map(|p| (id, p)))
        .collect();

    let needs_pwa = ids
        .iter()
        .any(|id| !resolved.contains_key(id) && id.starts_with("pwa:"));
    let pwas: std::collections::HashMap<String, crate::services::forge::PwaApp> = if needs_pwa {
        crate::services::forge::fetch_pwas(&crate::services::forge::user_lang())
            .await
            .into_iter()
            .map(|p| (p.appid.clone(), p))
            .collect()
    } else {
        Default::default()
    };

    for block in blocks.iter_mut() {
        if !block.is_app {
            continue;
        }
        if let Some(pkg) = resolved.get(&block.app_id) {
            block.app_name = pkg.name.clone();
            block.app_summary = pkg.description.clone();
            block.app_icon_url = crate::services::icons::resolve(&pkg.id, pkg.icon_url.as_deref());
        } else if let Some(pwa) = pwas.get(&block.app_id) {
            block.app_name = pwa.name.clone();
            block.app_summary = pwa.summary.clone();
            block.app_icon_url =
                crate::services::icons::resolve(&pwa.appid, pwa.icon_url.as_deref());
        }
    }
}
