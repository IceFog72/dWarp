use super::{Window, WindowCallbacks};

fn window_without_native_resources() -> Window {
    Window::new(WindowCallbacks {
        standard_action_callback: Box::new(|_, _| {}),
        event_callback: Box::new(|_, _| unreachable!()),
        resize_callback: Box::new(|_, _| {}),
        build_scene_callback: Box::new(|_, _| unreachable!()),
        frame_callback: Box::new(|_| {}),
        draw_frame_error_callback: Box::new(|_| {}),
        move_callback: Box::new(|_, _| {}),
        active_cursor_position_callback: Box::new(|_| None),
    })
}

#[test]
fn skipped_redraw_consumes_the_queued_request() {
    let window = window_without_native_resources();
    window.redraw_pending.set(true);

    // Startup and hidden windows can receive a redraw they cannot yet present.
    assert!(!window.begin_redraw());
    assert!(!window.redraw_pending.get());
}

#[test]
fn skipped_redraw_preserves_scene_invalidation() {
    let window = window_without_native_resources();
    window.needs_rebuild.set(true);
    window.redraw_pending.set(true);

    assert!(!window.begin_redraw());
    assert!(window.needs_rebuild.get());
    assert!(!window.redraw_pending.get());
}
