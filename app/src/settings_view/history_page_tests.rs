use super::*;
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
            .find(|(folder, _, _)| folder == &selected)
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
                .position(|(folder, _, _)| folder.path.as_deref() == Some("/work"))
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
                .position(|(folder, _, _)| folder.path.as_deref() == Some("/work"))
                .unwrap();
            view.handle_action(&HistoryPageAction::Select(index), ctx);
            view.handle_action(&HistoryPageAction::RequestClear, ctx);
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
