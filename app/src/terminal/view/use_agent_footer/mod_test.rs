use super::{rich_input_submit_strategy, CLIAgent, RichInputSubmitStrategy};

#[test]
fn multiline_codex_prompts_use_bracketed_paste() {
    assert!(matches!(
        rich_input_submit_strategy(CLIAgent::Codex),
        RichInputSubmitStrategy::BracketedPaste
    ));
}

#[test]
fn copilot_waits_after_bracketed_paste_before_submitting() {
    assert!(matches!(
        rich_input_submit_strategy(CLIAgent::Copilot),
        RichInputSubmitStrategy::BracketedPasteDelayedEnter
    ));
}

#[test]
fn claude_submits_enter_as_a_separate_write() {
    assert!(matches!(
        rich_input_submit_strategy(CLIAgent::Claude),
        RichInputSubmitStrategy::DelayedEnter
    ));
}

#[test]
fn additional_agents_use_their_supported_input_delivery() {
    assert!(matches!(
        rich_input_submit_strategy(CLIAgent::Muse),
        RichInputSubmitStrategy::BracketedPaste
    ));
    assert!(matches!(
        rich_input_submit_strategy(CLIAgent::Antigravity),
        RichInputSubmitStrategy::DelayedEnter
    ));
    assert!(matches!(
        rich_input_submit_strategy(CLIAgent::Kiro),
        RichInputSubmitStrategy::Inline
    ));
}
