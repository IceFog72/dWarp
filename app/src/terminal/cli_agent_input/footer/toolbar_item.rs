use serde::{Deserialize, Serialize};

use crate::context_chips::{available_chips, ContextChipKind};
use crate::features::FeatureFlag;
use crate::terminal::shared_session::SharedSessionStatus;
use crate::ui_components::icons::Icon;

/// A configurable item
///
/// This unifies context-chip data displays with interactive control buttons so
/// they can all be arranged through the same drag-and-drop editor.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[schemars(
    description = "An item that can appear in the agent toolbar.",
    rename_all = "snake_case"
)]
pub enum AgentToolbarItemKind {
    #[schemars(description = "A prompt context chip.")]
    ContextChip(ContextChipKind),
    FileExplorer,
    RichInput,
    // Renamed from ImageAttach; alias preserves existing user toolbar configs.
    #[serde(alias = "ImageAttach")]
    FileAttach,
    /// Opens settings to the Coding Agents section.
    Settings,

    // Items of removed features (session sharing, voice input, the Warp agent footer). Kept only so saved custom
    // layouts that list them still load. They are never offered or shown. See [`Self::is_removed`].
    #[schemars(skip)]
    ShareSession,
    #[schemars(skip)]
    VoiceInput,
    #[schemars(skip)]
    ModelSelector,
    #[schemars(skip)]
    NLDToggle,
    #[schemars(skip)]
    ContextWindowUsage,
    #[schemars(skip)]
    FastForwardToggle,
}

impl AgentToolbarItemKind {
    /// Whether this item should be visible to session viewers.
    /// Items that control host settings or initiate actions on the host's
    /// behalf are hidden from viewers.
    pub fn available_to_session_viewer(&self, status: &SharedSessionStatus) -> bool {
        match self {
            Self::Settings | Self::ShareSession | Self::FileExplorer | Self::FileAttach => {
                !status.is_viewer()
            }
            Self::FastForwardToggle => !status.is_viewer() || status.is_executor(),
            Self::ContextChip(_)
            | Self::ModelSelector
            | Self::NLDToggle
            | Self::ContextWindowUsage
            | Self::RichInput
            | Self::VoiceInput => true,
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::ContextChip(_) => "Context Chip",
            Self::ModelSelector => "Model Selector",
            Self::NLDToggle => "Autodetection",
            Self::VoiceInput => "Voice Input",
            Self::FileAttach => "Attach File",
            Self::ContextWindowUsage => "Context Usage",
            Self::FileExplorer => "File Explorer",
            Self::RichInput => "Rich Input",
            Self::ShareSession => "/remote-control",
            Self::Settings => "Settings",
            Self::FastForwardToggle => "Fast Forward",
        }
    }

    pub fn icon(&self) -> Option<Icon> {
        match self {
            Self::ContextChip(kind) => kind.udi_icon(),
            Self::ModelSelector => Some(Icon::Oz),
            Self::NLDToggle => Some(Icon::NLD),
            Self::VoiceInput => Some(Icon::Microphone),
            Self::FileAttach => Some(Icon::Plus),
            Self::ContextWindowUsage => Some(Icon::ConversationContext0),
            Self::FileExplorer => Some(Icon::FileCopy),
            Self::RichInput => Some(Icon::TextInput),
            Self::ShareSession => Some(Icon::Phone01),
            Self::Settings => Some(Icon::Settings),
            Self::FastForwardToggle => Some(Icon::FastForward),
        }
    }

    /// Whether this item belongs to a removed feature and must be dropped from saved layouts.
    pub fn is_removed(&self) -> bool {
        matches!(
            self,
            Self::ShareSession
                | Self::VoiceInput
                | Self::ModelSelector
                | Self::NLDToggle
                | Self::ContextWindowUsage
                | Self::FastForwardToggle
                | Self::ContextChip(ContextChipKind::AgentPlanAndTodoList)
        )
    }

    pub fn is_context_chip(&self) -> bool {
        matches!(self, Self::ContextChip(_))
    }

    pub fn context_chip_kind(&self) -> Option<&ContextChipKind> {
        match self {
            Self::ContextChip(kind) => Some(kind),
            _ => None,
        }
    }

    /// Default left-side items for the CLI agent footer.
    pub fn cli_default_left() -> Vec<Self> {
        let mut items = vec![
            Self::FileAttach,
            Self::ContextChip(ContextChipKind::GitDiffStats),
        ];
        items.push(Self::FileExplorer);
        if FeatureFlag::CLIAgentRichInput.is_enabled() {
            items.push(Self::RichInput);
        }
        items
    }

    /// Default right-side items for the CLI agent footer.
    pub fn cli_default_right() -> Vec<Self> {
        vec![
            Self::ContextChip(ContextChipKind::WorkingDirectory),
            Self::ContextChip(ContextChipKind::ShellGitBranch),
            Self::Settings,
        ]
    }

    /// All items available for the CLI agent footer configurator.
    pub fn all_available_for_cli_input() -> Vec<Self> {
        let mut items: Vec<Self> = available_chips()
            .into_iter()
            .map(Self::ContextChip)
            .collect();
        items.extend([
            Self::FileExplorer,
            Self::RichInput,
            Self::FileAttach,
            Self::Settings,
        ]);
        items
    }
}

impl From<ContextChipKind> for AgentToolbarItemKind {
    fn from(kind: ContextChipKind) -> Self {
        Self::ContextChip(kind)
    }
}
