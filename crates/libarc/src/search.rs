use crate::Package;

/// Returns true if every character in `needle` appears in `haystack` in order.
pub fn is_subsequence(needle: &str, haystack: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut h = haystack.chars();
    needle.chars().all(|c| h.any(|hc| hc == c))
}

/// Scores how well `field` matches `q` (both already lowercased).
/// Higher score = better match.
pub fn score_field(field: &str, q: &str) -> u32 {
    if field == q {
        return 100;
    }
    if field.starts_with(q) {
        return 80;
    }
    if field.contains(q) {
        return 60;
    }
    if is_subsequence(q, field) {
        return 30;
    }
    0
}

/// Scores a package against a lowercased query.
/// Returns `None` if the package has no match at all.
pub fn score_package(pkg: &Package, q: &str) -> Option<u32> {
    let name = pkg.name.to_lowercase();
    let id = pkg.id.to_lowercase();
    let desc = pkg.description.to_lowercase();
    let best = score_field(&name, q)
        .max(score_field(&id, q).saturating_sub(10))
        .max(score_field(&desc, q).saturating_sub(30));
    if best > 0 { Some(best) } else { None }
}

/// Filters `packages` to those matching `query` and returns them sorted
/// by relevance (highest score first).
pub fn search_and_rank(packages: Vec<Package>, query: &str) -> Vec<Package> {
    let q = query.to_lowercase();
    let mut scored: Vec<(Package, u32)> = packages
        .into_iter()
        .filter_map(|p| score_package(&p, &q).map(|s| (p, s)))
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(p, _)| p).collect()
}
