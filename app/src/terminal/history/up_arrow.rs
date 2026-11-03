use std::{cmp::Ordering, collections::HashSet};

use warpui::{AppContext, SingletonEntity};

use crate::input_suggestions::HistoryInputSuggestion;
use crate::settings::InputSettings;
use crate::suggestions::ignored_suggestions_model::{IgnoredSuggestionsModel, SuggestionType};
use crate::terminal::model::session::SessionId;

use super::History;

fn sort_and_dedupe_suggestions<'a>(
    mut suggestions: Vec<HistoryInputSuggestion<'a>>,
    session_id: Option<SessionId>,
    current_directory: Option<&str>,
    all_live_session_ids: &HashSet<SessionId>,
) -> Vec<HistoryInputSuggestion<'a>> {
    let in_current_folder = |suggestion: &HistoryInputSuggestion| {
        let HistoryInputSuggestion::Command { entry } = suggestion;
        current_directory.is_some_and(|path| entry.pwd.as_deref() == Some(path))
    };
    suggestions.sort_by(|a, b| match (in_current_folder(a), in_current_folder(b)) {
        // Stable order keeps saved commands chronological and newly appended live commands last.
        (true, true) => Ordering::Equal,
        (false, false) => a.cmp(b, session_id, all_live_session_ids),
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
    });

    // Deduplicate commands: keep the latest occurrence of each.
    let mut seen_commands: HashSet<&str> = HashSet::new();
    let mut skip_indices: HashSet<usize> = HashSet::new();
    for (idx, suggestion) in suggestions.iter().enumerate().rev() {
        let text = suggestion.text();
        if seen_commands.contains(text) {
            skip_indices.insert(idx);
        } else {
            seen_commands.insert(text);
        }
    }

    suggestions
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !skip_indices.contains(idx))
        .map(|(_, suggestion)| suggestion)
        .collect()
}

impl History {
    pub(crate) fn up_arrow_suggestions_for_terminal_view<'a>(
        &'a self,
        session_id: Option<SessionId>,
        current_directory: Option<&str>,
        app: &'a AppContext,
    ) -> Vec<HistoryInputSuggestion<'a>> {
        let ignored_suggestions = IgnoredSuggestionsModel::handle(app).as_ref(app);
        let Some(session_id_value) = session_id else {
            return Vec::new();
        };
        let Some(host) = self.session_id_to_shell_host.get(&session_id_value) else {
            return Vec::new();
        };
        let folder_commands = current_directory.and_then(|path| {
            self.persisted_folder_commands
                .get(&(host.clone(), path.to_owned()))
        });
        let mut saved: Vec<_> = folder_commands
            .into_iter()
            .flat_map(|commands| commands.values())
            .collect();
        // Match the saved-history viewer's ID order, reversed for bottom-first navigation.
        // Shell timestamps may be absent, equal, or from a clock that moved backwards.
        saved.sort_by_key(|(id, _)| *id);
        let mut entries: Vec<_> = saved.into_iter().map(|(_, entry)| entry).collect();
        let imported_count = self.history_file_commands.get(host).map_or(0, Vec::len);
        entries.extend(
            self.collect_visible_commands_for_session(
                session_id_value,
                current_directory,
                *InputSettings::as_ref(app).use_shell_history,
                *InputSettings::as_ref(app).use_restored_history,
                |index, entry| (index, entry.as_ref()),
            )
            .unwrap_or_default()
            .into_iter()
            .filter(|(index, entry)| {
                // Saved folder records already supply these commands in database order.
                !(*index < imported_count
                    && folder_commands
                        .is_some_and(|commands| commands.contains_key(&entry.command))
                    && current_directory.is_some_and(|path| entry.pwd.as_deref() == Some(path)))
            })
            .map(|(_, entry)| entry),
        );
        let commands = entries
            .into_iter()
            .filter(|entry| {
                !ignored_suggestions.is_ignored(&entry.command, SuggestionType::ShellCommand)
            })
            .map(|entry| HistoryInputSuggestion::Command { entry })
            .collect();
        sort_and_dedupe_suggestions(
            commands,
            session_id,
            current_directory,
            &self.all_live_session_ids(),
        )
    }
}

#[cfg(test)]
#[path = "up_arrow_tests.rs"]
mod tests;
