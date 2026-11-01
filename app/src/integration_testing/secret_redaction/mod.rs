mod assertion;

pub use assertion::*;

use settings::Setting;
use warpui::{integration::TestStep, SingletonEntity};

use crate::terminal::safe_mode_settings::SafeModeSettings;

pub fn toggle_secret_redaction() -> TestStep {
    TestStep::new("Toggle local secret redaction").with_action(|app, _, _| {
        SafeModeSettings::handle(app).update(app, |settings, ctx| {
            let enabled = !*settings.safe_mode_enabled;
            settings.safe_mode_enabled.set_value(enabled, ctx).unwrap();
        });
    })
}

pub fn toggle_hide_secrets() -> TestStep {
    TestStep::new("Toggle local secret visibility").with_action(|app, _, _| {
        SafeModeSettings::handle(app).update(app, |settings, ctx| {
            let hidden = !*settings.hide_secrets_in_block_list;
            settings
                .hide_secrets_in_block_list
                .set_value(hidden, ctx)
                .unwrap();
        });
    })
}
