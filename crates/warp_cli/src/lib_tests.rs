use clap::Parser;

use crate::{Args, Command};

#[test]
fn removed_cloud_commands_are_rejected() {
    for command in [
        "agent",
        "mcp",
        "task",
        "model",
        "provider",
        "schedule",
        "environment",
        "login",
        "artifact",
    ] {
        assert!(
            Args::try_parse_from(["warp", command]).is_err(),
            "{command}"
        );
    }
}

#[test]
fn app_and_completions_still_parse() {
    assert!(Args::try_parse_from(["warp"]).is_ok());
    let args = Args::try_parse_from(["warp", "completions", "bash"]).unwrap();
    assert!(matches!(args.command(), Some(Command::Completions { .. })));
}

#[test]
fn native_run_keeps_arguments_literal_and_requires_a_program() {
    let args = Args::try_parse_from([
        "dwarp",
        "run",
        "--group",
        "Codex",
        "--focus",
        "--",
        "bash",
        "-lc",
        "printf '%s' '$HOME'",
    ])
    .unwrap();
    let Some(Command::Run(run)) = args.command() else {
        panic!("expected native run");
    };
    assert!(run.focus);
    assert_eq!(run.group, "Codex");
    assert_eq!(run.command, ["bash", "-lc", "printf '%s' '$HOME'"]);
    assert!(Args::try_parse_from(["dwarp", "run"]).is_err());
    assert!(Args::try_parse_from(["dwarp", "run", "--timeout", "0", "--", "true"]).is_err());
}

#[test]
fn command_groups_require_literal_commands_and_cannot_request_new_windows() {
    let args = Args::try_parse_from([
        "dwarp",
        "--no-focus",
        "--command-group",
        "Codex",
        "--execute",
        "printf",
        "hello world",
    ])
    .unwrap();
    let crate::StartupRequest::Terminal(launch) = args.app_args().startup_request().unwrap() else {
        panic!("expected terminal launch");
    };
    assert!(launch.no_focus);
    assert_eq!(launch.command_group.as_deref(), Some("Codex"));
    assert_eq!(launch.command, ["printf", "hello world"]);
    assert!(Args::try_parse_from(["dwarp", "--new-window", "--command-group", "Codex"]).is_err());
    for group in ["", "bad\ngroup"] {
        let args = Args::try_parse_from(["dwarp", "--command-group", group, "--execute", "printf"])
            .unwrap();
        assert!(args.app_args().startup_request().is_err());
    }
    let args = Args::try_parse_from(["dwarp", "--command-group", "Codex"]).unwrap();
    assert!(args.app_args().startup_request().is_err());
}

#[test]
fn previous_terminal_launch_json_still_parses_without_a_group() {
    let launch: crate::TerminalLaunch = serde_json::from_value(serde_json::json!({
        "working_directory": "/tmp", "command": [], "new_window": false
    }))
    .unwrap();
    assert!(launch.command_group.is_none());
    assert!(!launch.no_focus);
}
