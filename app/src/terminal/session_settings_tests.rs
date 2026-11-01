use super::{CLIAgentToolbarChipSelection, ToolbarChipSelection};
use crate::terminal::cli_agent_input::footer::toolbar_item::AgentToolbarItemKind;

#[test]
fn saved_toolbar_layout_drops_sharing_and_preserves_local_controls() {
    let saved = r#"{"Custom":{"left":["ShareSession","FileAttach","RichInput"],"right":["Settings","ShareSession"]}}"#;
    let layout: CLIAgentToolbarChipSelection = serde_json::from_str(saved).unwrap();
    assert_eq!(
        layout.left_items(),
        vec![
            AgentToolbarItemKind::FileAttach,
            AgentToolbarItemKind::RichInput
        ]
    );
    assert_eq!(layout.right_items(), vec![AgentToolbarItemKind::Settings]);
}
