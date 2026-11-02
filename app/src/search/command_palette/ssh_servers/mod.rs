//! Command palette search for SSH servers by name or host.
//!
//! Selecting a result dispatches `WorkspaceAction::OpenSshTerminal`, opening a new
//! tab with SecretInjector, just like Connect in the SSH manager.

pub mod data_source;
pub mod search_item;

pub use data_source::SshServersDataSource;
pub use search_item::SshServerSearchItem;
