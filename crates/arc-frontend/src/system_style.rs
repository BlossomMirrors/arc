use std::path::PathBuf;

use ashpd::desktop::settings::{ColorScheme as PortalColorScheme, Settings as PortalSettings};
use ashpd::desktop::Color as PortalColor;
use futures_util::StreamExt;
use ini::Ini;
use slint::language::ColorScheme;
use slint::{Color, ComponentHandle, Weak};

use crate::{AppWindow, Palette, Theme};

// Colors read straight from the active KDE scheme (kdeglobals). Everything that
// is None falls back to the Slint defaults baked into ui/theme.slint.
#[derive(Default, Clone)]
struct KdeScheme {
    accent: Option<Color>,
    window_bg: Option<Color>,
    view_bg: Option<Color>,
    alternate_bg: Option<Color>,
    fg: Option<Color>,
    fg_dim: Option<Color>,
    button_bg: Option<Color>,
    selection_bg: Option<Color>,
    selection_fg: Option<Color>,
    header_bg: Option<Color>,
    negative: Option<Color>,
    neutral: Option<Color>,
    positive: Option<Color>,
    visited: Option<Color>,
    link: Option<Color>,
}

fn kdeglobals_path() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("kdeglobals")
}

// Parse a KDE "r,g,b" (optionally with a fourth alpha component) triple.
fn parse_rgb(raw: &str) -> Option<Color> {
    let mut parts = raw.split(',').map(|p| p.trim().parse::<u8>());
    let r = parts.next()?.ok()?;
    let g = parts.next()?.ok()?;
    let b = parts.next()?.ok()?;
    Some(Color::from_rgb_u8(r, g, b))
}

fn read_kdeglobals() -> KdeScheme {
    let Ok(ini) = Ini::load_from_file(kdeglobals_path()) else {
        return KdeScheme::default();
    };
    let get = |section: &str, key: &str| {
        ini.section(Some(section))
            .and_then(|s| s.get(key))
            .and_then(parse_rgb)
    };

    let accent = get("General", "AccentColor").or_else(|| get("Colors:Selection", "BackgroundNormal"));

    KdeScheme {
        accent,
        window_bg: get("Colors:Window", "BackgroundNormal"),
        view_bg: get("Colors:View", "BackgroundNormal"),
        alternate_bg: get("Colors:View", "BackgroundAlternate"),
        fg: get("Colors:Window", "ForegroundNormal"),
        fg_dim: get("Colors:Window", "ForegroundInactive"),
        button_bg: get("Colors:Button", "BackgroundNormal"),
        selection_bg: get("Colors:Selection", "BackgroundNormal"),
        selection_fg: get("Colors:Selection", "ForegroundNormal"),
        header_bg: get("Colors:Header", "BackgroundNormal"),
        negative: get("Colors:View", "ForegroundNegative"),
        neutral: get("Colors:View", "ForegroundNeutral"),
        positive: get("Colors:View", "ForegroundPositive"),
        visited: get("Colors:View", "ForegroundVisited"),
        link: get("Colors:View", "ForegroundLink"),
    }
}

fn apply_scheme(app: &AppWindow, scheme: &KdeScheme, color_scheme: Option<ColorScheme>) {
    if let Some(cs) = color_scheme {
        app.global::<Palette>().set_color_scheme(cs);
    }
    let theme = app.global::<Theme>();
    if let Some(c) = scheme.accent {
        theme.set_accent(c);
    }
    if let Some(c) = scheme.window_bg {
        theme.set_window_bg(c);
    }
    if let Some(c) = scheme.view_bg {
        theme.set_view_bg(c);
    }
    if let Some(c) = scheme.alternate_bg {
        theme.set_alternate_bg(c);
    }
    if let Some(c) = scheme.fg {
        theme.set_fg(c);
    }
    if let Some(c) = scheme.fg_dim {
        theme.set_fg_dim(c);
    }
    if let Some(c) = scheme.button_bg {
        theme.set_button_bg(c);
    }
    if let Some(c) = scheme.selection_bg {
        theme.set_selection_bg(c);
    }
    if let Some(c) = scheme.selection_fg {
        theme.set_selection_fg(c);
    }
    if let Some(c) = scheme.header_bg {
        theme.set_header_bg(c);
    }
    if let Some(c) = scheme.negative {
        theme.set_danger(c);
    }
    if let Some(c) = scheme.neutral {
        theme.set_warning(c);
    }
    if let Some(c) = scheme.positive {
        theme.set_success(c);
    }
    if let Some(c) = scheme.visited {
        theme.set_pwa(c);
    }
    if let Some(c) = scheme.link {
        theme.set_link(c);
    }
}

fn map_scheme(cs: PortalColorScheme) -> Option<ColorScheme> {
    match cs {
        PortalColorScheme::PreferDark => Some(ColorScheme::Dark),
        PortalColorScheme::PreferLight => Some(ColorScheme::Light),
        PortalColorScheme::NoPreference => None,
    }
}

fn accent_to_color(c: &PortalColor) -> Option<Color> {
    let (r, g, b) = (c.red(), c.green(), c.blue());
    if [r, g, b].iter().any(|v| !(0.0..=1.0).contains(v)) {
        return None;
    }
    Some(Color::from_rgb_u8(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    ))
}

// kdeglobals is the primary source for the full palette; the portal supplies
// the light/dark scheme and (when kdeglobals lacks one) the accent.
async fn read_all(settings: Option<&PortalSettings>) -> (KdeScheme, Option<ColorScheme>) {
    let mut scheme = read_kdeglobals();
    let mut color_scheme = None;
    if let Some(s) = settings {
        if let Ok(cs) = s.color_scheme().await {
            color_scheme = map_scheme(cs);
        }
        if scheme.accent.is_none() {
            if let Ok(c) = s.accent_color().await {
                scheme.accent = accent_to_color(&c);
            }
        }
    }
    (scheme, color_scheme)
}

fn push(ui: &Weak<AppWindow>, scheme: KdeScheme, color_scheme: Option<ColorScheme>) {
    let _ = ui.upgrade_in_event_loop(move |app| {
        apply_scheme(&app, &scheme, color_scheme);
    });
}

pub fn init(rt: &tokio::runtime::Handle, ui: Weak<AppWindow>) {
    // Read once synchronously (bounded) so the window opens already themed.
    let initial = rt.block_on(async {
        tokio::time::timeout(std::time::Duration::from_millis(500), async {
            let settings = PortalSettings::new().await.ok();
            read_all(settings.as_ref()).await
        })
        .await
        .ok()
    });
    if let Some((scheme, color_scheme)) = initial {
        if let Some(app) = ui.upgrade() {
            apply_scheme(&app, &scheme, color_scheme);
        }
    }

    // Follow live changes: any appearance change re-reads the full scheme.
    rt.spawn(async move {
        let Ok(settings) = PortalSettings::new().await else {
            return;
        };
        let (Ok(cs_changed), Ok(accent_changed)) = (
            settings.receive_color_scheme_changed().await,
            settings.receive_accent_color_changed().await,
        ) else {
            return;
        };
        let mut changes = futures_util::stream::select(
            Box::pin(cs_changed.map(|_| ())),
            Box::pin(accent_changed.map(|_| ())),
        );
        while changes.next().await.is_some() {
            let (scheme, color_scheme) = read_all(Some(&settings)).await;
            push(&ui, scheme, color_scheme);
        }
    });
}
