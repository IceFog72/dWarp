//! AI functionality that remains after removing the Warp agent: BYOP providers, AI command
//! suggestions, and skill discovery for third-party CLI agents.
pub(crate) mod agent_providers;
pub(crate) mod block_context;
pub mod execution_context;
pub(crate) mod llms;
pub mod outline;
pub(crate) mod persisted_workspace;
pub(crate) mod predict;
pub(crate) mod skills;
pub mod ui;
