//! These types are named after the database tables, and are used to represent specific queries.

use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use super::schema::{
    app, blocks, code_pane_tabs, code_panes, code_review_panes, commands, folders,
    ignored_suggestions, notebook_panes, notebooks, pane_branches, pane_leaves, pane_nodes, panels,
    project_rules, projects, server_experiments, settings_panes, ssh_nodes, ssh_servers, tabs,
    terminal_panes, welcome_panes, windows, workflows, workspace_language_server,
    workspace_metadata, workspaces,
};

#[derive(Insertable)]
#[diesel(table_name = app)]
pub struct NewApp {
    pub active_window_id: Option<i32>,
}

#[derive(Identifiable, Queryable)]
pub struct Window {
    pub id: i32,
    pub active_tab_index: i32,
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub origin_x: Option<f32>,
    pub origin_y: Option<f32>,
    pub quake_mode: bool,
    pub universal_search_width: Option<f32>,
    pub warp_ai_width: Option<f32>,
    pub voltron_width: Option<f32>,
    pub warp_drive_index_width: Option<f32>,
    pub fullscreen_state: i32,
    pub agent_management_filters: Option<String>,
    pub left_panel_open: Option<bool>,
    pub vertical_tabs_panel_open: Option<bool>,
}

#[derive(Insertable, Queryable)]
pub struct Workflow {
    pub id: i32,
    pub data: String,
}

/// A type representing a `Workflow` that is newly created. We purposefully
/// do not include the `id` here since it is unset.

#[derive(Identifiable, Insertable, Queryable)]
pub struct Notebook {
    pub id: i32,
    pub title: Option<String>,
    pub data: Option<String>,
    pub ai_document_id: Option<String>,
}

#[derive(Insertable, Identifiable, Queryable)]
pub struct Folder {
    pub id: i32,
    pub name: String,
    pub is_open: bool,
    pub is_warp_pack: bool,
}

#[derive(Identifiable, Insertable, Queryable)]
pub struct Workspace {
    pub id: i32,
    pub name: String,
    pub server_uid: String,
    pub is_selected: bool,
}

#[derive(Clone, Identifiable, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = project_rules)]
pub struct ProjectRules {
    pub id: i32,
    pub path: String,
    pub project_root: String,
}

#[derive(Clone, Identifiable, Queryable, AsChangeset)]
#[diesel(table_name = workspace_metadata)]
pub struct WorkspaceMetadata {
    pub id: i32,
    pub repo_path: String,
    pub navigated_ts: Option<NaiveDateTime>,
    pub modified_ts: Option<NaiveDateTime>,
    pub queried_ts: Option<NaiveDateTime>,
}

#[derive(Clone, Insertable, AsChangeset)]
#[diesel(table_name = workspace_metadata)]
pub struct NewWorkspaceMetadata {
    pub repo_path: String,
    pub navigated_ts: Option<NaiveDateTime>,
    pub modified_ts: Option<NaiveDateTime>,
    pub queried_ts: Option<NaiveDateTime>,
}

#[derive(Clone, Identifiable, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = workspace_language_server)]
pub struct WorkspaceLanguageServer {
    pub id: i32,
    pub workspace_id: i32,
    pub language_server_name: String,
    pub enabled: String,
}

#[derive(Clone, Insertable, AsChangeset)]
#[diesel(table_name = workspace_language_server)]
pub struct NewWorkspaceLanguageServer {
    pub workspace_id: i32,
    pub language_server_name: String,
    pub enabled: String,
}

#[derive(Default, Clone, Debug, Insertable, Queryable, AsChangeset)]
#[diesel(table_name = projects)]
pub struct Project {
    pub path: String,
    pub added_ts: NaiveDateTime,
    pub last_opened_ts: Option<NaiveDateTime>,
}

impl Project {
    pub fn last_used_at(&self) -> NaiveDateTime {
        self.last_opened_ts.unwrap_or(self.added_ts)
    }
}

impl PartialEq for Project {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for Project {}

#[derive(Insertable)]
#[diesel(table_name = windows)]
pub struct NewWindow {
    pub active_tab_index: i32,
    pub window_width: Option<f32>,
    pub window_height: Option<f32>,
    pub origin_x: Option<f32>,
    pub origin_y: Option<f32>,
    pub quake_mode: bool,
    pub universal_search_width: Option<f32>,
    pub warp_ai_width: Option<f32>,
    pub voltron_width: Option<f32>,
    pub warp_drive_index_width: Option<f32>,
    pub fullscreen_state: i32,
    pub agent_management_filters: Option<String>,
    pub left_panel_open: Option<bool>,
    pub vertical_tabs_panel_open: Option<bool>,
}

#[derive(Identifiable, Queryable, Associations)]
#[diesel(belongs_to(Window))]
pub struct Tab {
    pub id: i32,
    pub window_id: i32,
    pub custom_title: Option<String>,
    pub color: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = tabs)]
pub struct NewTab {
    pub window_id: i32,
    pub custom_title: Option<String>,
    pub color: Option<String>,
}

/// The panes data model includes pane_nodes, pane_leaves and pane_branches.
/// In addition, each kind of pane has a table for its specific data (i.e. the cwd for terminal panes).
/// The pane_nodes table specifically keeps the node data so it is responsible for
/// keeping track of the tree relationships.
/// The pane_leaves table keeps info about a given pane (i.e. what kind of pane it is).
/// The pane_branches table keeps info about a branch in the tree (e.g. whether
/// the branch splits horizontally or vertically).
#[derive(Identifiable, Queryable)]
#[diesel(table_name = pane_leaves)]
#[diesel(primary_key(pane_node_id, kind))]
pub struct PaneLeaf {
    pub pane_node_id: i32,
    pub kind: String,
    pub is_focused: bool,
    pub custom_vertical_tabs_title: Option<String>,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = terminal_panes)]
#[diesel(primary_key(id))]
pub struct TerminalPane {
    pub id: i32,
    // This is hardcoded in the database, and not used in the app, but Diesel requires it so that
    // fields are in the same order as the table's columns.
    pub kind: String,
    pub uuid: Vec<u8>,
    pub cwd: Option<String>,
    pub is_active: bool,
    /// This is serialized JSON data for a ShellLaunchData struct.
    pub shell_launch_data: Option<String>,
    /// This is serialized JSON data for an InputConfig struct.
    pub input_config: Option<String>,
    pub llm_model_override: Option<String>,
    pub active_profile_id: Option<String>,
    /// This is serialized JSON data for a Vec<AIConversationId>.
    pub conversation_ids: Option<String>,
    /// The active conversation ID if the agent view was open in fullscreen mode.
    pub active_conversation_id: Option<String>,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = notebook_panes)]
#[diesel(primary_key(id))]
pub struct NotebookPane {
    pub id: i32,
    // This is hardcoded in the database, and not used in the app, but Diesel requires it so that
    // fields are in the same order as the table's columns.
    pub kind: String,
    pub notebook_id: Option<String>,
    pub local_path: Option<Vec<u8>>,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = code_panes)]
#[diesel(primary_key(id))]
pub struct CodePane {
    pub id: i32,
    pub active_tab_index: i32,
    pub source_data: Option<String>,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = code_pane_tabs)]
#[diesel(primary_key(id))]
pub struct CodePaneTab {
    pub id: i32,
    pub code_pane_id: i32,
    pub tab_index: i32,
    pub local_path: Option<Vec<u8>>,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = code_review_panes)]
#[diesel(primary_key(id))]
pub struct CodeReviewPane {
    pub id: i32,
    pub kind: String,
    pub terminal_uuid: Vec<u8>,
    pub repo_path: String,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = settings_panes)]
#[diesel(primary_key(id))]
pub struct SettingsPane {
    pub id: i32,
    pub kind: String,
    pub current_page: String,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = welcome_panes)]
#[diesel(primary_key(id))]
pub struct WelcomePane {
    pub id: i32,
    pub kind: String,
    pub startup_directory: Option<String>,
}

/// Subset of the [`terminal_panes`] table needed to retrieve per-session block lists.
///
/// The true primary key of the table is [`terminal_panes::id`]. However, Diesel's associations API
/// requires matching on the primary key, so this view pretends that [`terminal_panes::uuid`] is
/// the primary key. This is safe because the UUID is _also_ unique across all panes.
#[derive(Identifiable, Selectable, Queryable)]
#[diesel(table_name = terminal_panes)]
#[diesel(primary_key(uuid))]
pub struct TerminalSession {
    pub uuid: Vec<u8>,
}

#[derive(Queryable)]
pub struct PaneBranch {
    #[allow(dead_code)]
    pub id: i32,
    #[allow(dead_code)]
    pub pane_node_id: i32,
    pub horizontal: bool,
}

#[derive(Queryable)]
pub struct PaneNode {
    pub id: i32,
    #[allow(dead_code)]
    pub tab_id: i32,
    #[allow(dead_code)]
    pub parent_pane_node_id: Option<i32>,
    pub flex: Option<f32>,
    pub is_leaf: bool,
}

#[derive(Insertable)]
#[diesel(table_name = pane_leaves)]
pub struct NewPane {
    pub pane_node_id: i32,
    pub kind: String,
    pub is_focused: bool,
    pub custom_vertical_tabs_title: Option<String>,
}

/// The [`pane_leaves::kind`] value for terminal panes.
pub const TERMINAL_PANE_KIND: &str = "terminal";

/// The [`pane_leaves::kind`] value for notebook panes.
pub const NOTEBOOK_PANE_KIND: &str = "notebook";

/// The [`pane_leaves::kind`] value for EVC panes.
pub const ENV_VAR_COLLECTION_PANE_KIND: &str = "env_var_collection";

/// The [`pane_leaves::kind`] value for code panes.
pub const CODE_PANE_KIND: &str = "code";

/// The [`pane_leaves::kind`] value for workflow panes.
pub const WORKFLOW_PANE_KIND: &str = "workflow";

/// The [`pane_leaves::kind`] value for settings panes.
pub const SETTINGS_PANE_KIND: &str = "settings";

/// The [`pane_leaves::kind`] value for AI fact panes
/// (where kind name is historical and not worth a migration to change).
pub const AI_FACT_PANE_KIND: &str = "ai_memory";

/// The [`pane_leaves::kind`] value for MCP server panes
pub const MCP_SERVER_PANE_KIND: &str = "mcp_server";

/// The [`pane_leaves::kind`] value for code review panes.
pub const CODE_REVIEW_PANE_KIND: &str = "code_review";

/// The [`pane_leaves::kind`] value for execution profile editor panes.
pub const EXECUTION_PROFILE_EDITOR_PANE_KIND: &str = "execution_profile_editor";

/// The [`pane_leaves::kind`] value for the welcome pane.
pub const WELCOME_PANE_KIND: &str = "welcome";

/// The [`pane_leaves::kind`] value for the get-started pane.
pub const GET_STARTED_PANE_KIND: &str = "get_started";

/// The [`pane_leaves::kind`] value for AI document panes.
pub const AI_DOCUMENT_PANE_KIND: &str = "ai_document";

/// The [`pane_leaves::kind`] value for ambient agent (cloud mode) panes.
pub const AMBIENT_AGENT_PANE_KIND: &str = "ambient_agent";

#[derive(Insertable)]
#[diesel(table_name = terminal_panes)]
pub struct NewTerminalPane {
    pub id: i32,
    pub uuid: Vec<u8>,
    pub cwd: Option<String>,
    pub is_active: bool,
    /// This is serialized JSON data for a ShellLaunchData struct.
    pub shell_launch_data: Option<String>,
    /// This is serialized JSON data for an InputConfig struct.
    pub input_config: Option<String>,
    pub llm_model_override: Option<String>,
    pub active_profile_id: Option<String>,
    /// This is serialized JSON data for a Vec<AIConversationId>.
    pub conversation_ids: Option<String>,
    /// The active conversation ID if the agent view was open in fullscreen mode.
    pub active_conversation_id: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = notebook_panes)]
pub struct NewNotebookPane {
    pub id: i32,
    pub notebook_id: Option<String>,
    pub local_path: Option<Vec<u8>>,
}

#[derive(Insertable)]
#[diesel(table_name = code_panes)]
pub struct NewCodePane {
    pub id: i32,
    pub active_tab_index: i32,
    pub source_data: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = code_pane_tabs)]
pub struct NewCodePaneTab {
    pub code_pane_id: i32,
    pub tab_index: i32,
    pub local_path: Option<Vec<u8>>,
}

#[derive(Insertable)]
#[diesel(table_name = code_review_panes)]
pub struct NewCodeReviewPane {
    pub id: i32,
    pub terminal_uuid: Vec<u8>,
    pub repo_path: String,
}

#[derive(Insertable)]
#[diesel(table_name = settings_panes)]
pub struct NewSettingsPane {
    pub id: i32,
    pub current_page: String,
}

#[derive(Insertable)]
#[diesel(table_name = welcome_panes)]
pub struct NewWelcomePane {
    pub id: i32,
    pub startup_directory: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = pane_branches)]
pub struct NewPaneBranch {
    pub pane_node_id: i32,
    pub horizontal: bool,
}

#[derive(Insertable)]
#[diesel(table_name = pane_nodes)]
pub struct NewPaneNode {
    pub tab_id: i32,
    pub parent_pane_node_id: Option<i32>,
    pub flex: Option<f32>,
    pub is_leaf: bool,
}

#[derive(Insertable)]
#[diesel(table_name = blocks)]
pub struct NewBlock<'a> {
    pub block_id: &'a str,
    // Note that there is no pane leaf UUID foreign key relationship because there's no good way to
    // enforce it: when we remove a pane and subsequently create a new snapshot, the old blocks
    // will now violate the constraint. While sqlite does have deferred constraints, it doesn't
    // work well with ON DELETE CASCADE (i.e. the cascade happens on the delete, not after the
    // transaction commit).
    pub pane_leaf_uuid: Vec<u8>,
    pub stylized_command: &'a Vec<u8>,
    pub stylized_output: &'a Vec<u8>,
    pub pwd: Option<&'a String>,
    pub git_branch: Option<&'a String>,
    pub git_branch_name: Option<&'a String>,
    pub virtual_env: Option<&'a String>,
    pub conda_env: Option<&'a String>,
    pub exit_code: i32,
    pub did_execute: bool,
    pub is_background: bool,
    pub completed_ts: Option<NaiveDateTime>,
    pub start_ts: Option<NaiveDateTime>,
    pub ps1: Option<&'a String>,
    pub rprompt: Option<&'a String>,
    pub honor_ps1: bool,
    pub shell: Option<&'a str>,
    pub user: Option<&'a str>,
    pub host: Option<&'a str>,
    pub prompt_snapshot: Option<&'a String>,
    pub ai_metadata: Option<&'a String>,
    pub is_local: Option<bool>,
    pub agent_view_visibility: Option<String>,
}

#[derive(Identifiable, Queryable, Selectable, Associations)]
#[diesel(table_name = blocks)]
#[diesel(belongs_to(TerminalSession, foreign_key = pane_leaf_uuid))]
pub struct Block {
    pub id: Option<i32>,
    pub pane_leaf_uuid: Vec<u8>,
    pub stylized_command: Vec<u8>,
    pub stylized_output: Vec<u8>,
    pub pwd: Option<String>,
    pub git_branch: Option<String>,
    pub git_branch_name: Option<String>,
    pub virtual_env: Option<String>,
    pub conda_env: Option<String>,
    pub exit_code: i32,
    pub did_execute: bool,
    pub completed_ts: Option<NaiveDateTime>,
    pub start_ts: Option<NaiveDateTime>,
    pub ps1: Option<String>,
    pub honor_ps1: bool,
    pub shell: Option<String>,
    pub user: Option<String>,
    pub host: Option<String>,
    pub is_background: bool,
    pub rprompt: Option<String>,
    /// JSON-serialized representation of the Warp prompt snapshot (Context Chips). Note that this
    /// is different from PS1 and RPROMPT1
    pub prompt_snapshot: Option<String>,
    pub block_id: String,
    pub ai_metadata: Option<String>,
    pub is_local: Option<bool>,
    pub agent_view_visibility: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = commands)]
pub struct NewCommand {
    pub command: String,
    pub exit_code: Option<i32>,
    pub start_ts: Option<NaiveDateTime>,
    pub completed_ts: Option<NaiveDateTime>,
    pub pwd: Option<String>,
    pub shell: Option<String>,
    pub username: Option<String>,
    pub hostname: Option<String>,
    pub session_id: Option<i64>,
    pub git_branch: Option<String>,
    pub cloud_workflow_id: Option<String>,
    pub workflow_command: Option<String>,
    pub is_agent_executed: Option<bool>,
}

#[derive(Identifiable, Queryable, Default, Clone)]
#[diesel(table_name = commands)]
pub struct Command {
    pub id: i32,
    pub command: String,
    pub exit_code: Option<i32>,
    pub start_ts: Option<NaiveDateTime>,
    pub completed_ts: Option<NaiveDateTime>,
    pub pwd: Option<String>,
    pub shell: Option<String>,
    pub username: Option<String>,
    pub hostname: Option<String>,
    pub session_id: Option<i64>,
    pub git_branch: Option<String>,
    pub cloud_workflow_id: Option<String>,
    pub workflow_command: Option<String>,
    pub is_agent_executed: Option<bool>,
}

#[derive(Insertable, Queryable)]
pub struct ServerExperiment {
    pub experiment: String,
}

// Queryable structs for reading from the database
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AIAgentActionId(pub String);

#[derive(Debug, Insertable)]
#[diesel(table_name = ignored_suggestions)]
pub struct NewIgnoredSuggestion {
    pub suggestion: String,
    pub suggestion_type: String,
}

#[derive(Insertable)]
#[diesel(table_name = panels)]
pub struct NewPanel {
    pub tab_id: i32,
    pub left_panel: Option<String>,
    pub right_panel: Option<String>,
}

#[derive(Identifiable, Queryable, Selectable)]
#[diesel(table_name = panels)]
pub struct Panel {
    pub id: i32,
    pub tab_id: i32,
    pub left_panel: Option<String>,
    pub right_panel: Option<String>,
}

// --- SSH Manager ---------------------------------------------------------
// Tree of folders and servers in the left-panel SSH manager. `parent_id` is
// nullable for root-level entries; `kind` is `'folder'` or `'server'`.
// Server-only metadata lives in `ssh_servers`. Secrets (password/passphrase)
// are NOT stored here — they go into the OS keychain keyed by `node_id`.

#[derive(Identifiable, Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = ssh_nodes)]
#[diesel(primary_key(id))]
pub struct SshNodeRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub sort_order: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    /// 仅对 folder 有意义;server 永远是 false。
    pub is_collapsed: bool,
}

#[derive(Insertable, AsChangeset, Clone, Debug)]
#[diesel(table_name = ssh_nodes)]
pub struct NewSshNode<'a> {
    pub id: &'a str,
    pub parent_id: Option<&'a str>,
    pub kind: &'a str,
    pub name: &'a str,
    pub sort_order: i32,
}

#[derive(Identifiable, Queryable, Selectable, Associations, Clone, Debug)]
#[diesel(table_name = ssh_servers)]
#[diesel(primary_key(node_id))]
#[diesel(belongs_to(SshNodeRow, foreign_key = node_id))]
pub struct SshServerRow {
    pub node_id: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub auth_type: String,
    pub key_path: Option<String>,
    pub last_connected_at: Option<NaiveDateTime>,
}

#[derive(Insertable, AsChangeset, Clone, Debug)]
#[diesel(table_name = ssh_servers)]
pub struct NewSshServer<'a> {
    pub node_id: &'a str,
    pub host: &'a str,
    pub port: i32,
    pub username: &'a str,
    pub auth_type: &'a str,
    pub key_path: Option<&'a str>,
}
