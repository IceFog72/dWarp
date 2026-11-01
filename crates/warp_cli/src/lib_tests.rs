use clap::Parser;

use crate::{Args, Command};

#[test]
fn removed_cloud_commands_are_rejected() {
    for command in [
        "agent",
        "mcp",
        "run",
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
