//! Data source for the inline history menu, providing command history.
//!
//! Ordering semantics match the legacy up-arrow history menu:
//! - Saved commands for the current folder and commands run there in other tabs appear last
//! - Other items retain their session grouping
//! - Folder records are oldest first; new live commands follow in execution order
//! - Commands are deduplicated, keeping the most recent occurrence
//! - The newest command for the current folder appears at the bottom (closer to input)

use crate::input_suggestions::HistoryInputSuggestion;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::DataSourceRunErrorWrapper;
use crate::search::SyncDataSource;
use crate::terminal::history::{History, LinkedWorkflowData};
use crate::terminal::input::inline_history::search_item::InlineHistoryItem;
use crate::terminal::input::inline_menu::{
    InlineMenuAction, InlineMenuClickBehavior, InlineMenuType,
};
use crate::terminal::model::session::active_session::ActiveSession;
use chrono::Local;
use ordered_float::OrderedFloat;
use warpui::{AppContext, Entity, ModelHandle, SingletonEntity};

#[derive(Clone, Debug)]
pub enum AcceptHistoryItem {
    Command {
        command: String,
        linked_workflow_data: Option<LinkedWorkflowData>,
    },
}

impl AcceptHistoryItem {
    pub fn buffer_replacement_text(&self) -> Option<&String> {
        match self {
            AcceptHistoryItem::Command { command, .. } => Some(command),
        }
    }
}

impl InlineMenuAction for AcceptHistoryItem {
    const MENU_TYPE: InlineMenuType = InlineMenuType::InlineHistoryMenu;

    fn click_behavior(&self) -> InlineMenuClickBehavior {
        InlineMenuClickBehavior::SelectOnClick
    }
}

/// Data source that provides command history for the active session.
pub struct InlineHistoryMenuDataSource {
    active_session: ModelHandle<ActiveSession>,
}

impl InlineHistoryMenuDataSource {
    pub fn new(active_session: ModelHandle<ActiveSession>) -> Self {
        Self { active_session }
    }
}

impl SyncDataSource for InlineHistoryMenuDataSource {
    type Action = AcceptHistoryItem;

    fn run_query(
        &self,
        query: &Query,
        app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        let trimmed_query = query.text.trim();
        let prefix_match_len = trimmed_query.len();

        let session_id = self.active_session.as_ref(app).session(app).map(|s| s.id());

        let history = History::handle(app).as_ref(app);
        let suggestions = history.up_arrow_suggestions_for_terminal_view(
            session_id,
            self.active_session
                .as_ref(app)
                .current_working_directory()
                .map(String::as_str),
            app,
        );

        let mut results: Vec<QueryResult<AcceptHistoryItem>> = Vec::new();
        for suggestion in suggestions {
            let HistoryInputSuggestion::Command { entry } = &suggestion;
            let command = entry.command.trim();
            if command.is_empty() {
                continue;
            }
            if !trimmed_query.is_empty() && !command.starts_with(trimmed_query) {
                continue;
            }
            let timestamp = entry.start_ts.unwrap_or_else(Local::now);
            let item = InlineHistoryItem::command(
                command.to_string(),
                entry.linked_workflow_data(),
                timestamp,
            )
            .with_prefix_match_len(prefix_match_len)
            .with_score(OrderedFloat(results.len() as f64));
            results.push(QueryResult::from(item));
        }

        Ok(results)
    }
}

impl Entity for InlineHistoryMenuDataSource {
    type Event = ();
}
