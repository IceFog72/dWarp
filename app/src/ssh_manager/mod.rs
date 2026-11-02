//! SSH manager with a folder/server tree in the left panel and a server editor.
//!
//! Storage and SSH command construction live in the `warp_ssh_manager` crate.

pub mod notifier;
pub mod panel;
pub mod secret_injector;
pub mod server_view;

pub use notifier::{SshTreeChangedEvent, SshTreeChangedNotifier};
pub use panel::SshManagerPanel;
// Re-exports for downstream UI consumers.
#[allow(unused_imports)]
pub use panel::{SshManagerPanelAction, SshManagerPanelEvent};
