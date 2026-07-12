use futures_util::future::join_all;
use libarc::ArcDaemonProxy;

#[derive(serde::Serialize, Default, Clone)]
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

fn push_decoded(text: &str, out: &mut String) {
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find('&') {
            None => {
                out.push_str(rest);
                break;
            }
            Some(pos) => {
                out.push_str(&rest[..pos]);
                rest = &rest[pos..];
                if rest.starts_with("&amp;") {
                    out.push('&');
                    rest = &rest[5..];
                } else if rest.starts_with("&lt;") {
                    out.push('<');
                    rest = &rest[4..];
                } else if rest.starts_with("&gt;") {
                    out.push('>');
                    rest = &rest[4..];
                } else if rest.starts_with("&quot;") {
                    out.push('"');
                    rest = &rest[6..];
                } else if rest.starts_with("&apos;") {
                    out.push('\'');
                    rest = &rest[6..];
                } else {
                    out.push('&');
                    rest = &rest[1..];
                }
            }
        }
    }
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
                result.push(text_block(remaining.to_string(), is_list_item, is_heading, false));
                break;
            }
            Some(start) => {
                if start > 0 {
                    result.push(text_block(remaining[..start].to_string(), is_list_item, is_heading, false));
                }
                remaining = &remaining[start + 2..];
                match remaining.find("**") {
                    None => {
                        result.push(text_block(remaining.to_string(), is_list_item, is_heading, false));
                        break;
                    }
                    Some(end) => {
                        let bold = &remaining[..end];
                        if !bold.is_empty() {
                            result.push(text_block(bold.to_string(), is_list_item, is_heading, true));
                        }
                        remaining = &remaining[end + 2..];
                    }
                }
            }
        }
    }
    result
}

fn attr_from_tag(tag: &str, name: &str) -> String {
    let needle = format!("{}=\"", name);
    let lower = tag.to_ascii_lowercase();
    if let Some(pos) = lower.find(&needle) {
        let after = &tag[pos + needle.len()..];
        if let Some(end) = after.find('"') {
            return after[..end].to_string();
        }
    }
    String::new()
}

fn parse_heading(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with("===") && s.ends_with("===") && s.len() > 6 {
        Some(s.trim_matches('=').trim().to_string())
    } else {
        None
    }
}

pub fn html_to_blocks(html: &str) -> Vec<DescBlock> {
    let mut blocks: Vec<DescBlock> = Vec::new();
    let mut current = String::new();
    let mut rest = html;
    while !rest.is_empty() {
        match rest.find('<') {
            None => {
                push_decoded(rest, &mut current);
                break;
            }
            Some(tag_start) => {
                push_decoded(&rest[..tag_start], &mut current);
                rest = &rest[tag_start + 1..];
                let tag_end = match rest.find('>') {
                    Some(e) => e,
                    None => {
                        current.push('<');
                        continue;
                    }
                };
                let raw_tag = rest[..tag_end].trim();
                let closing = raw_tag.starts_with('/');
                let name = raw_tag
                    .trim_start_matches('/')
                    .split_ascii_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                match (closing, name.as_str()) {
                    (false, "li") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, false, false));
                        }
                        current.clear();
                    }
                    (true, "li") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, true, false));
                        }
                        current.clear();
                    }
                    (true, "p") | (true, "figcaption") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            if let Some(heading) = parse_heading(&t) {
                                blocks.push(text_block(heading, false, true, false));
                            } else {
                                blocks.extend(split_bold(t, false, false));
                            }
                        }
                        current.clear();
                    }
                    (true, "h1") | (true, "h2") | (true, "h3") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.push(text_block(t, false, true, false));
                        }
                        current.clear();
                    }
                    (false, "app") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, false, false));
                            current.clear();
                        }
                        let id = attr_from_tag(raw_tag, "id");
                        if !id.is_empty() {
                            blocks.push(DescBlock {
                                is_app: true,
                                app_id: id,
                                ..Default::default()
                            });
                        }
                    }
                    (_, "img") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, false, false));
                            current.clear();
                        }
                        let src = attr_from_tag(raw_tag, "src");
                        if !src.is_empty() {
                            blocks.push(DescBlock {
                                text: attr_from_tag(raw_tag, "alt"),
                                is_image: true,
                                image_url: src,
                                ..Default::default()
                            });
                        }
                    }
                    (true, "figure") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, false, false));
                            current.clear();
                        }
                    }
                    _ => {}
                }
                rest = &rest[tag_end + 1..];
            }
        }
    }
    let t = current.trim().to_string();
    if !t.is_empty() {
        blocks.extend(split_bold(t, false, false));
    }
    blocks
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
                Some(p) => p
                    .search(&id)
                    .await
                    .ok()
                    .and_then(|j| serde_json::from_str::<Vec<libarc::Package>>(&j).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .find(|pkg| pkg.id == id),
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

    for block in blocks.iter_mut() {
        if block.is_app {
            if let Some(pkg) = resolved.get(&block.app_id) {
                block.app_name = pkg.name.clone();
                block.app_summary = pkg.description.clone();
                block.app_icon_url = crate::services::icons::resolve(&pkg.id, pkg.icon_url.as_deref());
            }
        }
    }
}
