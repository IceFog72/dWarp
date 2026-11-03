use super::*;
use crate::{
    persistence::model::Command,
    terminal::{
        history::{HistoryEntry, PersistedCommand, ReadHistoryFileState, ShellHost},
        shell::ShellType,
    },
};
use chrono::{DateTime, Utc};
use settings::Setting as _;
use std::sync::Arc;
use warpui::App;

fn host() -> ShellHost {
    ShellHost {
        shell_type: ShellType::Bash,
        user: "alice".into(),
        hostname: "local".into(),
    }
}

fn saved(id: i32, command: &str, path: &str, seconds: i64) -> Command {
    Command {
        id,
        command: command.into(),
        pwd: Some(path.into()),
        hostname: Some("local".into()),
        username: Some("alice".into()),
        shell: Some("bash".into()),
        start_ts: Some(
            DateTime::<Utc>::from_timestamp(seconds, 0)
                .unwrap()
                .naive_utc(),
        ),
        ..Default::default()
    }
}

fn history(records: Vec<Command>, imports: Vec<HistoryEntry>) -> History {
    let mut history = History::new(records.into_iter().map(Into::into).collect());
    let host = host();
    for id in 1..=3 {
        let session = SessionId::from(id);
        history
            .session_id_to_shell_host
            .insert(session, host.clone());
        history.session_start_indices.insert(session, imports.len());
        history
            .session_skip_indices
            .insert(session, Default::default());
    }
    history
        .read_history_file_state
        .insert(host.clone(), ReadHistoryFileState::Done);
    history.session_commands.insert(host.clone(), Vec::new());
    history
        .history_file_commands
        .insert(host, imports.into_iter().map(Arc::new).collect());
    history
}

fn live(session: u64, command: &str, path: &str) -> HistoryEntry {
    HistoryEntry {
        session_id: Some(SessionId::from(session)),
        shell_host: Some(host()),
        pwd: Some(path.into()),
        ..HistoryEntry::command_only(command)
    }
}

fn texts(history: &History, session: u64, folder: Option<&str>, ctx: &AppContext) -> Vec<String> {
    history
        .up_arrow_suggestions_for_terminal_view(Some(SessionId::from(session)), folder, ctx)
        .iter()
        .map(|suggestion| suggestion.text().to_owned())
        .collect()
}

fn init(app: &mut App) {
    app.add_singleton_model(InputSettings::new_with_defaults);
    app.add_singleton_model(|_| IgnoredSuggestionsModel::new(Vec::new()));
}

#[test]
fn history_folder_suggestions_prefer_saved_folder_records_and_keep_newest_at_bottom() {
    App::test((), |mut app| async move {
        init(&mut app);
        let records = vec![
            saved(4, "same command", "/elsewhere", 30),
            saved(3, "new folder command", "/work", 20),
            saved(2, "same command", "/work", 10),
            saved(1, "old folder command", "/work", 5),
        ];
        // Deliberately reverse the imported folder commands. Saved record IDs own their order.
        let imports = vec![
            HistoryEntry::command_only("shell import"),
            HistoryEntry::from(PersistedCommand::from(records[1].clone())),
            HistoryEntry::from(PersistedCommand::from(records[3].clone())),
        ];
        let handle = app.add_model(|_| history(records.clone(), imports));
        handle.read(&app, |history, ctx| {
            assert_eq!(
                texts(history, 1, Some("/work"), ctx),
                vec![
                    "shell import",
                    "old folder command",
                    "same command",
                    "new folder command"
                ]
            );
        });
        InputSettings::handle(&app).update(&mut app, |settings, ctx| {
            settings
                .use_shell_history
                .load_value(false, true, ctx)
                .unwrap()
        });
        handle.read(&app, |history, ctx| {
            assert_eq!(
                texts(history, 1, Some("/work"), ctx),
                vec!["old folder command", "same command", "new folder command"]
            );
        });
        handle.update(&mut app, |history, ctx| {
            history.forget_saved_records(&[records[1].clone()], ctx);
            history.refresh_saved_records(
                records
                    .into_iter()
                    .filter(|record| record.id != 3)
                    .map(Into::into)
                    .collect(),
                ctx,
            );
            assert_eq!(
                texts(history, 1, Some("/work"), ctx),
                vec!["old folder command", "same command"]
            );
            history.refresh_saved_records(Vec::new(), ctx);
            assert!(texts(history, 1, Some("/work"), ctx).is_empty());
        });
    });
}

#[test]
fn history_folder_suggestions_share_new_commands_between_tabs_in_execution_order() {
    App::test((), |mut app| async move {
        init(&mut app);
        let handle = app.add_model(|_| history(Vec::new(), Vec::new()));
        handle.update(&mut app, |history, ctx| {
            let mut earlier = live(2, "older current-tab command", "/work");
            earlier.start_ts = Some(chrono::Local::now());
            history.append_commands(SessionId::from(2), vec![earlier]);
            // New execution can precede the shell's start timestamp. Append order still wins.
            history.append_commands(
                SessionId::from(1),
                vec![live(1, "new shared command", "/work")],
            );
            history.append_commands(
                SessionId::from(3),
                vec![live(3, "other folder command", "/elsewhere")],
            );
            assert_eq!(
                texts(history, 2, Some("/work"), ctx),
                vec!["older current-tab command", "new shared command"]
            );
            assert_eq!(
                texts(history, 3, Some("/elsewhere"), ctx),
                vec!["other folder command"]
            );
            history.append_commands(
                SessionId::from(1),
                vec![live(1, "older current-tab command", "/work")],
            );
            let suggestions = history.up_arrow_suggestions_for_terminal_view(
                Some(SessionId::from(2)),
                Some("/work"),
                ctx,
            );
            assert_eq!(
                suggestions
                    .iter()
                    .map(|suggestion| suggestion.text())
                    .collect::<Vec<_>>(),
                vec!["new shared command", "older current-tab command"]
            );
            let HistoryInputSuggestion::Command { entry } = suggestions.last().unwrap();
            assert_eq!(entry.session_id, Some(SessionId::from(1)));
            assert!(!texts(history, 2, None, ctx).contains(&"new shared command".to_owned()));
        });
    });
}

#[test]
fn history_folder_suggestions_do_not_share_between_hosts_or_accounts() {
    App::test((), |mut app| async move {
        init(&mut app);
        let mut remote = saved(2, "remote secret", "/work", 20);
        remote.hostname = Some("remote".into());
        let mut other_user = saved(3, "other account secret", "/work", 30);
        other_user.username = Some("bob".into());
        let handle = app.add_model(|_| {
            history(
                vec![other_user, remote, saved(1, "local command", "/work", 10)],
                Vec::new(),
            )
        });
        handle.read(&app, |history, ctx| {
            assert_eq!(texts(history, 1, Some("/work"), ctx), vec!["local command"])
        });
    });
}

#[test]
fn history_folder_suggestions_preserve_record_order_with_tied_missing_or_reversed_timestamps() {
    App::test((), |mut app| async move {
        init(&mut app);
        for timestamps in [
            (Some(100), Some(100)),
            (None, None),
            (Some(100), Some(200)),
            (None, Some(200)),
        ] {
            let mut newer = saved(30, "alpha newer", "/work", 100);
            let mut older = saved(20, "zulu older", "/work", 200);
            newer.start_ts = timestamps.0.map(|seconds| {
                DateTime::<Utc>::from_timestamp(seconds, 0)
                    .unwrap()
                    .naive_utc()
            });
            older.start_ts = timestamps.1.map(|seconds| {
                DateTime::<Utc>::from_timestamp(seconds, 0)
                    .unwrap()
                    .naive_utc()
            });
            let history = history(vec![newer, older], Vec::new());
            app.read(|ctx| {
                assert_eq!(
                    texts(&history, 1, Some("/work"), ctx),
                    vec!["zulu older", "alpha newer"]
                )
            });
        }
    });
}
