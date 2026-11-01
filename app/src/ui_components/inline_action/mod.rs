//! Inline action UI pieces (headers, requested actions and scripts, code blocks) shared by
//! terminal flows such as warpify and remote-server setup.

pub(crate) mod code_block;
pub(crate) mod inline_action_header;
pub(crate) mod inline_action_icons;
pub(crate) mod keyboard_navigable_buttons;
pub(crate) mod requested_action;
pub(crate) mod requested_script;

use std::fmt::Display;

use serde::{Deserialize, Serialize};
use warpui::elements::Container;
use warpui::{AppContext, Element};

use crate::terminal::shell::ShellType;
use inline_action_icons::icon_size;

/// Horizontal padding applied to inline content items.
pub(crate) const CONTENT_HORIZONTAL_PADDING: f32 = 20.;

/// Vertical margin between inline content items.
pub(crate) const CONTENT_ITEM_VERTICAL_MARGIN: f32 = 16.;

pub(crate) trait WithContentItemSpacing {
    /// Returns a [`Container`] with standard margin and padding values for a content item.
    fn with_content_item_spacing(self) -> Container;

    /// Returns a [`Container`] content-item spacing with additional left margin that aligns the
    /// item with text that follows an icon.
    fn with_agent_output_item_spacing(self, app: &AppContext) -> Container;
}

impl WithContentItemSpacing for Box<dyn Element> {
    fn with_content_item_spacing(self) -> Container {
        Container::new(self)
            .with_horizontal_margin(CONTENT_HORIZONTAL_PADDING)
            .with_margin_bottom(CONTENT_ITEM_VERTICAL_MARGIN)
    }

    fn with_agent_output_item_spacing(self, app: &AppContext) -> Container {
        let left_margin = CONTENT_HORIZONTAL_PADDING + icon_size(app) + 16.;
        Container::new(self)
            .with_margin_left(left_margin)
            .with_margin_right(CONTENT_HORIZONTAL_PADDING)
            .with_margin_bottom(CONTENT_ITEM_VERTICAL_MARGIN)
    }
}

/// Language of a code snippet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgrammingLanguage {
    Shell(ShellType),
    Other(String),
}

impl ProgrammingLanguage {
    pub fn display_name(&self) -> String {
        match self {
            Self::Shell(shell_type) => shell_type.name().to_owned(),
            Self::Other(language) => language.to_lowercase(),
        }
    }

    /// Return whether this language is a shell language.
    /// This is used to determine whether to show the "execute in terminal" button.
    pub fn is_shell(&self) -> bool {
        matches!(self, Self::Shell(_))
    }
}

impl Display for ProgrammingLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgrammingLanguage::Shell(shell_type) => write!(f, "{}", shell_type.name()),
            ProgrammingLanguage::Other(language) => write!(f, "{}", language.to_lowercase()),
        }
    }
}

impl From<String> for ProgrammingLanguage {
    // Returns a programming language for a markdown language specifier
    fn from(value: String) -> Self {
        if let Some(shell_type) = ShellType::from_markdown_language_spec(value.as_str()) {
            ProgrammingLanguage::Shell(shell_type)
        } else {
            ProgrammingLanguage::Other(value)
        }
    }
}
