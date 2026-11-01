#[cfg(not(target_family = "wasm"))]
use crate::search::ai_context_menu::view::AIContextMenu;
#[cfg(not(target_family = "wasm"))]
use crate::settings::InputSettings;
use pathfinder_color::ColorU;
#[cfg(not(target_family = "wasm"))]
use settings::Setting as _;
use std::cell::RefCell;
use std::rc::Rc;
use warpui::SingletonEntity;
use warpui::{
    elements::{
        ChildView, Clipped, Container, CrossAxisAlignment, Flex, MainAxisAlignment, MainAxisSize,
        ParentElement,
    },
    AppContext, Element, Entity, TypedActionView, View, ViewContext, ViewHandle,
};

use warp_core::ui::appearance::Appearance;
use warp_core::ui::color::{coloru_with_opacity, Opacity};
use warp_core::ui::theme::{self, color::internal_colors};

#[cfg(not(target_family = "wasm"))]
use crate::terminal::model::session::SessionType;
use crate::{
    terminal::model::{block::BlockMetadata, session::Sessions},
    ui_components::icons::Icon,
    view_components::action_button::{
        ActionButton, ActionButtonTheme, ButtonSize, NakedTheme, TooltipAlignment,
    },
};

pub enum AtContextMenuDisabledReason {
    #[cfg(target_family = "wasm")]
    Wasm,
    #[cfg(not(target_family = "wasm"))]
    NoObjectsAvailable,
    #[cfg(not(target_family = "wasm"))]
    SshSession,
    #[cfg(not(target_family = "wasm"))]
    Subshell,
    #[cfg(not(target_family = "wasm"))]
    DisabledInTerminalMode,
}

impl AtContextMenuDisabledReason {
    fn tooltip_text(&self) -> String {
        match self {
            #[cfg(not(target_family = "wasm"))]
            AtContextMenuDisabledReason::NoObjectsAvailable => {
                "No available objects in the current context.".to_string()
            }
            #[cfg(not(target_family = "wasm"))]
            AtContextMenuDisabledReason::SshSession => "Not supported in SSH sessions".to_string(),
            #[cfg(not(target_family = "wasm"))]
            AtContextMenuDisabledReason::Subshell => "Not supported in subshells".to_string(),
            #[cfg(target_family = "wasm")]
            AtContextMenuDisabledReason::Wasm => "Requires a filesystem".to_string(),
            #[cfg(not(target_family = "wasm"))]
            AtContextMenuDisabledReason::DisabledInTerminalMode => {
                "Disabled in terminal mode, re-enable in settings".to_string()
            }
        }
    }

    #[cfg(target_family = "wasm")]
    pub fn get_disable_reason(
        _active_block_metadata: Option<&BlockMetadata>,
        _sessions: &Sessions,
        _ctx: &AppContext,
    ) -> Option<AtContextMenuDisabledReason> {
        Some(AtContextMenuDisabledReason::Wasm)
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn get_disable_reason(
        active_block_metadata: Option<&BlockMetadata>,
        sessions: &Sessions,
        ctx: &AppContext,
    ) -> Option<AtContextMenuDisabledReason> {
        // Derive session information from block metadata and sessions
        let (is_ssh_session, is_subshell) = active_block_metadata
            .and_then(|metadata| metadata.session_id())
            .and_then(|session_id| sessions.get(session_id))
            .map(|session| {
                let is_ssh_session = session.is_legacy_ssh_session()
                    || matches!(session.session_type(), SessionType::WarpifiedRemote { .. });
                let is_subshell = session.subshell_info().is_some();
                (is_ssh_session, is_subshell)
            })
            .unwrap_or((false, false));

        if !*InputSettings::as_ref(ctx)
            .at_context_menu_in_terminal_mode
            .value()
        {
            return Some(AtContextMenuDisabledReason::DisabledInTerminalMode);
        }

        if is_ssh_session {
            return Some(AtContextMenuDisabledReason::SshSession);
        }
        if is_subshell {
            return Some(AtContextMenuDisabledReason::Subshell);
        }

        // Allow @ context menu outside of git repositories, when we have categories available.
        // Repo-based restrictions will be enforced at the category level
        // (e.g., Code will only be available inside a git repository).
        if AIContextMenu::get_categories_for_mode(false, ctx).is_empty() {
            return Some(AtContextMenuDisabledReason::NoObjectsAvailable);
        }

        None
    }
}

const AT_CONTEXT_TOOLTIP: &str = "Attach context";

const BLURRED_OPACITY: Opacity = 50;

/// Shared theme for icon-only input buttons.
#[derive(Clone)]
pub struct PromptIconButtonTheme {
    is_blurred: bool,
}

impl PromptIconButtonTheme {
    pub fn new(is_blurred: bool) -> Self {
        Self { is_blurred }
    }
}

impl ActionButtonTheme for PromptIconButtonTheme {
    fn background(&self, hovered: bool, appearance: &Appearance) -> Option<theme::Fill> {
        NakedTheme.background(hovered, appearance)
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<theme::Fill>,
        appearance: &Appearance,
    ) -> ColorU {
        let color = appearance
            .theme()
            .sub_text_color(appearance.theme().surface_1())
            .into_solid();
        if self.is_blurred {
            coloru_with_opacity(color, BLURRED_OPACITY)
        } else {
            color
        }
    }
}

/// Custom disabled theme for UDI buttons that preserves background but changes font color
struct UDIDisabledButtonTheme;

impl ActionButtonTheme for UDIDisabledButtonTheme {
    fn background(&self, hovered: bool, appearance: &Appearance) -> Option<theme::Fill> {
        // Use the same background as the enabled state
        NakedTheme.background(hovered, appearance)
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<theme::Fill>,
        appearance: &Appearance,
    ) -> ColorU {
        internal_colors::text_disabled(appearance.theme(), appearance.theme().surface_1())
    }
}

/// Denormalized state that is required to render UDI styles that isn't easily directly accessible
/// because its owned by other views in other parts of the hierarchy.
struct CachedUIState {
    is_hovered: bool,
    is_in_active_terminal: bool,
}

impl CachedUIState {
    fn is_button_bar_blurred(&self) -> bool {
        !self.is_hovered && !self.is_in_active_terminal
    }
}

/// Button row under the universal developer input: slash commands and @ context.
pub struct UniversalDeveloperInputButtonBar {
    at_button: ViewHandle<ActionButton>,
    slash_command_button: ViewHandle<ActionButton>,
    cached_ui_state: Rc<RefCell<CachedUIState>>,
}

#[derive(Debug, Clone)]
pub enum UniversalDeveloperInputButtonBarAction {
    SetAIContextMenuOpen(bool),
    OpenSlashCommandMenu,
}

pub enum UniversalDeveloperInputButtonBarEvent {
    SetAIContextMenuOpen(bool),
    OpenSlashCommandMenu,
}

impl UniversalDeveloperInputButtonBar {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let button_size = ButtonSize::UDIButton;

        let at_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("", PromptIconButtonTheme::new(false))
                .with_icon(Icon::AtSign)
                .with_tooltip(AT_CONTEXT_TOOLTIP)
                .with_size(button_size)
                .with_disabled_theme(UDIDisabledButtonTheme)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(
                        UniversalDeveloperInputButtonBarAction::SetAIContextMenuOpen(true),
                    );
                })
        });

        let slash_command_button = ctx.add_typed_action_view(|_ctx| {
            ActionButton::new("", PromptIconButtonTheme::new(false))
                .with_icon(Icon::SlashCommands)
                .with_tooltip(crate::t!("terminal-slash-commands-tooltip"))
                .with_size(button_size)
                .with_disabled_theme(UDIDisabledButtonTheme)
                .with_tooltip_alignment(TooltipAlignment::Left)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(
                        UniversalDeveloperInputButtonBarAction::OpenSlashCommandMenu,
                    );
                })
        });

        let me = Self {
            at_button,
            slash_command_button,
            cached_ui_state: Rc::new(RefCell::new(CachedUIState {
                is_hovered: false,
                is_in_active_terminal: false,
            })),
        };
        me.update_icon_button_themes(ctx);
        me
    }

    /// Update the at button's disabled state based on whether AI context menu should render
    pub fn set_at_button_disabled(
        &mut self,
        disable_reason: Option<AtContextMenuDisabledReason>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.at_button.update(ctx, |button, ctx| {
            button.set_disabled(disable_reason.is_some(), ctx);
            button.set_tooltip(
                disable_reason
                    .map(|reason| reason.tooltip_text())
                    .or(Some(AT_CONTEXT_TOOLTIP.to_string())),
                ctx,
            );
            ctx.notify();
        });
    }

    /// Update the slash button's disabled state based on whether the buffer is empty.
    pub fn set_slash_button_disabled(&mut self, should_disable: bool, ctx: &mut ViewContext<Self>) {
        self.slash_command_button.update(ctx, |button, ctx| {
            button.set_disabled(should_disable, ctx);
            ctx.notify();
        });
    }

    pub fn set_is_in_active_terminal(
        &mut self,
        is_in_active_terminal: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.cached_ui_state.borrow().is_in_active_terminal == is_in_active_terminal {
            return;
        }

        self.cached_ui_state.borrow_mut().is_in_active_terminal = is_in_active_terminal;
        self.update_icon_button_themes(ctx);
        ctx.notify();
    }

    /// Set the parent UDI input hovered state.
    /// Used to fade the button bar when the input is not in focus.
    pub fn set_udi_hovered(&mut self, is_hovered: bool, ctx: &mut ViewContext<Self>) {
        if self.cached_ui_state.borrow().is_hovered == is_hovered {
            return;
        }
        self.cached_ui_state.borrow_mut().is_hovered = is_hovered;

        self.update_icon_button_themes(ctx);
        ctx.notify();
    }

    /// Update the themes of the icon buttons to reflect the blurred state
    fn update_icon_button_themes(&self, ctx: &mut ViewContext<Self>) {
        let is_blurred = self.cached_ui_state.borrow().is_button_bar_blurred();
        let theme = PromptIconButtonTheme::new(is_blurred);

        self.at_button.update(ctx, |button, ctx| {
            button.set_theme(theme.clone(), ctx);
        });

        self.slash_command_button.update(ctx, |button, ctx| {
            button.set_theme(theme.clone(), ctx);
        });
    }
}

impl Entity for UniversalDeveloperInputButtonBar {
    type Event = UniversalDeveloperInputButtonBarEvent;
}

impl View for UniversalDeveloperInputButtonBar {
    fn ui_name() -> &'static str {
        "UniversalDeveloperInputButtonBar"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        let buttons = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_alignment(MainAxisAlignment::Start)
            .with_child(ChildView::new(&self.slash_command_button).finish())
            .with_child(ChildView::new(&self.at_button).finish())
            .finish();

        Container::new(Clipped::new(buttons).finish())
            .with_padding_bottom(12.0)
            .with_padding_right(8.0)
            .finish()
    }
}

impl TypedActionView for UniversalDeveloperInputButtonBar {
    type Action = UniversalDeveloperInputButtonBarAction;

    fn handle_action(
        &mut self,
        action: &UniversalDeveloperInputButtonBarAction,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            UniversalDeveloperInputButtonBarAction::SetAIContextMenuOpen(open) => {
                ctx.emit(UniversalDeveloperInputButtonBarEvent::SetAIContextMenuOpen(
                    *open,
                ));
            }
            UniversalDeveloperInputButtonBarAction::OpenSlashCommandMenu => {
                ctx.emit(UniversalDeveloperInputButtonBarEvent::OpenSlashCommandMenu);
            }
        }
    }
}
