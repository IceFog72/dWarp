use super::*;

#[test]
fn cpu_terminal_frame_interval_uses_env_fps() {
    assert_eq!(
        fps_interval(Some("10"), DEFAULT_CPU_TERMINAL_FPS),
        Duration::from_millis(100)
    );
}

#[test]
fn cpu_terminal_frame_interval_uses_default_for_invalid_fps() {
    assert_eq!(
        fps_interval(Some("0"), DEFAULT_CPU_TERMINAL_FPS),
        Duration::from_micros(1_000_000 / DEFAULT_CPU_TERMINAL_FPS)
    );
}
