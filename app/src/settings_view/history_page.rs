use settings::Setting as _;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use chrono::{DateTime, Local, Utc};
use futures::channel::oneshot;
use warpui::{
    clipboard::ClipboardContent,
    elements::{
        Border, Container, CornerRadius, CrossAxisAlignment, Expanded, Flex, MainAxisSize,
        MouseStateHandle, ParentElement, Radius,
    },
    fonts::Weight,
    ui_components::{
        button::ButtonVariant,
        components::{Coords, UiComponent, UiComponentStyles},
    },
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use super::{
    settings_page::{
        render_page_title, MatchData, PageType, SettingsPageEvent, SettingsPageMeta,
        SettingsPageViewHandle, SettingsWidget, HEADER_FONT_SIZE,
    },
    SettingsSection,
};
use crate::{
    appearance::Appearance,
    persistence::{model::Command, ModelEvent, PersistenceWriter},
    settings::InputSettings,
    terminal::history::{History, ShellHost},
};

const COMMANDS_PER_PAGE: usize = 100;

// An identical path on a different host or account is a separate history folder.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct HistoryFolder {
    host: Option<String>,
    user: Option<String>,
    path: Option<String>,
}

impl HistoryFolder {
    fn for_record(record: &Command) -> Self {
        Self {
            host: record.hostname.clone(),
            user: record.username.clone(),
            path: record.pwd.clone(),
        }
    }

    fn matches_record(&self, record: &Command) -> bool {
        self.path == record.pwd && self.host == record.hostname && self.user == record.username
    }

    fn label(&self) -> String {
        format!(
            "{}@{}: {}",
            self.user.as_deref().unwrap_or("Unknown user"),
            self.host.as_deref().unwrap_or("Unknown host"),
            self.path.as_deref().unwrap_or("Unknown folder")
        )
    }
}

#[derive(Clone, Debug)]
pub enum HistoryPageAction {
    Select(usize),
    ToggleCommand(i32),
    SelectAllCommands,
    DeselectAllCommands,
    DeleteCommand(i32),
    DeleteSelectedCommands,
    ToggleShellHistory,
    RequestLiveClear,
    ConfirmLiveClear,
    CancelLiveClear,
    ToggleRestoredHistory,
    RequestShellClear(usize),
    ConfirmShellClear,
    CancelShellClear,
    Refresh,
    Back,
    RequestClear,
    ClearFolder(usize),
    ConfirmClear,
    CancelClear,
    Next,
    Previous,
    Copy(i32),
}

pub struct HistoryPageView {
    page: PageType<Self>,
    shell_toggle: MouseStateHandle,
    live_clear: MouseStateHandle,
    live_confirm: MouseStateHandle,
    live_cancel: MouseStateHandle,
    pending_live_clear: bool,
    restored_toggle: MouseStateHandle,
    shell_files: Vec<(PathBuf, Vec<ShellHost>, MouseStateHandle)>,
    pending_shell_clear: Option<(PathBuf, Vec<ShellHost>)>,
    shell_confirm: MouseStateHandle,
    shell_cancel: MouseStateHandle,
    message: Option<String>,
    records: Vec<Command>,
    folders: Vec<(HistoryFolder, usize, MouseStateHandle, MouseStateHandle)>,
    selected: Option<HistoryFolder>,
    offset: usize,
    folder_filter: String,
    back: MouseStateHandle,
    pending_clear: Option<Vec<Command>>,
    busy: bool,
    error: Option<String>,
    refresh: MouseStateHandle,
    clear: MouseStateHandle,
    confirm: MouseStateHandle,
    cancel: MouseStateHandle,
    next: MouseStateHandle,
    previous: MouseStateHandle,
    copy_states: Vec<MouseStateHandle>,
    command_select_states: Vec<MouseStateHandle>,
    command_delete_states: Vec<MouseStateHandle>,
    selected_commands: HashSet<i32>,
    select_all: MouseStateHandle,
    deselect_all: MouseStateHandle,
    delete_selected: MouseStateHandle,
}

impl HistoryPageView {
    pub fn new() -> Self {
        Self {
            page: PageType::new_monolith(HistoryWidget, None, true),
            shell_toggle: Default::default(),
            live_clear: Default::default(),
            live_confirm: Default::default(),
            live_cancel: Default::default(),
            pending_live_clear: false,
            restored_toggle: Default::default(),
            shell_files: Vec::new(),
            pending_shell_clear: None,
            shell_confirm: Default::default(),
            shell_cancel: Default::default(),
            message: None,
            records: Vec::new(),
            folders: Vec::new(),
            selected: None,
            offset: 0,
            folder_filter: String::new(),
            back: Default::default(),
            pending_clear: None,
            busy: false,
            error: None,
            refresh: Default::default(),
            clear: Default::default(),
            confirm: Default::default(),
            cancel: Default::default(),
            next: Default::default(),
            previous: Default::default(),
            command_select_states: (0..COMMANDS_PER_PAGE).map(|_| Default::default()).collect(),
            command_delete_states: (0..COMMANDS_PER_PAGE).map(|_| Default::default()).collect(),
            selected_commands: HashSet::new(),
            select_all: Default::default(),
            deselect_all: Default::default(),
            delete_selected: Default::default(),
            copy_states: (0..COMMANDS_PER_PAGE)
                .map(|_| MouseStateHandle::default())
                .collect(),
        }
    }

    fn request(&mut self, records: Option<Vec<Command>>, ctx: &mut ViewContext<Self>) {
        let Some(sender) = PersistenceWriter::as_ref(ctx).sender() else {
            self.error =
                Some("Saved history is unavailable because local persistence is disabled.".into());
            ctx.notify();
            return;
        };
        let deleting = records.is_some();
        let (reply, receiver) = oneshot::channel();
        let event = match records {
            Some(records) => ModelEvent::DeleteCommandHistory { records, reply },
            None => ModelEvent::ReadCommandHistory { reply },
        };
        self.busy = true;
        self.error = None;
        ctx.spawn(
            async move {
                sender
                    .send(event)
                    .map_err(|_| "History writer is unavailable.".to_string())?;
                receiver
                    .await
                    .map_err(|_| "History writer disconnected.".to_string())?
            },
            move |view, result, ctx| {
                view.busy = false;
                match result {
                    Ok(records) if deleting => {
                        History::handle(ctx).update(ctx, |history, ctx| {
                            history.forget_saved_records(&records, ctx)
                        });
                        view.load(ctx);
                    }
                    Ok(records) => {
                        History::handle(ctx).update(ctx, |history, ctx| {
                            history.refresh_saved_records(
                                records.iter().cloned().map(Into::into).collect(),
                                ctx,
                            );
                        });
                        view.set_records(records);
                    }
                    Err(error) => view.error = Some(error),
                }
                ctx.notify();
            },
        );
        ctx.notify();
    }

    fn load(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.busy {
            let mut files: BTreeMap<PathBuf, Vec<ShellHost>> = BTreeMap::new();
            for (path, host) in History::as_ref(ctx).local_history_files() {
                files.entry(path).or_default().push(host);
            }
            self.shell_files = files
                .into_iter()
                .map(|(path, hosts)| (path, hosts, Default::default()))
                .collect();
            self.pending_clear = None;
            self.request(None, ctx);
        }
    }

    fn set_records(&mut self, records: Vec<Command>) {
        let mut folders = BTreeMap::new();
        for record in &records {
            *folders
                .entry(HistoryFolder::for_record(record))
                .or_insert(0) += 1;
        }
        self.folders = folders
            .into_iter()
            .map(|(folder, count)| {
                (
                    folder,
                    count,
                    MouseStateHandle::default(),
                    MouseStateHandle::default(),
                )
            })
            .collect();
        if self.selected.as_ref().is_some_and(|folder| {
            !self
                .folders
                .iter()
                .any(|(candidate, _, _, _)| candidate == folder)
        }) {
            self.selected = None;
        }
        self.selected_commands.clear();
        self.records = records;
        self.offset = 0;
        self.page.reset_scroll();
    }

    fn selected_records(&self) -> Vec<&Command> {
        self.records
            .iter()
            .filter(|record| {
                self.selected
                    .as_ref()
                    .is_some_and(|folder| folder.matches_record(record))
            })
            .collect()
    }
}

impl TypedActionView for HistoryPageView {
    type Action = HistoryPageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        if self.busy {
            return;
        }
        match action {
            HistoryPageAction::ToggleCommand(id) => {
                if self
                    .selected_records()
                    .iter()
                    .any(|record| record.id == *id)
                {
                    if !self.selected_commands.remove(id) {
                        self.selected_commands.insert(*id);
                    }
                    self.pending_clear = None;
                }
            }
            HistoryPageAction::SelectAllCommands => {
                self.selected_commands = self
                    .selected_records()
                    .iter()
                    .map(|record| record.id)
                    .collect();
                self.pending_clear = None;
            }
            HistoryPageAction::DeselectAllCommands => {
                self.selected_commands.clear();
                self.pending_clear = None;
            }
            HistoryPageAction::DeleteCommand(id) => {
                let records: Vec<_> = self
                    .selected_records()
                    .into_iter()
                    .filter(|record| record.id == *id)
                    .cloned()
                    .collect();
                if !records.is_empty() {
                    self.pending_clear = Some(records);
                }
            }
            HistoryPageAction::DeleteSelectedCommands => {
                let records: Vec<_> = self
                    .selected_records()
                    .into_iter()
                    .filter(|record| self.selected_commands.contains(&record.id))
                    .cloned()
                    .collect();
                if !records.is_empty() {
                    self.pending_clear = Some(records);
                }
            }
            HistoryPageAction::RequestLiveClear => self.pending_live_clear = true,
            HistoryPageAction::CancelLiveClear => self.pending_live_clear = false,
            HistoryPageAction::ConfirmLiveClear => {
                if self.pending_live_clear {
                    self.pending_live_clear = false;
                    History::handle(ctx)
                        .update(ctx, |history, ctx| history.clear_live_history(ctx));
                    self.message = Some("Live and restored commands removed from the popup for this app run. Saved records and shell files are separate. Restored blocks may return after restart unless Restored commands is Off.".into());
                }
            }
            HistoryPageAction::ToggleShellHistory => {
                let result = InputSettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings
                        .use_shell_history
                        .set_value(!*settings.use_shell_history, ctx)
                });
                if let Err(error) = result {
                    self.error = Some(error.to_string());
                }
            }
            HistoryPageAction::ToggleRestoredHistory => {
                let result = InputSettings::handle(ctx).update(ctx, |settings, ctx| {
                    settings
                        .use_restored_history
                        .set_value(!*settings.use_restored_history, ctx)
                });
                if let Err(error) = result {
                    self.error = Some(error.to_string());
                }
            }
            HistoryPageAction::RequestShellClear(index) => {
                if let Some((path, hosts, _)) = self.shell_files.get(*index) {
                    self.pending_shell_clear = Some((path.clone(), hosts.clone()));
                }
            }
            HistoryPageAction::CancelShellClear => self.pending_shell_clear = None,
            HistoryPageAction::ConfirmShellClear => {
                if let Some((path, hosts)) = self.pending_shell_clear.take() {
                    self.busy = true;
                    self.error = None;
                    self.message = None;
                    ctx.spawn(async move { clear_shell_history_file(&path).await }, move |view, result, ctx| {
                        view.busy = false;
                        match result {
                            Ok(()) => {
                                History::handle(ctx).update(ctx, |history, ctx| history.forget_shell_history(hosts, ctx));
                                view.message = Some("Shell history file cleared. Open shells may write their in-memory history back when they exit.".into());
                            }
                            Err(error) => view.error = Some(format!("Could not clear shell history: {error}")),
                        }
                        ctx.notify();
                    });
                }
            }
            HistoryPageAction::Select(index) => {
                if let Some((folder, _, _, _)) = self.folders.get(*index) {
                    self.selected_commands.clear();
                    self.selected = Some(folder.clone());
                    self.offset = 0;
                    self.pending_clear = None;
                }
            }
            HistoryPageAction::Refresh => self.load(ctx),
            HistoryPageAction::Back => {
                self.selected_commands.clear();
                self.selected = None;
                self.pending_clear = None;
            }
            HistoryPageAction::ClearFolder(index) => {
                if let Some((folder, _, _, _)) = self.folders.get(*index) {
                    self.selected_commands.clear();
                    self.selected = Some(folder.clone());
                    self.offset = 0;
                    self.pending_clear =
                        Some(self.selected_records().into_iter().cloned().collect());
                }
            }
            HistoryPageAction::RequestClear => {
                self.pending_clear = Some(
                    self.selected_records()
                        .iter()
                        .map(|record| (*record).clone())
                        .collect(),
                );
            }
            HistoryPageAction::ConfirmClear => {
                if let Some(records) = self.pending_clear.take() {
                    self.request(Some(records), ctx);
                }
            }
            HistoryPageAction::CancelClear => self.pending_clear = None,
            HistoryPageAction::Next => {
                if self.offset + COMMANDS_PER_PAGE < self.selected_records().len() {
                    self.offset += COMMANDS_PER_PAGE;
                }
            }
            HistoryPageAction::Previous => {
                self.offset = self.offset.saturating_sub(COMMANDS_PER_PAGE)
            }
            HistoryPageAction::Copy(id) => {
                if let Some(record) = self.records.iter().find(|record| record.id == *id) {
                    ctx.clipboard()
                        .write(ClipboardContent::plain_text(record.command.clone()));
                }
            }
        }
        if matches!(
            action,
            HistoryPageAction::Select(..)
                | HistoryPageAction::ClearFolder(..)
                | HistoryPageAction::Back
                | HistoryPageAction::Next
                | HistoryPageAction::Previous
        ) {
            self.page.reset_scroll();
        }
        ctx.notify();
    }
}

impl Entity for HistoryPageView {
    type Event = SettingsPageEvent;
}
impl View for HistoryPageView {
    fn ui_name() -> &'static str {
        "HistoryPage"
    }
    fn render(&self, ctx: &AppContext) -> Box<dyn Element> {
        self.page.render(self, ctx)
    }
}

async fn clear_shell_history_file(path: &Path) -> std::io::Result<()> {
    // Open an existing file only. Do not replace it or change its permissions.
    let file = async_fs::OpenOptions::new().write(true).open(path).await?;
    if !file.metadata().await?.is_file() {
        return Err(std::io::Error::other("History path is not a regular file"));
    }
    file.set_len(0).await
}

struct HistoryWidget;
impl SettingsWidget for HistoryWidget {
    type View = HistoryPageView;
    fn search_terms(&self) -> &str {
        "command history commands history saved folders directory view clear delete cleanup shell import restored HISTFILE"
    }
    fn render(
        &self,
        view: &HistoryPageView,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let ui = appearance.ui_builder();
        let mut body = Flex::column().with_spacing(16.)
            .with_child(
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(Expanded::new(1., render_page_title("Command history", HEADER_FONT_SIZE, appearance)).finish())
                    .with_child(history_button("Refresh", &view.refresh, HistoryPageAction::Refresh, view.busy, appearance))
                    .finish()
            )
            .with_child(ui.span("Manage saved folder history, shell imports, and commands from open or restored tabs.").with_soft_wrap().build().finish());
        let settings = InputSettings::as_ref(ctx);
        let mut sources = Flex::column().with_spacing(12.)
            .with_child(ui.span("History sources").with_style(UiComponentStyles { font_weight: Some(Weight::Semibold), ..Default::default() }).build().finish())
            .with_child(history_caption("The popup combines saved records, shell imports, live commands, and restored blocks.", appearance))
            .with_child(history_button(if *settings.use_shell_history { "Use shell history: On" } else { "Use shell history: Off" }, &view.shell_toggle, HistoryPageAction::ToggleShellHistory, view.busy, appearance))
            .with_child(history_caption("Show imported shell commands. Turning this off keeps the shell file, saved dWarp records, and new commands.", appearance))
            .with_child(history_button(if *settings.use_restored_history { "Restored commands: On" } else { "Restored commands: Off" }, &view.restored_toggle, HistoryPageAction::ToggleRestoredHistory, view.busy, appearance))
            .with_child(history_caption("Show commands from restored terminal blocks. Terminal scrollback is kept.", appearance));
        sources.add_child(history_button(
            "Clear live popup history…",
            &view.live_clear,
            HistoryPageAction::RequestLiveClear,
            view.busy,
            appearance,
        ));
        sources.add_child(history_caption("Hide commands held by open tabs until restart. New commands still appear. Saved records, shell files, and scrollback are kept.", appearance));
        if view.pending_live_clear {
            sources.add_child(history_caption("Remove current live and restored commands from popup history across all open tabs?", appearance));
            sources.add_child(
                Flex::row()
                    .with_spacing(12.)
                    .with_child(history_button(
                        "Clear live history",
                        &view.live_confirm,
                        HistoryPageAction::ConfirmLiveClear,
                        view.busy,
                        appearance,
                    ))
                    .with_child(history_button(
                        "Cancel",
                        &view.live_cancel,
                        HistoryPageAction::CancelLiveClear,
                        view.busy,
                        appearance,
                    ))
                    .finish(),
            );
        }
        sources.add_child(
            ui.span("Local shell history files")
                .with_style(UiComponentStyles {
                    font_weight: Some(Weight::Semibold),
                    ..Default::default()
                })
                .build()
                .finish(),
        );
        sources.add_child(history_caption("Files detected from local sessions, including HISTFILE overrides. Remote files are excluded. Clearing affects all folders using the file. Open shells may write history back on exit.", appearance));
        if view.shell_files.is_empty() {
            sources.add_child(history_caption(
                "No local shell history files detected. Open a local terminal, then Refresh.",
                appearance,
            ));
        }
        for (index, (path, _, state)) in view.shell_files.iter().enumerate() {
            sources.add_child(
                ui.span(path.display().to_string())
                    .with_soft_wrap()
                    .build()
                    .finish(),
            );
            sources.add_child(history_button(
                "Clear shell history file…",
                state,
                HistoryPageAction::RequestShellClear(index),
                view.busy,
                appearance,
            ));
        }
        if let Some((path, _)) = &view.pending_shell_clear {
            sources.add_child(history_caption(format!("Permanently empty {}? This clears shell history across all folders using this file. dWarp database records and live session commands are separate.", path.display()), appearance));
            sources.add_child(
                Flex::row()
                    .with_spacing(12.)
                    .with_child(history_button(
                        "Clear file",
                        &view.shell_confirm,
                        HistoryPageAction::ConfirmShellClear,
                        view.busy,
                        appearance,
                    ))
                    .with_child(history_button(
                        "Cancel",
                        &view.shell_cancel,
                        HistoryPageAction::CancelShellClear,
                        view.busy,
                        appearance,
                    ))
                    .finish(),
            );
        }
        body.add_child(history_panel(sources.finish(), appearance));
        if let Some(message) = &view.message {
            body.add_child(history_caption(message, appearance));
        }
        if view.busy {
            body.add_child(ui.span("Loading…").build().finish());
        }
        if let Some(error) = &view.error {
            body.add_child(history_panel(
                ui.span(error.clone()).with_soft_wrap().build().finish(),
                appearance,
            ));
        }
        if view.folders.is_empty() && !view.busy && view.error.is_none() {
            body.add_child(history_panel(
                ui.span("No saved command history.").build().finish(),
                appearance,
            ));
        }
        if view.selected.is_none() && !view.folders.is_empty() {
            let visible = view
                .folders
                .iter()
                .filter(|(folder, _, _, _)| {
                    folder.label().to_lowercase().contains(&view.folder_filter)
                })
                .count();
            body.add_child(
                ui.span(format!(
                    "Folders: {visible} · Saved commands: {}",
                    view.records.len()
                ))
                .build()
                .finish(),
            );
            let mut folders = Flex::column().with_spacing(8.);
            for (index, (folder, count, view_state, clear_state)) in view.folders.iter().enumerate()
            {
                if !folder.label().to_lowercase().contains(&view.folder_filter) {
                    continue;
                }
                let details = Flex::column()
                    .with_spacing(6.)
                    .with_child(
                        ui.span(
                            folder
                                .path
                                .clone()
                                .unwrap_or_else(|| "Unknown folder".into()),
                        )
                        .with_style(UiComponentStyles {
                            font_weight: Some(Weight::Semibold),
                            ..Default::default()
                        })
                        .with_soft_wrap()
                        .build()
                        .finish(),
                    )
                    .with_child(history_caption(
                        format!(
                            "{}@{} · Saved commands: {count}",
                            folder.user.as_deref().unwrap_or("Unknown user"),
                            folder.host.as_deref().unwrap_or("Unknown host")
                        ),
                        appearance,
                    ));
                let actions = Flex::row()
                    .with_spacing(8.)
                    .with_child(history_button(
                        "View",
                        view_state,
                        HistoryPageAction::Select(index),
                        view.busy,
                        appearance,
                    ))
                    .with_child(history_button(
                        "Clear history…",
                        clear_state,
                        HistoryPageAction::ClearFolder(index),
                        view.busy,
                        appearance,
                    ));
                folders.add_child(history_panel(
                    Flex::row()
                        .with_main_axis_size(MainAxisSize::Max)
                        .with_cross_axis_alignment(CrossAxisAlignment::Center)
                        .with_spacing(16.)
                        .with_child(Expanded::new(1., details.finish()).finish())
                        .with_child(actions.finish())
                        .finish(),
                    appearance,
                ));
            }
            body.add_child(folders.finish());
        }
        if let Some(folder) = &view.selected {
            let records = view.selected_records();
            body.add_child(
                Flex::row()
                    .with_spacing(8.)
                    .with_child(history_button(
                        "Back to folders",
                        &view.back,
                        HistoryPageAction::Back,
                        view.busy,
                        appearance,
                    ))
                    .finish(),
            );
            let mut folder_header = Flex::column()
                .with_spacing(12.)
                .with_child(
                    ui.span(
                        folder
                            .path
                            .clone()
                            .unwrap_or_else(|| "Unknown folder".into()),
                    )
                    .with_soft_wrap()
                    .build()
                    .finish(),
                )
                .with_child(history_caption(
                    format!(
                        "{}@{} · Saved commands: {}",
                        folder.user.as_deref().unwrap_or("Unknown user"),
                        folder.host.as_deref().unwrap_or("Unknown host"),
                        records.len()
                    ),
                    appearance,
                ));
            if let Some(records) = &view.pending_clear {
                folder_header.add_child(
                    ui.span(format!(
                        "Delete {} saved commands from this folder? This cannot be undone.",
                        records.len()
                    ))
                    .with_soft_wrap()
                    .build()
                    .finish(),
                );
                folder_header.add_child(
                    Flex::row()
                        .with_spacing(8.)
                        .with_child(history_button(
                            "Delete commands",
                            &view.confirm,
                            HistoryPageAction::ConfirmClear,
                            view.busy,
                            appearance,
                        ))
                        .with_child(history_button(
                            "Cancel",
                            &view.cancel,
                            HistoryPageAction::CancelClear,
                            view.busy,
                            appearance,
                        ))
                        .finish(),
                );
            } else {
                folder_header.add_child(
                    Flex::row()
                        .with_child(history_button(
                            "Clear history…",
                            &view.clear,
                            HistoryPageAction::RequestClear,
                            view.busy,
                            appearance,
                        ))
                        .finish(),
                );
            }
            body.add_child(history_panel(folder_header.finish(), appearance));
            body.add_child(
                Flex::column()
                    .with_spacing(8.)
                    .with_child(
                        Flex::row()
                            .with_spacing(8.)
                            .with_child(history_button(
                                "Select all",
                                &view.select_all,
                                HistoryPageAction::SelectAllCommands,
                                view.busy || view.selected_commands.len() == records.len(),
                                appearance,
                            ))
                            .with_child(history_button(
                                "Deselect all",
                                &view.deselect_all,
                                HistoryPageAction::DeselectAllCommands,
                                view.busy || view.selected_commands.is_empty(),
                                appearance,
                            ))
                            .finish(),
                    )
                    .with_child(history_caption(
                        format!(
                            "{} selected · Selection covers this folder across all pages",
                            view.selected_commands.len()
                        ),
                        appearance,
                    ))
                    .with_child(history_button(
                        "Delete selected…",
                        &view.delete_selected,
                        HistoryPageAction::DeleteSelectedCommands,
                        view.busy || view.selected_commands.is_empty(),
                        appearance,
                    ))
                    .finish(),
            );
            let mut navigation = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(8.)
                .with_child(
                    Expanded::new(
                        1.,
                        history_caption(
                            format!(
                                "{}–{} of {} · Newest first",
                                view.offset + 1,
                                (view.offset + COMMANDS_PER_PAGE).min(records.len()),
                                records.len()
                            ),
                            appearance,
                        ),
                    )
                    .finish(),
                );
            if records.len() > COMMANDS_PER_PAGE {
                navigation.add_child(history_button(
                    "Previous",
                    &view.previous,
                    HistoryPageAction::Previous,
                    view.busy || view.offset == 0,
                    appearance,
                ));
                navigation.add_child(history_button(
                    "Next",
                    &view.next,
                    HistoryPageAction::Next,
                    view.busy || view.offset + COMMANDS_PER_PAGE >= records.len(),
                    appearance,
                ));
            }
            body.add_child(navigation.finish());
            let mut commands = Flex::column().with_spacing(8.);
            for (index, record) in records
                .into_iter()
                .skip(view.offset)
                .take(COMMANDS_PER_PAGE)
                .enumerate()
            {
                let time = record
                    .start_ts
                    .map(|ts| {
                        DateTime::<Utc>::from_naive_utc_and_offset(ts, Utc)
                            .with_timezone(&Local)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                    })
                    .unwrap_or_else(|| "Unknown time".into());
                let exit = record
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "Unknown".into());
                let id = record.id;
                let mut checkbox = ui
                    .checkbox(view.command_select_states[index].clone(), None)
                    .check(view.selected_commands.contains(&id))
                    .with_label(ui.span("Select"))
                    .with_style(
                        UiComponentStyles::default()
                            .set_background(appearance.theme().outline().into()),
                    );
                if view.busy {
                    checkbox = checkbox.disabled();
                }
                let actions = Flex::row()
                    .with_spacing(8.)
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(
                        checkbox
                            .build()
                            .on_click(move |ctx, _, _| {
                                ctx.dispatch_typed_action(HistoryPageAction::ToggleCommand(id))
                            })
                            .finish(),
                    )
                    .with_child(Expanded::new(1., Flex::row().finish()).finish())
                    .with_child(history_button(
                        "Copy",
                        &view.copy_states[index],
                        HistoryPageAction::Copy(id),
                        view.busy,
                        appearance,
                    ))
                    .with_child(history_button(
                        "Delete…",
                        &view.command_delete_states[index],
                        HistoryPageAction::DeleteCommand(id),
                        view.busy,
                        appearance,
                    ));
                commands.add_child(history_panel(
                    Flex::column()
                        .with_spacing(8.)
                        .with_child(history_caption(
                            format!("{time} · Exit: {exit}"),
                            appearance,
                        ))
                        .with_child(
                            ui.span(record.command.clone())
                                .with_style(UiComponentStyles {
                                    font_family_id: Some(appearance.monospace_font_family()),
                                    ..Default::default()
                                })
                                .with_soft_wrap()
                                .build()
                                .finish(),
                        )
                        .with_child(actions.finish())
                        .finish(),
                    appearance,
                ));
            }
            body.add_child(commands.finish());
        }
        body.finish()
    }
}

fn history_panel(content: Box<dyn Element>, appearance: &Appearance) -> Box<dyn Element> {
    Container::new(content)
        .with_uniform_padding(16.)
        .with_background(appearance.theme().surface_1())
        .with_border(Border::all(1.).with_border_fill(appearance.theme().outline()))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish()
}

fn history_caption(text: impl Into<String>, appearance: &Appearance) -> Box<dyn Element> {
    appearance
        .ui_builder()
        .span(text.into())
        .with_style(UiComponentStyles {
            font_size: Some(12.),
            font_color: Some(
                appearance
                    .theme()
                    .sub_text_color(appearance.theme().surface_1())
                    .into_solid(),
            ),
            ..Default::default()
        })
        .with_soft_wrap()
        .build()
        .finish()
}

fn history_button(
    label: impl Into<String>,
    state: &MouseStateHandle,
    action: HistoryPageAction,
    disabled: bool,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let variant = match action {
        HistoryPageAction::ConfirmClear
        | HistoryPageAction::ConfirmShellClear
        | HistoryPageAction::ConfirmLiveClear => ButtonVariant::Error,
        HistoryPageAction::Select(..) => ButtonVariant::Accent,
        HistoryPageAction::ToggleCommand(..)
        | HistoryPageAction::SelectAllCommands
        | HistoryPageAction::DeselectAllCommands
        | HistoryPageAction::DeleteCommand(..)
        | HistoryPageAction::DeleteSelectedCommands
        | HistoryPageAction::RequestLiveClear
        | HistoryPageAction::CancelLiveClear
        | HistoryPageAction::ToggleShellHistory
        | HistoryPageAction::ToggleRestoredHistory
        | HistoryPageAction::RequestShellClear(..)
        | HistoryPageAction::CancelShellClear
        | HistoryPageAction::ClearFolder(..)
        | HistoryPageAction::RequestClear
        | HistoryPageAction::Refresh
        | HistoryPageAction::Back
        | HistoryPageAction::CancelClear
        | HistoryPageAction::Next
        | HistoryPageAction::Previous
        | HistoryPageAction::Copy(..) => ButtonVariant::Basic,
    };
    let mut button = appearance
        .ui_builder()
        .button(variant, state.clone())
        .with_text_label(label.into())
        .with_style(
            UiComponentStyles::default()
                .set_padding(Coords::default().top(8.).bottom(8.).left(12.).right(12.)),
        );
    if disabled {
        button = button.disabled();
    }
    button
        .build()
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
}

impl SettingsPageMeta for HistoryPageView {
    fn section() -> SettingsSection {
        SettingsSection::History
    }
    fn should_render(&self, _: &AppContext) -> bool {
        true
    }
    fn on_page_selected(&mut self, _: bool, ctx: &mut ViewContext<Self>) {
        self.load(ctx);
    }
    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        let query = query.trim().to_lowercase();
        let generic = query.is_empty() || HistoryWidget.search_terms().contains(&query);
        self.folder_filter = if generic { String::new() } else { query };
        self.selected = None;
        self.pending_clear = None;
        self.page.update_filter("", ctx);
        ctx.notify();
        MatchData::from(
            generic
                || self.folders.iter().any(|(folder, _, _, _)| {
                    folder.label().to_lowercase().contains(&self.folder_filter)
                }),
        )
    }
    fn scroll_to_widget(&mut self, id: &'static str) {
        self.page.scroll_to_widget(id);
    }
    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}
impl From<ViewHandle<HistoryPageView>> for SettingsPageViewHandle {
    fn from(view: ViewHandle<HistoryPageView>) -> Self {
        Self::History(view)
    }
}

#[cfg(test)]
#[path = "history_page_tests.rs"]
mod tests;
