use crate::report_if_error;
use crate::settings::{AISettings, CodeSettings};
use crate::workspace::tab_settings::TabSettings;
use onboarding::slides::AgentDevelopmentSettings;
use onboarding::{SelectedSettings, UICustomizationSettings};
use settings::Setting as _;
use warp_core::features::FeatureFlag;
use warpui::{AppContext, SingletonEntity as _};

/// Applies onboarding settings based on the user's selected mode.
pub fn apply_onboarding_settings(selected_settings: &SelectedSettings, app: &mut AppContext) {
    let is_ai_enabled = match selected_settings {
        SelectedSettings::AgentDrivenDevelopment {
            agent_settings,
            ui_customization,
            ..
        } => {
            apply_agent_settings(agent_settings, app);
            let is_ai_enabled = !agent_settings.disable_oz;
            if let Some(ui) = ui_customization {
                apply_ui_customization_settings(ui, app);
            }
            is_ai_enabled
        }
        SelectedSettings::Terminal {
            ui_customization,
            cli_agent_toolbar_enabled,
            show_agent_notifications,
        } => {
            // In old onboarding, there's nothing to set for terminal intent.
            if !FeatureFlag::OpenWarpNewSettingsModes.is_enabled() {
                true
            } else {
                if let Some(ui) = ui_customization {
                    apply_ui_customization_settings(ui, app);
                }
                AISettings::handle(app).update(app, |settings, ctx| {
                    report_if_error!(settings
                        .should_render_cli_agent_footer
                        .set_value(*cli_agent_toolbar_enabled, ctx));
                    report_if_error!(settings
                        .show_agent_notifications
                        .set_value(*show_agent_notifications, ctx));
                });
                false
            }
        }
    };

    if FeatureFlag::OpenWarpNewSettingsModes.is_enabled() {
        AISettings::handle(app).update(app, |settings, ctx| {
            report_if_error!(settings.is_any_ai_enabled.set_value(is_ai_enabled, ctx));
        });
    }
}

/// Applies the explicit UI customization settings chosen during the
/// "Customize your UI" onboarding slide.
fn apply_ui_customization_settings(ui: &UICustomizationSettings, app: &mut AppContext) {
    // Customize UI slide should only exist with this flag enabled.
    if !FeatureFlag::OpenWarpNewSettingsModes.is_enabled() {
        return;
    }
    TabSettings::handle(app).update(app, |settings, ctx| {
        report_if_error!(settings
            .use_vertical_tabs
            .set_value(ui.use_vertical_tabs, ctx));
        report_if_error!(settings
            .show_code_review_button
            .set_value(ui.show_code_review_button, ctx));
    });

    CodeSettings::handle(app).update(app, |settings, ctx| {
        report_if_error!(settings
            .show_project_explorer
            .set_value(ui.show_project_explorer, ctx));
        report_if_error!(settings
            .show_global_search
            .set_value(ui.show_global_search, ctx));
    });
}

fn apply_agent_settings(agent_settings: &AgentDevelopmentSettings, app: &mut AppContext) {
    AISettings::handle(app).update(app, |settings, ctx| {
        report_if_error!(settings
            .should_render_cli_agent_footer
            .set_value(agent_settings.cli_agent_toolbar_enabled, ctx));
        report_if_error!(settings
            .show_agent_notifications
            .set_value(agent_settings.show_agent_notifications, ctx));
    });
}
