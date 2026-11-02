use std::collections::BTreeMap;

use chrono::{DateTime, Local, Utc};
use futures::channel::oneshot;
use warpui::{
    clipboard::ClipboardContent,
    elements::{Flex, MouseStateHandle, ParentElement},
    ui_components::{button::ButtonVariant, components::UiComponent},
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
    terminal::history::History,
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
    Refresh,
    Back,
    RequestClear,
    ConfirmClear,
    CancelClear,
    Next,
    Previous,
    Copy(i32),
}

pub struct HistoryPageView {
    page: PageType<Self>,
    records: Vec<Command>,
    folders: Vec<(HistoryFolder, usize, MouseStateHandle)>,
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
}

impl HistoryPageView {
    pub fn new() -> Self {
        Self {
            page: PageType::new_monolith(HistoryWidget, None, true),
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
            .map(|(folder, count)| (folder, count, MouseStateHandle::default()))
            .collect();
        if self.selected.as_ref().is_some_and(|folder| {
            !self
                .folders
                .iter()
                .any(|(candidate, _, _)| candidate == folder)
        }) {
            self.selected = None;
        }
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
            HistoryPageAction::Select(index) => {
                if let Some((folder, _, _)) = self.folders.get(*index) {
                    self.selected = Some(folder.clone());
                    self.offset = 0;
                    self.pending_clear = None;
                }
            }
            HistoryPageAction::Refresh => self.load(ctx),
            HistoryPageAction::Back => {
                self.selected = None;
                self.pending_clear = None;
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

struct HistoryWidget;
impl SettingsWidget for HistoryWidget {
    type View = HistoryPageView;
    fn search_terms(&self) -> &str {
        "command history commands history saved folders directory view clear delete cleanup"
    }
    fn render(
        &self,
        view: &HistoryPageView,
        appearance: &Appearance,
        _: &AppContext,
    ) -> Box<dyn Element> {
        let ui = appearance.ui_builder();
        let mut body = Flex::column()
            .with_child(render_page_title("Command history", HEADER_FONT_SIZE, appearance))
            .with_child(ui.span("Browse saved commands by folder and host. Clear removes dWarp's saved command records. Shell history files and terminal scrollback are kept.").with_soft_wrap().build().finish())
            .with_child(history_button("Refresh", &view.refresh, HistoryPageAction::Refresh, view.busy, appearance));
        if view.busy {
            body.add_child(ui.span("Loading…").build().finish());
        }
        if let Some(error) = &view.error {
            body.add_child(ui.span(error.clone()).with_soft_wrap().build().finish());
        }
        if view.folders.is_empty() && !view.busy && view.error.is_none() {
            body.add_child(ui.span("No saved command history.").build().finish());
        }
        if view.selected.is_none() {
            for (index, (folder, count, state)) in view.folders.iter().enumerate() {
                if !folder.label().to_lowercase().contains(&view.folder_filter) {
                    continue;
                }
                body.add_child(history_button(
                    format!("{} ({count} commands)", folder.label()),
                    state,
                    HistoryPageAction::Select(index),
                    view.busy,
                    appearance,
                ));
            }
        }
        if let Some(folder) = &view.selected {
            let records = view.selected_records();
            body.add_child(history_button(
                "Back to folders",
                &view.back,
                HistoryPageAction::Back,
                view.busy,
                appearance,
            ));
            body.add_child(ui.span(folder.label()).with_soft_wrap().build().finish());
            if let Some(records) = &view.pending_clear {
                body.add_child(
                    ui.span(format!(
                        "Permanently delete {} saved command records from this folder?",
                        records.len()
                    ))
                    .with_soft_wrap()
                    .build()
                    .finish(),
                );
                body.add_child(history_button(
                    "Confirm deletion",
                    &view.confirm,
                    HistoryPageAction::ConfirmClear,
                    view.busy,
                    appearance,
                ));
                body.add_child(history_button(
                    "Cancel",
                    &view.cancel,
                    HistoryPageAction::CancelClear,
                    view.busy,
                    appearance,
                ));
            } else {
                body.add_child(history_button(
                    "Clear this folder's saved history…",
                    &view.clear,
                    HistoryPageAction::RequestClear,
                    view.busy,
                    appearance,
                ));
            }
            body.add_child(
                ui.span(format!(
                    "{}–{} of {} commands (newest first)",
                    view.offset + 1,
                    (view.offset + COMMANDS_PER_PAGE).min(records.len()),
                    records.len()
                ))
                .build()
                .finish(),
            );
            body.add_child(history_button(
                "Previous",
                &view.previous,
                HistoryPageAction::Previous,
                view.busy || view.offset == 0,
                appearance,
            ));
            body.add_child(history_button(
                "Next",
                &view.next,
                HistoryPageAction::Next,
                view.busy || view.offset + COMMANDS_PER_PAGE >= records.len(),
                appearance,
            ));
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
                body.add_child(ui.span(format!("{time} · Exit: {exit}")).build().finish());
                body.add_child(
                    ui.span(record.command.clone())
                        .with_soft_wrap()
                        .build()
                        .finish(),
                );
                body.add_child(history_button(
                    "Copy command",
                    &view.copy_states[index],
                    HistoryPageAction::Copy(record.id),
                    view.busy,
                    appearance,
                ));
            }
        }
        body.finish()
    }
}

fn history_button(
    label: impl Into<String>,
    state: &MouseStateHandle,
    action: HistoryPageAction,
    disabled: bool,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let mut button = appearance
        .ui_builder()
        .button(ButtonVariant::Secondary, state.clone())
        .with_text_label(label.into());
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
                || self.folders.iter().any(|(folder, _, _)| {
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
