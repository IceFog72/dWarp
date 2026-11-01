//! Generic inline menu view for rendering search results with selection and navigation.
mod message_bar;
mod message_provider;
mod model;
pub(crate) mod positioning;
pub mod styles;
mod view;

use super::InputSuggestionsMode;
use serde::{Deserialize, Serialize};

pub use message_bar::{InlineMenuMessageArgs, InlineMenuMessageBarArgs};
pub use message_provider::{default_navigation_message_items, InlineMenuMessageProvider};
pub use model::{InlineMenuModel, InlineMenuModelEvent, InlineMenuTabConfig};
pub use positioning::InlineMenuPositioner;
pub use view::{
    DetailsRenderConfig, InlineMenuAction, InlineMenuClickBehavior, InlineMenuEvent,
    InlineMenuHeaderConfig, InlineMenuRowAction, InlineMenuView,
};

/// Identifies a specific inline menu type.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "Identifies a specific inline menu.",
    rename_all = "snake_case"
)]
pub enum InlineMenuType {
    SlashCommands,
    SkillMenu,
    InlineHistoryMenu,
    IndexedReposMenu,
}

impl InlineMenuType {
    fn display_label(&self) -> &'static str {
        match self {
            InlineMenuType::SlashCommands => "/Commands",
            InlineMenuType::SkillMenu => "/Skills",
            InlineMenuType::InlineHistoryMenu => "History",
            InlineMenuType::IndexedReposMenu => "/Repos",
        }
    }

    pub(crate) fn from_suggestions_mode(mode: &InputSuggestionsMode) -> Option<Self> {
        match mode {
            InputSuggestionsMode::SlashCommands => Some(InlineMenuType::SlashCommands),
            InputSuggestionsMode::SkillMenu => Some(InlineMenuType::SkillMenu),
            InputSuggestionsMode::InlineHistoryMenu => Some(InlineMenuType::InlineHistoryMenu),
            InputSuggestionsMode::IndexedReposMenu => Some(InlineMenuType::IndexedReposMenu),
            InputSuggestionsMode::Closed
            | InputSuggestionsMode::HistoryUp { .. }
            | InputSuggestionsMode::CompletionSuggestions { .. }
            | InputSuggestionsMode::StaticWorkflowEnumSuggestions { .. }
            | InputSuggestionsMode::DynamicWorkflowEnumSuggestions { .. }
            | InputSuggestionsMode::AIContextMenu { .. } => None,
        }
    }
}
