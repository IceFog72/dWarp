use byte_unit::Byte;
use serde::{Deserialize, Serialize};
use session_sharing_protocol::common::Role;
#[cfg(test)]
use session_sharing_protocol::common::{Scrollback, ScrollbackBlock};
use session_sharing_protocol::sharer::SessionSourceType;
use warpui::{id, keymap::ContextPredicate};

use crate::editor::{InteractionState, ReplicaId};

#[cfg(test)]
use super::model::block::SerializedBlock;
use super::{model::terminal_model::BlockIndex, GridType, TerminalModel};

pub mod manager;
pub mod network;
pub mod participant_avatar_view;
pub mod presence_manager;
pub mod render_util;
pub mod role_change_modal;
mod selections;
pub mod settings;
pub mod viewer;

#[cfg(test)]
pub use tests::MAX_BYTES_SHAREABLE;

/// The type of shared session a particular session is, if applicable.
#[derive(Debug, Clone)]
pub enum SharedSessionStatus {
    /// This session is not a shared session.
    /// When a sharer ends a session, the status
    /// changes back to [`SharedSessionStatus::NotShared`].
    NotShared,

    /// We're in the process of joining the session but have not
    /// established the connection with the server yet, or have not received all the events that occurred before the viewer joined yet.
    ViewPending,

    /// This session is a shared session that we are actively viewing.
    /// We have received all the scrollback and events for the shared session that occurred before the viewer joined, and are caught up and receiving events live.
    ActiveViewer { role: Role },

    /// We were viewing a shared session but it ended.
    FinishedViewer,

    /// We haven't yet attempted to share the session because it is not bootstrapped yet.
    /// The `source_type` encodes what kind of shared session will be created once the
    /// session finishes bootstrapping.
    SharePendingPreBootstrap { source_type: SessionSourceType },

    /// The session is bootstrapped and we're in the process of
    /// sharing the session but have not yet established the
    /// connection with the server.
    SharePending,

    /// This session is actively being shared.
    ActiveSharer,
}

impl SharedSessionStatus {
    pub fn reader() -> Self {
        Self::ActiveViewer { role: Role::Reader }
    }

    pub fn executor() -> Self {
        Self::ActiveViewer {
            role: Role::Executor,
        }
    }

    pub fn is_view_pending(&self) -> bool {
        matches!(self, SharedSessionStatus::ViewPending)
    }

    pub fn is_active_viewer(&self) -> bool {
        matches!(self, SharedSessionStatus::ActiveViewer { .. })
    }

    pub fn is_finished_viewer(&self) -> bool {
        matches!(self, SharedSessionStatus::FinishedViewer)
    }

    pub fn is_viewer(&self) -> bool {
        self.is_view_pending() || self.is_active_viewer() || self.is_finished_viewer()
    }

    pub fn is_executor(&self) -> bool {
        matches!(self, SharedSessionStatus::ActiveViewer { role } if role.can_execute())
    }

    pub fn is_reader(&self) -> bool {
        matches!(
            self,
            SharedSessionStatus::ActiveViewer { role: Role::Reader }
        )
    }

    pub fn is_share_pending(&self) -> bool {
        matches!(
            self,
            SharedSessionStatus::SharePending
                | SharedSessionStatus::SharePendingPreBootstrap { .. }
        )
    }

    pub fn is_active_sharer(&self) -> bool {
        matches!(self, SharedSessionStatus::ActiveSharer)
    }

    pub fn is_sharer(&self) -> bool {
        self.is_share_pending() || self.is_active_sharer()
    }

    pub fn is_sharer_or_viewer(&self) -> bool {
        !matches!(self, Self::NotShared)
    }

    pub fn as_keymap_context(&self) -> &'static str {
        match self {
            Self::NotShared => "SharedSessionStatus_NotShared",
            Self::ViewPending => "SharedSessionStatus_ViewPending",
            Self::ActiveViewer { role: Role::Reader } => "SharedSessionStatus_Reader",
            Self::ActiveViewer {
                role: Role::Executor | Role::Full,
            } => "SharedSessionStatus_Executor",
            Self::FinishedViewer => "SharedSessionStatus_FinishedViewer",
            Self::SharePendingPreBootstrap { .. } => "SharedSessionStatus_SharePendingPreBootstrap",
            Self::SharePending => "SharedSessionStatus_SharePending",
            Self::ActiveSharer => "SharedSessionStatus_ActiveSharer",
        }
    }

    pub fn active_viewer_keymap_context() -> ContextPredicate {
        id!(Self::reader().as_keymap_context()) | id!(Self::executor().as_keymap_context())
    }
}

/// The scrollback options when starting a shared session.
/// Note: currently, these options only encode the point at which
/// scrollback _starts_. We do not yet support more
/// selective scrollback (e.g. a closed range).
/// The active block is always included in scrollback for the prompt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedSessionScrollbackType {
    /// Do not include any scrollback in this shared session.
    /// Note the active block is still sent as part of scrollback for the prompt.
    /// TODO(suraj): consider renaming this to "from active block" or encapsulating
    /// this with the `FromBlock` variant with the block_index equal to the
    /// active block index.
    None,

    /// Include scrollback starting at `block_index`.
    FromBlock { block_index: BlockIndex },

    /// The entire blocklist should be part of the scrollback.
    All,
}

impl SharedSessionScrollbackType {
    /// Returns the set of scrollback that adheres to the scrollback type.
    /// Note that some blocks might not actually be included in the scrollback
    /// even if they were specified as part of the scrollback type.
    /// For example, if the [`Self::All]` variant is used, restored blocks
    /// _won't_ be included in scrollback.
    #[cfg(test)]
    fn to_scrollback(self, model: &TerminalModel) -> Scrollback {
        let first_block_index = self.first_block_index(model);
        let blocks = model
            .block_list()
            .blocks()
            .iter()
            .skip(first_block_index.into())
            .filter(|block| block.is_scrollback_block_for_shared_session())
            .filter_map(|block| {
                let serialized_block: SerializedBlock = block.into();
                let bytes = serde_json::to_vec(&serialized_block);
                bytes.ok().map(|raw| ScrollbackBlock { raw })
            })
            .collect();

        let is_alt_screen_active = model.is_alt_screen_active();

        Scrollback {
            blocks,
            is_alt_screen_active,
        }
    }

    /// Returns the first block index that will be used for scrollback.
    pub fn first_block_index(self, model: &TerminalModel) -> BlockIndex {
        match self {
            Self::None => model.block_list().active_block_index(),
            Self::FromBlock { block_index } => model
                .block_list()
                .blocks()
                .iter()
                .skip(block_index.into())
                .find(|block| block.is_scrollback_block_for_shared_session())
                .map_or(model.block_list().active_block_index(), |block| {
                    block.index()
                }),
            Self::All => Self::FromBlock {
                block_index: BlockIndex::zero(),
            }
            .first_block_index(model),
        }
    }
}

pub fn max_session_size() -> Byte {
    Byte::from_u64_with_unit(100, byte_unit::Unit::MB).unwrap()
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum SharedSessionActionSource {
    /// From right-click menu in blocklist
    /// * `block_index`: provided with selected block, none when no blocks selected
    BlocklistContextMenu {
        block_index: Option<BlockIndex>,
    },
    Tab,
    PaneHeader,
    /// Includes keybindings.
    CommandPalette,
    OnboardingBlock,
    Closed {
        is_confirm_close_session: bool,
    },
    InactivityModal,
    /// The user did not initiate this action themselves.
    NonUser,
    /// The object-specific sharing dialog.
    SharingDialog,
    /// From the session sharing context menu items.
    RightClickMenu,
    /// From the agent/CLI footer chip.
    FooterChip,
}

impl From<GridType> for session_sharing_protocol::common::GridType {
    fn from(val: GridType) -> Self {
        match val {
            GridType::Prompt => session_sharing_protocol::common::GridType::Prompt,
            GridType::Rprompt => session_sharing_protocol::common::GridType::Rprompt,
            GridType::Output => session_sharing_protocol::common::GridType::Output,
            GridType::PromptAndCommand => {
                session_sharing_protocol::common::GridType::PromptAndCommand
            }
        }
    }
}

impl From<session_sharing_protocol::common::GridType> for GridType {
    fn from(value: session_sharing_protocol::common::GridType) -> Self {
        match value {
            session_sharing_protocol::common::GridType::Prompt => Self::Prompt,
            session_sharing_protocol::common::GridType::Rprompt => Self::Rprompt,
            session_sharing_protocol::common::GridType::Output => Self::Output,
            session_sharing_protocol::common::GridType::PromptAndCommand => Self::PromptAndCommand,
        }
    }
}

impl From<ReplicaId> for session_sharing_protocol::common::InputReplicaId {
    fn from(value: ReplicaId) -> Self {
        value.to_string().into()
    }
}

impl From<session_sharing_protocol::common::InputReplicaId> for ReplicaId {
    fn from(value: session_sharing_protocol::common::InputReplicaId) -> Self {
        ReplicaId::new(value)
    }
}

impl From<&Role> for InteractionState {
    fn from(value: &Role) -> InteractionState {
        match value {
            Role::Reader => InteractionState::Selectable,
            Role::Executor => InteractionState::Editable,
            Role::Full => InteractionState::Editable,
        }
    }
}

/// Decode scrollback blocks from their JSON wire format into [`SerializedBlock`]s.
///
/// Blocks that fail to deserialize are silently dropped.
#[cfg(test)]
pub(crate) fn decode_scrollback(scrollback: &Scrollback) -> Vec<SerializedBlock> {
    scrollback
        .blocks
        .iter()
        .filter_map(|block| serde_json::from_slice(&block.raw).ok())
        .collect()
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
