use super::TerminalAction;

use crate::settings_view::flags;
use crate::terminal::ssh::error::{SshErrorBlockAction, SSH_ERROR_BLOCK_VISIBLE_KEY};
use crate::util::bindings::{cmd_or_ctrl_shift, is_binding_pty_compliant};
use crate::{
    channel::{Channel, ChannelState},
    features::FeatureFlag,
    terminal::model::{
        escape_sequences::{self, EscCodes},
        selection::SelectionDirection,
    },
    terminal::TerminalView,
    util::bindings::CustomAction,
};
use warpui::keymap::{BindingDescription, PerPlatformKeystroke};
use warpui::platform::OperatingSystem;
use warpui::{
    keymap::{EditableBinding, FixedBinding},
    units::IntoLines,
    AppContext,
};

pub const TOGGLE_BLOCK_FILTER_KEYBINDING: &str =
    "terminal:toggle_block_filter_on_selected_or_last_block";

pub const CANCEL_COMMAND_KEYBINDING: &str = "terminal:cancel_command";
pub const OPEN_CLI_AGENT_RICH_INPUT_KEYBINDING: &str = "terminal:open_cli_agent_rich_input";

const SELECT_NEXT_BLOCK_ACTION_NAME: &str = "terminal:select_next_block";
pub const SELECT_PREVIOUS_BLOCK_ACTION_NAME: &str = "terminal:select_previous_block";

pub const INPUT_BOX_VISIBLE_KEY: &str = "InputVisible";
pub const KEYBOARD_PROTOCOL_ENABLED_KEY: &str = "KeyboardProtocolEnabled";
pub const CLI_AGENT_SESSION_ACTIVE_KEY: &str = "CLIAgentSessionActive";

/// Some keybindings will do different things in different contexts. We break
/// these into their own function to ensure we pay special attention to
/// these overlaps, and ensure only 1 action is taken.
fn init_overlapping_keybindings(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    let escape_key: &str = "escape";
    let cmd_or_ctrl_enter: &str = "cmdorctrl-enter";

    // No Active Block Context
    app.register_fixed_bindings([FixedBinding::new(
        escape_key,
        TerminalAction::MaybeDismissToolTip {
            from_keybinding: true,
        },
        !id!(SSH_ERROR_BLOCK_VISIBLE_KEY) & id!("Terminal"),
    )]);

    let block_action_context = || id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand");

    // SSH Error Block Context
    app.register_fixed_bindings([
        FixedBinding::new(
            escape_key,
            TerminalAction::NotifySshErrorBlock(SshErrorBlockAction::ContinueWithoutWarpification),
            id!(SSH_ERROR_BLOCK_VISIBLE_KEY) & block_action_context(),
        ),
        FixedBinding::new(
            cmd_or_ctrl_enter,
            TerminalAction::NotifySshErrorBlock(SshErrorBlockAction::ContinueWithoutWarpification),
            id!(SSH_ERROR_BLOCK_VISIBLE_KEY) & block_action_context(),
        ),
    ]);
}

/// Register keybindings for [`TerminalView`] actions.
pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_binding_validator::<TerminalView>(is_binding_pty_compliant);

    init_overlapping_keybindings(app);

    app.register_fixed_bindings([
        FixedBinding::new("up", TerminalAction::Up, id!("Terminal") & !id!("IMEOpen")),
        FixedBinding::new(
            "down",
            TerminalAction::Down,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "left",
            TerminalAction::UserInputSequence(vec![EscCodes::ARROW_LEFT]),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "right",
            TerminalAction::UserInputSequence(vec![EscCodes::ARROW_RIGHT]),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "home",
            TerminalAction::Home,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "end",
            TerminalAction::End,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "shift-enter",
            TerminalAction::KeyDown("\n".to_owned()),
            id!("Terminal")
                & !id!("IMEOpen")
                & (id!("LongRunningCommand") | id!("AltScreen"))
                & !id!(KEYBOARD_PROTOCOL_ENABLED_KEY),
        ),
        FixedBinding::new(
            "numpadenter",
            TerminalAction::KeyDown("\r".to_owned()),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "backspace",
            TerminalAction::ControlSequence("\x7f".as_bytes().to_vec()),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "insert",
            TerminalAction::ControlSequence("\x1b[2~".as_bytes().to_vec()),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "delete",
            TerminalAction::ControlSequence("\x1b[3~".as_bytes().to_vec()),
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "pageup",
            TerminalAction::PageUp,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        FixedBinding::new(
            "pagedown",
            TerminalAction::PageDown,
            id!("Terminal") & !id!("IMEOpen"),
        ),
        // On the web, we get pastes from system paste events.
        #[cfg(target_family = "wasm")]
        FixedBinding::standard(
            warpui::actions::StandardAction::Paste,
            TerminalAction::Paste,
            id!("Terminal") & !id!("IMEOpen"),
        ),
    ]);
    if ChannelState::channel() == Channel::Integration {
        app.register_fixed_bindings([
            // Hack: Add explicit bindings for the tests, since the tests' injected
            // keypresses won't trigger Mac menu items. Unfortunately we can't use
            // cfg[test] because we are a separate process!
            FixedBinding::new(
                cmd_or_ctrl_shift("l"),
                TerminalAction::FocusInputAndClearSelection,
                id!("Terminal"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("f"),
                TerminalAction::ShowFindBar,
                id!("Terminal"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("k"),
                TerminalAction::ClearBuffer,
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("d"),
                TerminalAction::SplitRight(None),
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new_per_platform(
                PerPlatformKeystroke {
                    mac: "cmd-shift-D",
                    linux_and_windows: "ctrl-shift-E",
                },
                TerminalAction::SplitDown(None),
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("v"),
                TerminalAction::Paste,
                id!("Terminal") & !id!("IMEOpen"),
            ),
            FixedBinding::new(
                cmd_or_ctrl_shift("c"),
                TerminalAction::Copy,
                id!("Terminal") & !id!("IMEOpen"),
            ),
        ]);
    }

    // By default, Windows Terminal recognizes both `ctrl-v` and `ctrl-shift-v` to paste into the
    // terminal. It also allows users to disable it, so we also make this an EditableBinding.
    #[cfg(windows)]
    app.register_editable_bindings([EditableBinding::new(
        "terminal:alternate_terminal_paste",
        crate::t!("keybinding-desc-terminal-alternate-paste"),
        TerminalAction::Paste,
    )
    .with_key_binding("ctrl-v")
    .with_context_predicate(id!("Terminal") & !id!("IMEOpen"))]);

    app.register_fixed_bindings([
        FixedBinding::new(
            "shift-left",
            TerminalAction::KeyboardSelectText(SelectionDirection::Left),
            id!("Terminal") & !id!("IMEOpen") & id!("ActiveBlockTextSelection"),
        ),
        FixedBinding::new(
            "shift-right",
            TerminalAction::KeyboardSelectText(SelectionDirection::Right),
            id!("Terminal") & !id!("IMEOpen") & id!("ActiveBlockTextSelection"),
        ),
        FixedBinding::new(
            "shift-up",
            TerminalAction::KeyboardSelectText(SelectionDirection::Up),
            id!("Terminal") & !id!("IMEOpen") & id!("ActiveBlockTextSelection"),
        ),
        FixedBinding::new(
            "shift-down",
            TerminalAction::KeyboardSelectText(SelectionDirection::Down),
            id!("Terminal") & !id!("IMEOpen") & id!("ActiveBlockTextSelection"),
        ),
    ]);

    app.register_editable_bindings([
        // Ctrl-G: open rich input for CLI agents before the keystroke reaches the PTY.
        EditableBinding::new(
            OPEN_CLI_AGENT_RICH_INPUT_KEYBINDING,
            crate::t!("keybinding-desc-terminal-toggle-cli-agent-rich-input"),
            TerminalAction::OpenCLIAgentRichInput,
        )
        .with_key_binding("ctrl-g")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & (id!("LongRunningCommand") | id!("AltScreen"))
                & id!(flags::CLI_AGENT_FOOTER_ENABLED)
                & id!(flags::CLI_AGENT_RICH_INPUT_CHIP_ENABLED),
        ),
        EditableBinding::new(
            "terminal:warpify_subshell",
            crate::t!("keybinding-desc-terminal-warpify-subshell"),
            TerminalAction::TriggerSubshellBootstrap,
        )
        .with_key_binding("ctrl-i")
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand") & id!("SubshellBanner"),
        ),
        EditableBinding::new(
            "terminal:warpify_ssh_session",
            crate::t!("keybinding-desc-terminal-warpify-ssh-session"),
            TerminalAction::WarpifySSHSession,
        )
        .with_key_binding("ctrl-i")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & id!("LongRunningCommand")
                & id!("SshWarpificationBanner"),
        ),
        EditableBinding::new(
            CANCEL_COMMAND_KEYBINDING,
            if cfg!(windows) {
                crate::t!("keybinding-desc-terminal-cancel-process-windows")
            } else {
                crate::t!("keybinding-desc-terminal-cancel-process")
            },
            TerminalAction::CtrlC,
        )
        .with_key_binding("ctrl-c")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:focus_input",
            crate::t!("keybinding-desc-terminal-focus-input"),
            TerminalAction::FocusInputAndClearSelection,
        )
        .with_custom_action(CustomAction::FocusInput)
        .with_context_predicate(id!("Terminal")),
        // Paste is not rebindable on the web.
        #[cfg(not(target_family = "wasm"))]
        EditableBinding::new(
            "terminal:paste",
            crate::t!("keybinding-desc-terminal-paste"),
            TerminalAction::Paste,
        )
        .with_custom_action(CustomAction::Paste)
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:copy",
            crate::t!("keybinding-desc-terminal-copy"),
            TerminalAction::Copy,
        )
        .with_custom_action(CustomAction::Copy)
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:reinput_commands",
            crate::t!("keybinding-desc-terminal-reinput-commands"),
            TerminalAction::ReinputCommands,
        )
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:reinput_commands_with_sudo",
            crate::t!("keybinding-desc-terminal-reinput-commands-sudo"),
            TerminalAction::ReinputCommandsWithSudo,
        )
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:find",
            crate::t!("keybinding-desc-terminal-find"),
            TerminalAction::ShowFindBar,
        )
        .with_key_binding(cmd_or_ctrl_shift("f"))
        .with_custom_action(CustomAction::Find)
        .with_context_predicate(id!("Terminal")),
        EditableBinding::new(
            "terminal:select_bookmark_up",
            crate::t!("keybinding-desc-terminal-select-bookmark-up"),
            TerminalAction::SelectBookmarkUp,
        )
        .with_key_binding("alt-up")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:select_bookmark_down",
            crate::t!("keybinding-desc-terminal-select-bookmark-down"),
            TerminalAction::SelectBookmarkDown,
        )
        .with_key_binding("alt-down")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen")),
        EditableBinding::new(
            "terminal:open_block_list_context_menu_via_keybinding",
            crate::t!("keybinding-desc-terminal-open-block-context-menu"),
            TerminalAction::OpenBlockListContextMenu,
        )
        .with_mac_key_binding("ctrl-m")
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:copy_git_branch",
            crate::t!("keybinding-desc-terminal-copy-git-branch"),
            TerminalAction::CopyGitBranch,
        )
        .with_context_predicate(
            id!("Terminal")
                & (eq!("TerminalView_BlockSelectionCardinality", "One")
                    | eq!("TerminalView_BlockSelectionCardinality", "None")),
        ),
        EditableBinding::new(
            "terminal:clear_blocks",
            crate::t!("keybinding-desc-terminal-clear-blocks"),
            TerminalAction::ClearBuffer,
        )
        .with_custom_action(CustomAction::ClearBlocks)
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & id!("TerminalView_NonEmptyBlockList"),
        ),
        EditableBinding::new(
            "terminal:executing_command_move_cursor_word_left",
            crate::t!("keybinding-desc-terminal-cursor-word-left"),
            TerminalAction::ControlSequence(Vec::from(EscCodes::WORD_LEFT)),
        )
        .with_mac_key_binding("alt-left")
        .with_linux_or_windows_key_binding("ctrl-left")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_move_cursor_word_right",
            crate::t!("keybinding-desc-terminal-cursor-word-right"),
            TerminalAction::ControlSequence(Vec::from(EscCodes::WORD_RIGHT)),
        )
        .with_mac_key_binding("alt-right")
        .with_linux_or_windows_key_binding("ctrl-right")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_move_cursor_home",
            crate::t!("keybinding-desc-terminal-cursor-home"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::SOH]),
        )
        // We already have bindings for home/end (the keybindings for this on Linux and Mac) that
        // send the correct control sequence to the PTY.
        .with_mac_key_binding("cmd-left")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_move_cursor_end",
            crate::t!("keybinding-desc-terminal-cursor-end"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::ENQ]),
        )
        .with_mac_key_binding("cmd-right")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_delete_word_left",
            crate::t!("keybinding-desc-terminal-delete-word-left"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::ETB]),
        )
        .with_mac_key_binding("alt-backspace")
        .with_linux_or_windows_key_binding("ctrl-backspace")
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand")),
        EditableBinding::new(
            "terminal:executing_command_delete_line_start",
            crate::t!("keybinding-desc-terminal-delete-line-start"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::NAK]),
        )
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand"))
        // Set this for mac-only. The default binding for this on Linux / Windows is `ctrl-y`, which
        // we can't hijack because it is already reserved for the PTY.
        .with_mac_key_binding("cmd-backspace"),
        EditableBinding::new(
            "terminal:executing_command_delete_line_end",
            crate::t!("keybinding-desc-terminal-delete-line-end"),
            TerminalAction::ControlSequence(vec![escape_sequences::C0::VT]),
        )
        .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & id!("LongRunningCommand"))
        // Set this for mac-only since the corresponding editor action is also Mac-only.
        .with_mac_key_binding("cmd-delete"),
        EditableBinding::new(
            "terminal:backward_tabulation",
            crate::t!("keybinding-desc-terminal-backward-tabulation"),
            TerminalAction::ControlSequence(EscCodes::build_escape_sequence_with_c1(
                escape_sequences::C1::CSI,
                EscCodes::BACKWARD_TABULATION,
            )),
        )
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & (id!("LongRunningCommand") | id!("AltScreen")),
        )
        .with_key_binding("shift-tab"),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            SELECT_PREVIOUS_BLOCK_ACTION_NAME,
            crate::t!("keybinding-desc-terminal-select-previous-block"),
            TerminalAction::SelectPriorBlock,
        )
        .with_custom_action(CustomAction::SelectBlockAbove)
        .with_context_predicate(
            id!("Terminal") & id!("TerminalView_NonEmptyBlockList") & !id!("AltScreen"),
        ),
        EditableBinding::new(
            SELECT_NEXT_BLOCK_ACTION_NAME,
            crate::t!("keybinding-desc-terminal-select-next-block"),
            TerminalAction::SelectNextBlock,
        )
        .with_custom_action(CustomAction::SelectBlockBelow)
        .with_context_predicate(
            id!("Terminal") & id!("TerminalView_NonEmptyBlockList") & !id!("AltScreen"),
        ),
        EditableBinding::new(
            "terminal:bookmark_selected_block",
            crate::t!("keybinding-desc-terminal-bookmark-selected-block"),
            TerminalAction::BookmarkSelectedBlock,
        )
        .with_custom_action(CustomAction::ToggleBookmarkBlock)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:find",
            crate::t!("keybinding-desc-terminal-find-within-selected-block"),
            TerminalAction::ShowFindBar,
        )
        .with_custom_action(CustomAction::FindWithinBlock)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:copy",
            crate::t!("keybinding-desc-terminal-copy-command-and-output"),
            TerminalAction::Copy,
        )
        .with_custom_action(CustomAction::CopyBlock)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:copy_outputs",
            crate::t!("keybinding-desc-terminal-copy-command-output"),
            TerminalAction::CopyOutputs,
        )
        .with_custom_action(CustomAction::CopyBlockOutput)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
        EditableBinding::new(
            "terminal:copy_commands",
            crate::t!("keybinding-desc-terminal-copy-command"),
            TerminalAction::CopyCommands,
        )
        .with_custom_action(CustomAction::CopyBlockCommand)
        .with_context_predicate(
            id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
        ),
    ]);

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:scroll_up_one_line",
            crate::t!("keybinding-desc-terminal-scroll-up-one-line"),
            TerminalAction::Scroll {
                delta: 1.0.into_lines(),
            },
        )
        .with_context_predicate(id!("Terminal") & id!("TerminalView_NonEmptyBlockList")),
        EditableBinding::new(
            "terminal:scroll_down_one_line",
            crate::t!("keybinding-desc-terminal-scroll-down-one-line"),
            TerminalAction::Scroll {
                delta: -(1.0.into_lines()),
            },
        )
        .with_context_predicate(id!("Terminal") & id!("TerminalView_NonEmptyBlockList")),
    ]);

    app.register_editable_bindings([EditableBinding::new(
        "terminal:scroll_to_top_of_selected_block",
        crate::t!("keybinding-desc-terminal-scroll-to-top-of-block"),
        TerminalAction::ScrollToTopOfSelectedBlocks,
    )
    .with_custom_action(CustomAction::ScrollToTopOfSelectedBlocks)
    .with_context_predicate(
        id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
    )]);
    app.register_editable_bindings([EditableBinding::new(
        "terminal:scroll_to_bottom_of_selected_block",
        crate::t!("keybinding-desc-terminal-scroll-to-bottom-of-block"),
        TerminalAction::ScrollToBottomOfSelectedBlocks,
    )
    .with_custom_action(CustomAction::ScrollToBottomOfSelectedBlocks)
    .with_context_predicate(
        id!("Terminal") & ne!("TerminalView_BlockSelectionCardinality", "None"),
    )]);

    // Register a mac only keybinding for selecting all blocks that uses the "Select All" mac menu
    // item. We don't want this registered on Linux/Windows since this would mean the binding needs
    // to be "PTY compliant", which would end up making select all have a binding of `ctrl-shift-a`
    // instead of `ctrl-a` within the editor view.
    if OperatingSystem::get().is_mac() {
        app.register_editable_bindings([
            // Note that we register a separate action for SelectAll blocks
            // that always works, regardless of context - this one is triggered
            // from the menus and doesn't conflict with cmd-A in the editor.
            EditableBinding::new(
                "terminal:select_all_blocks",
                crate::t!("keybinding-desc-terminal-select-all-blocks"),
                TerminalAction::SelectAllBlocks,
            )
            .with_context_predicate(
                id!("Terminal") & !id!("IMEOpen") & id!("TerminalView_NonEmptyBlockList"),
            )
            .with_custom_action(CustomAction::SelectAll),
            EditableBinding::new(
                "terminal:select_all_blocks",
                crate::t!("keybinding-desc-terminal-select-all-blocks"),
                TerminalAction::SelectAllBlocks,
            )
            .with_context_predicate(
                id!("Terminal") & !id!("IMEOpen") & id!("TerminalView_NonEmptyBlockList"),
            )
            .with_custom_action(CustomAction::SelectAllBlocks),
        ]);
    } else {
        app.register_editable_bindings([EditableBinding::new(
            "terminal:select_all_blocks",
            crate::t!("keybinding-desc-terminal-select-all-blocks"),
            TerminalAction::SelectAllBlocks,
        )
        .with_context_predicate(
            id!("Terminal") & !id!("IMEOpen") & id!("TerminalView_NonEmptyBlockList"),
        )])
    }

    app.register_editable_bindings([
        EditableBinding::new(
            "terminal:expand_block_selection_above",
            crate::t!("keybinding-desc-terminal-expand-blocks-above"),
            TerminalAction::ExpandBlockSelectionAbove,
        )
        .with_key_binding("shift-up")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & !id!("ActiveBlockTextSelection")
                & !id!("AltScreen"),
        ),
        EditableBinding::new(
            "terminal:expand_block_selection_below",
            crate::t!("keybinding-desc-terminal-expand-blocks-below"),
            TerminalAction::ExpandBlockSelectionBelow,
        )
        .with_key_binding("shift-down")
        .with_context_predicate(
            id!("Terminal")
                & !id!("IMEOpen")
                & !id!("ActiveBlockTextSelection")
                & !id!("AltScreen"),
        ),
    ]);

    if FeatureFlag::CommandCorrectionKey.is_enabled() {
        app.register_editable_bindings([EditableBinding::new(
            "input:insert_command_correction",
            crate::t!("keybinding-desc-terminal-insert-command-correction"),
            TerminalAction::InsertMostRecentCommandCorrection,
        )
        .with_context_predicate(id!("Terminal"))]);
    }

    app.register_editable_bindings([EditableBinding::new(
        "workspace:open_settings_import_page",
        crate::t!("keybinding-desc-terminal-import-external-settings"),
        TerminalAction::ImportSettings,
    )
    .with_context_predicate(id!("Terminal") & id!(flags::HAS_SETTINGS_TO_IMPORT_FLAG))]);

    app.register_editable_bindings([EditableBinding::new(
        TOGGLE_BLOCK_FILTER_KEYBINDING,
        crate::t!("keybinding-desc-terminal-toggle-block-filter"),
        TerminalAction::ToggleBlockFilterOnSelectedOrLastBlock,
    )
    .with_mac_key_binding("shift-alt-F")
    .with_context_predicate(id!("Terminal") & !id!("IMEOpen") & !id!("AltScreen"))]);

    app.register_editable_bindings([EditableBinding::new(
        "terminal:toggle_snackbar_in_active_pane",
        crate::t!("keybinding-desc-terminal-toggle-sticky-command-header"),
        TerminalAction::ToggleSnackbarInActivePane,
    )
    .with_context_predicate(id!("Terminal"))]);

    app.register_editable_bindings([EditableBinding::new(
        "terminal:toggle_session_recording",
        crate::t!("keybinding-desc-terminal-toggle-session-recording"),
        TerminalAction::ToggleSessionRecording,
    )
    .with_enabled(|| cfg!(feature = "local_fs") && ChannelState::enable_debug_features())
    .with_context_predicate(id!("Terminal"))]);

    app.register_editable_bindings([EditableBinding::new(
        "workspace:add_current_dir_as_project",
        BindingDescription::new(crate::t!("keybinding-desc-workspace-add-current-folder")),
        TerminalAction::AddProjectAtCurrentDirectory,
    )
    .with_enabled(|| FeatureFlag::Projects.is_enabled())
    .with_context_predicate(id!("Workspace") & id!(flags::IS_ANY_AI_ENABLED))]);
}
