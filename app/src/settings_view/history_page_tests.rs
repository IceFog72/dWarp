use super::*;
use crate::settings::{UseRestoredHistory, UseShellHistory};
use warpui::{platform::WindowStyle, units::Pixels, App};

fn record(id: i32, path: Option<&str>, host: &str, user: &str) -> Command {
    Command {
        id,
        command: format!("command {id}"),
        pwd: path.map(str::to_owned),
        hostname: Some(host.into()),
        username: Some(user.into()),
        ..Default::default()
    }
}

#[test]
fn history_management_folder_browser_separates_hosts_users_and_missing_paths() {
    let mut view = HistoryPageView::new();
    view.set_records(vec![
        record(1, Some("/work"), "local", "alice"),
        record(2, Some("/work"), "local", "alice"),
        record(3, Some("/work/child"), "local", "alice"),
        record(4, Some("/work"), "remote", "alice"),
        record(5, Some("/work"), "local", "bob"),
        record(6, None, "local", "alice"),
    ]);
    assert_eq!(view.folders.len(), 5);
    let selected = HistoryFolder::for_record(&view.records[0]);
    assert_eq!(
        view.folders
            .iter()
            .find(|(folder, _, _, _)| folder == &selected)
            .unwrap()
            .1,
        2
    );
    view.selected = Some(selected);
    assert_eq!(
        view.selected_records()
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    view.set_records(vec![record(3, Some("/work/child"), "local", "alice")]);
    assert!(view.selected.is_none());
}

#[test]
fn history_management_viewer_paginates_and_confirmation_tracks_only_selected_folder() {
    App::test((), |mut app| async move {
        init_history_settings(&mut app);
        app.add_singleton_model(|_| Appearance::mock());
        let (_, view) = app.add_window(WindowStyle::NotStealFocus, |_| HistoryPageView::new());
        view.update(&mut app, |view, ctx| {
            let mut records: Vec<_> = (1..=205)
                .rev()
                .map(|id| record(id, Some("/work"), "local", "alice"))
                .collect();
            records.push(record(206, Some("/other"), "local", "alice"));
            view.set_records(records);
            let index = view
                .folders
                .iter()
                .position(|(folder, _, _, _)| folder.path.as_deref() == Some("/work"))
                .unwrap();
            let PageType::Monolith {
                vertical_scroll_state: Some(scroll),
                ..
            } = &view.page
            else {
                panic!("History page must scroll");
            };
            let scroll = scroll.clone();
            scroll.scroll_to(Pixels::new(400.0));
            view.handle_action(&HistoryPageAction::Select(index), ctx);
            assert_eq!(scroll.scroll_start().as_f32(), 0.0);
            assert_eq!(view.selected_records().len(), 205);
            scroll.scroll_to(Pixels::new(300.0));
            view.handle_action(&HistoryPageAction::Next, ctx);
            view.handle_action(&HistoryPageAction::Next, ctx);
            view.handle_action(&HistoryPageAction::Next, ctx);
            assert_eq!(view.offset, 200);
            assert_eq!(scroll.scroll_start().as_f32(), 0.0);
            assert_eq!(view.selected_records().iter().skip(view.offset).count(), 5);
            view.handle_action(&HistoryPageAction::Previous, ctx);
            assert_eq!(view.offset, 100);
            view.handle_action(&HistoryPageAction::RequestClear, ctx);
            let pending = view.pending_clear.as_ref().unwrap();
            assert_eq!(pending.len(), 205);
            assert!(pending.iter().all(|record| record.id != 206));
            view.handle_action(&HistoryPageAction::CancelClear, ctx);
            assert!(view.pending_clear.is_none());
            assert_eq!(view.records.len(), 206);
            view.handle_action(&HistoryPageAction::Back, ctx);
            assert!(view.selected.is_none());
            view.handle_action(&HistoryPageAction::ClearFolder(index), ctx);
            assert_eq!(
                view.selected.as_ref().unwrap().path.as_deref(),
                Some("/work")
            );
            assert_eq!(view.offset, 0);
            assert_eq!(view.pending_clear.as_ref().unwrap().len(), 205);
            assert_eq!(view.records.len(), 206);
            view.handle_action(&HistoryPageAction::CancelClear, ctx);
            assert!(view.pending_clear.is_none());
            assert!(view.update_filter("/other", ctx).is_truthy());
            assert_eq!(view.folder_filter, "/other");
            assert!(view.update_filter("command history", ctx).is_truthy());
            assert!(view.folder_filter.is_empty());
        });
    });
}

#[test]
fn history_management_page_loads_and_clears_through_writer_acknowledgement() {
    use crate::persistence::WriterHandles;
    use std::{sync::mpsc, time::Duration};
    App::test((), |mut app| async move {
        init_history_settings(&mut app);
        app.add_singleton_model(|_| Appearance::mock());
        let (sender, receiver) = mpsc::sync_channel(8);
        let handle = std::thread::spawn(move || {
            let mut saved = vec![
                record(1, Some("/work"), "local", "alice"),
                record(2, Some("/other"), "local", "alice"),
            ];
            loop {
                match receiver.recv().unwrap() {
                    ModelEvent::ReadCommandHistory { reply } => {
                        reply.send(Ok(saved.clone())).unwrap();
                    }
                    ModelEvent::DeleteCommandHistory { records, reply } => {
                        let removed: Vec<_> = saved
                            .iter()
                            .filter(|candidate| {
                                records.iter().any(|record| record.id == candidate.id)
                            })
                            .cloned()
                            .collect();
                        saved.retain(|candidate| {
                            !removed.iter().any(|record| record.id == candidate.id)
                        });
                        reply.send(Ok(removed)).unwrap();
                    }
                    ModelEvent::Terminate => break,
                    event => panic!("Unexpected history writer event: {event:?}"),
                }
            }
        });
        app.add_singleton_model(|_| PersistenceWriter::new(Some(WriterHandles { handle, sender })));
        app.add_singleton_model(|_| History::default());
        let (_, view) = app.add_window(WindowStyle::NotStealFocus, |_| HistoryPageView::new());
        view.update(&mut app, |view, ctx| view.on_page_selected(false, ctx));
        for _attempt in 0..200 {
            if !view.read(&app, |view, _| view.busy) {
                break;
            }
            warpui::r#async::Timer::after(Duration::from_millis(5)).await;
        }
        view.update(&mut app, |view, ctx| {
            assert!(!view.busy);
            assert!(view.error.is_none());
            assert_eq!(view.records.len(), 2);
            let index = view
                .folders
                .iter()
                .position(|(folder, _, _, _)| folder.path.as_deref() == Some("/work"))
                .unwrap();
            view.handle_action(&HistoryPageAction::Select(index), ctx);
            view.handle_action(&HistoryPageAction::DeleteCommand(1), ctx);
            view.handle_action(&HistoryPageAction::ConfirmClear, ctx);
            assert!(view.busy);
        });
        for _attempt in 0..200 {
            if !view.read(&app, |view, _| view.busy) {
                break;
            }
            warpui::r#async::Timer::after(Duration::from_millis(5)).await;
        }
        view.read(&app, |view, _| {
            assert!(!view.busy);
            assert!(view.error.is_none());
            assert_eq!(view.records.len(), 1);
            assert_eq!(view.records[0].pwd.as_deref(), Some("/other"));
            assert!(view.selected.is_none());
        });
    });
}

#[test]
fn history_management_layout_keeps_folder_actions_and_commands_inside_page() {
    use pathfinder_geometry::vector::vec2f;
    use warpui::{Presenter, WindowInvalidation};

    App::test((), |mut app| async move {
        init_history_settings(&mut app);
        app.add_singleton_model(|_| Appearance::mock());
        let (window, view) = app.add_window(WindowStyle::NotStealFocus, |_| HistoryPageView::new());
        view.update(&mut app, |view, _| {
            let mut command = record(
                1,
                Some("/mnt/Data/Apps/project/long-folder-name"),
                "desktop",
                "alice",
            );
            command.command = "cargo test --offline -p warp --lib history_management".into();
            view.set_records(vec![
                command,
                record(2, Some("/home/alice/projects"), "desktop", "alice"),
            ]);
        });
        for width in [520., 900.] {
            for selected in [false, true] {
                if selected {
                    view.update(&mut app, |view, ctx| {
                        view.handle_action(&HistoryPageAction::ClearFolder(0), ctx)
                    });
                } else {
                    view.update(&mut app, |view, ctx| {
                        view.handle_action(&HistoryPageAction::Back, ctx)
                    });
                }
                app.update(|ctx| {
                    let mut presenter = Presenter::new(window);
                    presenter.invalidate(
                        WindowInvalidation {
                            updated: [ctx.root_view_id(window).unwrap()].into_iter().collect(),
                            ..Default::default()
                        },
                        ctx,
                    );
                    let scene = presenter.build_scene(vec2f(width, 900.), 1., None, ctx);
                    let rects: Vec<_> = scene.layers().flat_map(|layer| &layer.rects).collect();
                    assert!(!rects.is_empty());
                    for rect in rects {
                        assert!(
                            rect.bounds.min_x() >= 0.,
                            "Left overflow: {:?}",
                            rect.bounds
                        );
                        assert!(
                            rect.bounds.max_x() <= width,
                            "Right overflow at {width}: {:?}",
                            rect.bounds
                        );
                    }
                });
            }
        }
    });
}

#[test]
fn history_management_shell_file_clear_preserves_file_and_reports_missing_paths() {
    futures_lite::future::block_on(async {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history");
        std::fs::write(&path, "first command\nsecond command\n").unwrap();
        let permissions = std::fs::metadata(&path).unwrap().permissions();
        clear_shell_history_file(&path).await.unwrap();
        assert!(path.is_file());
        assert!(std::fs::read(&path).unwrap().is_empty());
        assert_eq!(std::fs::metadata(&path).unwrap().permissions(), permissions);
        clear_shell_history_file(&path).await.unwrap();
        let missing = directory.path().join("missing");
        assert_eq!(
            clear_shell_history_file(&missing).await.unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        assert!(!missing.exists());
        assert!(clear_shell_history_file(directory.path()).await.is_err());
    });
}

fn init_history_settings(ctx: &mut App) {
    use settings::{PrivatePreferences, PublicPreferences, SettingsManager};
    use warpui_extras::user_preferences::in_memory::InMemoryPreferences;
    ctx.add_singleton_model(|_| PublicPreferences::new(Box::<InMemoryPreferences>::default()));
    ctx.add_singleton_model(|_| PrivatePreferences::new(Box::<InMemoryPreferences>::default()));
    ctx.add_singleton_model(|_| SettingsManager::default());
    InputSettings::register(ctx);
}

#[test]
fn history_management_source_controls_save_settings_and_require_file_confirmation() {
    App::test((), |mut app| async move {
        init_history_settings(&mut app);
        app.add_singleton_model(|_| Appearance::mock());
        app.add_singleton_model(|_| History::default());
        let (_, view) = app.add_window(WindowStyle::NotStealFocus, |_| HistoryPageView::new());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history");
        std::fs::write(&path, "keep until confirmed\n").unwrap();
        view.update(&mut app, |view, ctx| {
            assert!(*InputSettings::as_ref(ctx).use_shell_history);
            assert!(*InputSettings::as_ref(ctx).use_restored_history);
            view.handle_action(&HistoryPageAction::ToggleShellHistory, ctx);
            view.handle_action(&HistoryPageAction::ToggleRestoredHistory, ctx);
            assert!(!*InputSettings::as_ref(ctx).use_shell_history);
            assert!(!*InputSettings::as_ref(ctx).use_restored_history);
            view.shell_files
                .push((path.clone(), vec![], Default::default()));
            view.handle_action(&HistoryPageAction::ConfirmShellClear, ctx);
            assert!(!view.busy);
            view.handle_action(&HistoryPageAction::RequestShellClear(0), ctx);
            assert_eq!(view.pending_shell_clear.as_ref().unwrap().0, path);
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                "keep until confirmed\n"
            );
            view.handle_action(&HistoryPageAction::CancelShellClear, ctx);
            assert!(view.pending_shell_clear.is_none());
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                "keep until confirmed\n"
            );
            view.handle_action(&HistoryPageAction::RequestShellClear(0), ctx);
            view.handle_action(&HistoryPageAction::ConfirmShellClear, ctx);
        });
        for _attempt in 0..200 {
            if !view.read(&app, |view, _| view.busy) {
                break;
            }
            warpui::r#async::Timer::after(std::time::Duration::from_millis(5)).await;
        }
        assert!(!view.read(&app, |view, _| view.busy));
        app.update(|ctx| {
            assert!(!*UseShellHistory::new_from_storage(ctx).value());
            assert!(!*UseRestoredHistory::new_from_storage(ctx).value());
        });
        assert!(std::fs::read(&path).unwrap().is_empty());
        assert!(view.read(&app, |view, _| view.error.is_none()));
        assert!(view.read(&app, |view, _| view.message.is_some()));
    });
}

#[test]
fn history_management_command_selection_spans_pages_and_stays_inside_selected_folder() {
    App::test((), |mut app| async move {
        init_history_settings(&mut app);
        app.add_singleton_model(|_| Appearance::mock());
        let (_, view) = app.add_window(WindowStyle::NotStealFocus, |_| HistoryPageView::new());
        view.update(&mut app, |view, ctx| {
            let mut saved: Vec<_> = (1..=205)
                .rev()
                .map(|id| record(id, Some("/work"), "local", "alice"))
                .collect();
            saved.push(record(206, Some("/other"), "local", "alice"));
            view.set_records(saved.clone());
            let index = view
                .folders
                .iter()
                .position(|(folder, _, _, _)| folder.path.as_deref() == Some("/work"))
                .unwrap();
            view.handle_action(&HistoryPageAction::Select(index), ctx);
            view.handle_action(&HistoryPageAction::ToggleCommand(205), ctx);
            view.handle_action(&HistoryPageAction::Next, ctx);
            view.handle_action(&HistoryPageAction::ToggleCommand(101), ctx);
            assert_eq!(view.selected_commands, [205, 101].into());
            view.handle_action(&HistoryPageAction::DeleteSelectedCommands, ctx);
            assert_eq!(
                view.pending_clear
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|record| record.id)
                    .collect::<Vec<_>>(),
                vec![205, 101]
            );
            view.handle_action(&HistoryPageAction::CancelClear, ctx);
            view.handle_action(&HistoryPageAction::ToggleCommand(206), ctx);
            assert!(!view.selected_commands.contains(&206));
            view.handle_action(&HistoryPageAction::SelectAllCommands, ctx);
            assert_eq!(view.selected_commands.len(), 205);
            view.handle_action(&HistoryPageAction::ToggleCommand(1), ctx);
            view.handle_action(&HistoryPageAction::DeleteSelectedCommands, ctx);
            assert_eq!(view.pending_clear.as_ref().unwrap().len(), 204);
            assert!(!view
                .pending_clear
                .as_ref()
                .unwrap()
                .iter()
                .any(|record| record.id == 1 || record.id == 206));
            view.handle_action(&HistoryPageAction::DeselectAllCommands, ctx);
            view.handle_action(&HistoryPageAction::DeleteSelectedCommands, ctx);
            assert!(view.pending_clear.is_none());
            view.handle_action(&HistoryPageAction::DeleteCommand(101), ctx);
            assert_eq!(
                view.pending_clear
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|record| record.id)
                    .collect::<Vec<_>>(),
                vec![101]
            );
            assert_eq!(
                view.records
                    .iter()
                    .map(|record| record.id)
                    .collect::<Vec<_>>(),
                saved.iter().map(|record| record.id).collect::<Vec<_>>()
            );
            view.handle_action(&HistoryPageAction::CancelClear, ctx);
            view.handle_action(&HistoryPageAction::DeleteCommand(206), ctx);
            assert!(view.pending_clear.is_none());
            view.handle_action(&HistoryPageAction::SelectAllCommands, ctx);
            view.set_records(saved);
            assert!(view.selected_commands.is_empty());
        });
    });
}
