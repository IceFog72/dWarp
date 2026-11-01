//! Shared AI-related colors used by the terminal input and block list.
use pathfinder_color::ColorU;
use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::{AnsiColorIdentifier, Fill, WarpTheme};
use warpui::{AppContext, SingletonEntity};

/// Background fill for overlay surfaces such as the inline menu.
pub fn agent_view_bg_fill(app: &AppContext) -> Fill {
    let appearance = Appearance::as_ref(app);
    appearance.theme().surface_overlay_1()
}

pub fn agent_view_bg_color(app: &AppContext) -> ColorU {
    agent_view_bg_fill(app).into_solid()
}

/// Accent color used for AI-related UI.
pub fn ai_brand_color(theme: &WarpTheme) -> ColorU {
    AnsiColorIdentifier::Magenta
        .to_ansi_color(&theme.terminal_colors().normal)
        .into()
}
