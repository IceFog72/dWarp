# Handoff: Warp Drive / auth / cloud-object removal (mid-flight, tree does NOT compile)

## Goal and constraints
Fork = debloated terminal emulator (OpenWarp / warp-cpu). Remove ALL built-in Warp agent code, login/signup, Warp Drive, teams, cloud objects, and Warp-server features. KEEP: third-party CLI agent support (footer, slash menu, skills, rich input, input history panel, context chips, BYOP AI command suggestions), code-symbol index `ai/outline`, SSH manager. GPU rendering default, CPU softbuffer opt-in.
User rules (verbatim intent):
- "use del code and edit it instead inventing new". **Do not use sub-agents.**
- "nothing should be sent anywhere like telemetry on warp server".
- "pause as you finish a sub task" (report to user after each).
- Never push. Commit only if user authorizes. No destructive git ops. One branch only. Test/run only after all tasks done.
- User asked: "remove warp drive" and "Yes, remove" login/signup + other Warp-server features.
- User remark unanswered: "i dont see options ab history" — history options expected under Settings → Features (inline history configure button opens `SettingsSection::Features`). Check and report.

## Current state
Branch `feature/software-low-cpu-rendering`, HEAD 81c14aed (all work uncommitted in tree).
Backup: `refs/backup/pre-drive-removal` (47e95fbb, pre-Drive tree via stash-create) + `$S/untracked-pre-drive.tgz`. Before Drive removal the tree compiled at 0 errors/0 warnings and the app ran.
`$S` = `/tmp/claude-1000/-mnt-Data-Apps-warp/6a584dc7-522c-4de2-96ba-d33deb9dba64/scratchpad`

Last `cargo check -p warp --tests` (before the most recent edit batch): **206 errors, 45 warnings**, mostly unresolved-import/resolution errors; type errors will show only after those are cleared.

### Done this session (Drive removal, in tree)
- Deleted dirs: `app/src/{drive,cloud_object,env_vars,workspaces}`, `server/{cloud_objects,sync_queue}`, auth UI (`auth_manager`, `auth_view_*`, `login_*`, `web_handoff`, etc.), notebooks cloud parts, workflow views/manager, referrals_page, teams_page, warp_drive_page, `workspace/view/build_plan_migration_modal.rs`, and more (see `git status`).
- `persistence/mod.rs`, `persistence/sqlite.rs` (+tests): removed cloud objects/workspaces/user profiles/object actions/current-user persistence; `ModelEvent` trimmed; `read_sqlite_data(conn)` takes no user id; WORKFLOW/ENV_VAR/cloud-notebook panes restore as Welcome. DB schema/migrations untouched.
- `app_state.rs`: removed Workflow/EnvVarCollection leaf contents; `NotebookPaneSnapshot` only `LocalFileNotebook`.
- `lib.rs`: removed AuthManager, UserWorkspaces, CloudModel, SyncQueue, UpdateManager, etc. singleton setup.
- `auth/auth_state.rs`: removed PersistedUser/persist logic. `auth/mod.rs` only has auth_state, credentials, user, user_uid.
- `workspace/{action,mod,util,view,one_time_modal_model,view/left_panel}.rs`: removed all Drive/notebook-cloud/workflow-cloud/env-var/login/build-plan actions, bindings, modals; left panel has no WarpDrive tab (`MIN_SIDEBAR_WIDTH`/`MAX_SIDEBAR_WIDTH_RATIO` now defined in `left_panel.rs`, imported by `right_panel.rs`).
- `root_view.rs`, `uri/mod.rs`: removed drive/auth/team deep links and handlers.
- `pane_group/mod.rs` Event enum + `pane/terminal_pane.rs`; `terminal/{view,view/action,input}.rs`: removed workflow-modal, signup, add-prompt, env-var-block, team-settings actions/events; `is_block_considered_remote(session_id, app)` now has no policy regex logic; `init_subshell_command(shell_type, ctx)` no env vars.
- `settings_view/mod.rs`, `settings_page.rs`, `code_page.rs`, `keybindings.rs`, `features_page.rs`: removed Teams/Referrals/WarpDrive pages and settings-sync "local only" indicators (`LocalOnlyIconState::for_setting` always Hidden).
- New helper: `terminal/model/session/command_executor/shared.rs::serialize_variables_for_shell(pairs of &str, ShellType)` replacing deleted `env_vars` helper; wired into remote/wsl executors.

## Remaining compile errors (non-test lib, from last check)
- `terminal/view.rs`: stale imports lines ~84, 97-119 (OnboardingDriveSharingBlock, AuthManager, AuthViewVariant, UpdateManager, ObjectActionType, WarpDriveSettings, CloudObject..., env_vars, CloudObjectTypeAndId, UserWorkspaces/UserWorkspacesEvent). `RichContentMetadata::EnvVarCollectionBlock` variant still exists in `terminal/view/rich_content.rs` and use in `terminal/model/blocks/selection.rs:~1043` — remove. `AfterBlockCompletedEvent.cloud_workflow_id/cloud_env_var_collection_id` (terminal/event.rs, model/blocks.rs, block.rs `set_cloud_env_var_state`) now unused → remove. `terminal/view.rs` `ENV_VAR`/`block_onboarding::onboarding_drive_sharing_block` mod gone.
- `pane_group/mod.rs` (8): imports of WarpDriveItemId/ObjectUid/Workflow etc. and `notebook_pane_by_pane_id`/env_var/workflow pane accessors; `pane/mod.rs` (drive::sharing::ShareableObject, EnvVarCollectionView, notebook::NotebookView, workflow_view); `pane/file_pane.rs` needs `subscribe_to_link_model` (was in deleted `notebook_pane.rs`; restore from `git show HEAD:app/src/pane_group/pane/notebook_pane.rs` or move); `pane/view/header/sharing.rs` (drive::sharing) → delete/simplify.
- `search/*`: `command_palette/{data_sources,zero_state,mixer,view,filter_chip_renderer}.rs`, `welcome_palette/view.rs`, `command_search/{view,searcher,zero_state}.rs`, `command_search/workflows/{mod,workflow_search_item}.rs`, `action/search_item.rs` — remove warp_drive/CloudModel/CloudWorkflow/EnvVar/Notebook search items, AuthManager upgrade click (`AttemptLoginGatedUpgrade`), `CommandPaletteEvent::{ExecuteWorkflow,InvokeEnvironmentVariables,OpenNotebook,ViewInWarpDrive}` (workspace/view.rs handler arms already removed).
- `terminal/input.rs`(CloudModel import ~71, env-var selection via CloudModel ~4286, `EnvVarCollectionState`, enum suggestions), `terminal/input/prompts/data_source.rs`, `terminal/input/slash_commands/{mod,data_source/mod,data_source/saved_prompts}.rs` (saved prompts = cloud workflows → delete), `terminal/history.rs` (cloud_object imports), `terminal/safe_mode_settings.rs`, `terminal/cli_agent.rs`, `terminal/cli_agent_input/footer/mod.rs` (AuthManager + UserWorkspaces subscriptions; `sync_remote_control_button`), `terminal/block_list_element.rs`, `terminal/shared_session/permissions_manager.rs` (SharingAccessLevel), `terminal/view/shared_session/view_impl.rs` (ShareableObject).
- `settings/{privacy,init,onboarding,ai,mod}.rs`: remove `cloud_preferences*` re-export (`settings/mod.rs:42`), CloudModel/CloudPreferencesSyncer/EnterpriseSecretRegex/UserWorkspaces uses, `WarpDriveSettings` in init/onboarding. `settings_view/{privacy_page,tab_menu}.rs` (AuthManager open_url_maybe_with_anonymous_token, teams_page, Team, CloudModel), `settings_view/platform/create_api_key_modal.rs`.
- `server/server_api.rs` (AuthManager at ~8 & `app:log_out` dispatch ~667; `object`/`team`/`workspace` client mods removed), `server/graphql/schema/{mod,util}.rs` (cloud_object/env_vars/workflow_enum), `server/experiments/mod.rs` (UserWorkspaces/CustomerType), `server/telemetry/events.rs` (cloud_object/drive imports). Likely delete graphql schema pieces / experiments entirely.
- `ai/aws_credentials.rs`, `ai/predict/next_command_model.rs`, `ai/outline/native.rs` (UserWorkspaces); `notebooks/editor/mod.rs` (CloudWorkflow); `workspace/view/vertical_tabs.rs` (drive icon color/DriveObjectType); `settings_view/mod.rs` leftover refs, `workflows::WorkflowType::Cloud` already removed.
- Tests (`*_test(s).rs`, `test_util/*`, `integration_testing/{warp_drive,cloud_object,workflow,notebook}`): many still import AuthManager/UpdateManager/SyncQueue/Listener/CloudModel/UserWorkspaces/MockTeamClient. Delete cloud-related integration modules and prune test setup.
- Helper `Workflow::default_env_vars`/`workflow.object_id()` leftovers may remain.

## Tools (all in `$S`)
- `ckj.sh`: `cargo check -p warp --tests --message-format json ${FEAT}`; prints "N errors M warnings" + first 50; writes `errs.txt`, `unused.txt`, `d.jsonl`. `FEAT=--all-features` for all-features pass. `apply_diag.py` applies machine-fixable unused import fixes.
- `dropfn.py file fn...` remove fns; `dropitems.py file Type...` remove types+impls; `dropvar.py file Enum Var...` remove enum variants AND single-line `| Var` pattern lines; `droparm.py file Var...` remove match arms (check diff — verified OK so far); `wsrefs.py file` lists lines referencing Drive/auth symbols per fn; `droparg.py`, `okmatch.py`, `useprune.py`.
- Always check diff after scripted edits; `.bak` copies in `$S` (`tv2.bak`, `wv.bak`, `sv.bak`).

## Next action
1. Run `bash $S/ckj.sh`, keep deleting dead code at its owner (list above), re-run until 0 errors and 0 warnings (also `FEAT=--all-features`, `cargo fmt`).
2. Then remove remaining Warp-server pieces: `server_api` auth/block-sharing/referrals/integrations/managed_secrets/presigned_upload, graphql schema, experiments, session sharing, platform/billing/changelog/autoupdate server calls, Warp server URLs + Firebase key in channel config; `app:log_out` dispatch.
3. Dead-code follow-ups: input workflow-enum suggestion machinery, `env_var_collection_state`, `onboarding` crate telemetry/agent slides, ambient_agent leftovers, `IntervalTimer` threading, 6 debug-build lib warnings (code_page `Coords`/`workspace_idx`, input.rs `self` import, shared_session/sharer/network.rs).
4. Fix untranslated Chinese placeholder in AI command-suggestion model picker ("未配置自定义提供商 — 请到").
5. Build `CARGO_INCREMENTAL=0 cargo build --bin warp-oss` (disk nearly full; don't use incremental), run in terminal tab c1 (`ulimit -c 0; RUST_LOG=info ./target/debug/warp-oss`; state dir `~/.local/state/openwarp/`), verify no startup panic (reading unregistered singleton models panics — grep `X::as_ref/handle` for deleted models), confirm blocks run + session restore, find history options in Settings→Features, then tests/clippy/presubmit. Report to user.

## Gotchas
- Rust reports resolution errors first; type errors appear only after.
- Reading an unregistered singleton model panics at runtime (a prior startup panic was `ProjectContextModel`). After removing registrations in `lib.rs`, grep for remaining users.
- Opus subagents hit 429 earlier and user forbade agents.
- App launched via Bash dies with shell; launch in terminal tab.
- Never push; no commit without user OK; commit attribution trailer per system reminder.
