//! Local CLI launch requests. Executable requests never enter the public URL handler.

use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{ensure, Context as _, Result};
use warp_cli::{AppArgs, StartupRequest, TerminalLaunch};
use warpui::{AppContext, ViewContext};

use crate::{
    pane_group::{Direction, NewTerminalOptions, PaneGroup, PanesLayout},
    root_view::{open_new_with_workspace_source, NewWorkspaceSource},
    terminal::shell::ShellType,
    util::path::file_exists_and_is_executable,
    workspace::{active_terminal_in_window, Workspace},
};

pub(crate) struct PendingCommand {
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
}

pub fn prepare_request(args: &AppArgs) -> Result<StartupRequest> {
    let mut request = args.startup_request()?;
    if let StartupRequest::Terminal(launch) = &mut request {
        validate(launch)?;
    }
    Ok(request)
}

/// Resolve in the caller's directory and PATH before handing off to another process.
pub fn validate(launch: &mut TerminalLaunch) -> Result<()> {
    ensure!(
        !launch.no_focus || !launch.new_window,
        "background launches cannot request a new window"
    );
    if let Some(group) = &launch.command_group {
        ensure!(
            !group.is_empty() && group.len() <= 128 && !group.chars().any(char::is_control),
            "command group must contain 1 to 128 bytes without control characters"
        );
        ensure!(
            !launch.command.is_empty(),
            "command groups require a command"
        );
        ensure!(
            !launch.new_window,
            "command groups cannot request a new window"
        );
    }
    ensure!(
        launch.working_directory.is_absolute() && launch.working_directory.is_dir(),
        "terminal working directory must be an existing absolute directory"
    );
    ensure!(
        launch.command.iter().all(|arg| !arg.contains('\0')),
        "command arguments cannot contain NUL"
    );
    if let Some(program) = launch.command.first_mut() {
        ensure!(!program.is_empty(), "executable cannot be empty");
        let requested = PathBuf::from(&*program);
        let candidates = if requested.components().count() > 1 || requested.is_absolute() {
            vec![launch.working_directory.join(requested)]
        } else {
            env::split_paths(&env::var_os("PATH").unwrap_or_default())
                .map(|directory| launch.working_directory.join(directory).join(&*program))
                .collect()
        };
        #[cfg(windows)]
        let candidates = candidates
            .into_iter()
            .flat_map(|path| {
                let extensions = if path.extension().is_none() {
                    env::var("PATHEXT")
                        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned())
                        .split(';')
                        .filter(|extension| extension.starts_with('.'))
                        .map(|extension| PathBuf::from(format!("{}{extension}", path.display())))
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                std::iter::once(path).chain(extensions)
            })
            .collect::<Vec<_>>();
        let path = candidates
            .into_iter()
            .find(|path| file_exists_and_is_executable(path))
            .with_context(|| format!("executable not found: {program}"))?;
        *program = path
            .canonicalize()?
            .to_str()
            .context("executable path is not UTF-8")?
            .to_owned();
    }
    Ok(())
}

pub fn handle_request(request: StartupRequest, ctx: &mut AppContext) {
    match request {
        StartupRequest::Urls(urls) => {
            for url in urls {
                crate::uri::handle_incoming_uri(&url, ctx);
            }
        }
        StartupRequest::Terminal(launch) => open_terminal(launch, ctx),
    }
}

fn open_terminal(launch: TerminalLaunch, ctx: &mut AppContext) {
    if launch.command_group.is_some() && open_in_command_group(&launch, ctx) {
        return;
    }
    let command_directory = launch
        .command_group
        .as_ref()
        .map(|_| launch.working_directory.clone());
    let options = NewTerminalOptions::default()
        .with_initial_directory(launch.working_directory)
        .with_homepage_hidden();
    let existing = if launch.new_window {
        None
    } else {
        crate::uri::get_primary_window(ctx.windows().frontmost_window_id(), ctx)
    };
    if launch.no_focus && existing.is_none() {
        log::error!("Background command launches require an existing dWarp window");
        return;
    }
    let window_id = if let Some(window_id) = existing {
        let Some(workspace) = ctx
            .views_of_type::<Workspace>(window_id)
            .and_then(|workspaces| workspaces.first().cloned())
        else {
            log::error!("No workspace available for terminal launch");
            return;
        };
        workspace.update(ctx, |workspace, ctx| {
            workspace.add_tab_with_pane_layout(
                PanesLayout::SingleTerminal(Box::new(options)),
                Arc::new(HashMap::new()),
                None,
                ctx,
            );
        });
        window_id
    } else {
        open_new_with_workspace_source(
            NewWorkspaceSource::Session {
                options: Box::new(options),
            },
            ctx,
        )
        .0
    };
    if !launch.no_focus {
        ctx.windows().show_window_and_focus_app(window_id);
    }
    if let Some(group) = launch.command_group {
        if let Some(workspace) = ctx
            .views_of_type::<Workspace>(window_id)
            .and_then(|views| views.first().cloned())
        {
            let pane_group = workspace.as_ref(ctx).active_tab_pane_group().clone();
            pane_group.update(ctx, |pane_group, ctx| {
                pane_group.command_group = Some(group.clone());
                pane_group.set_title(&group, ctx);
            });
        }
    }
    if !launch.command.is_empty() {
        active_terminal_in_window(window_id, ctx, |terminal, ctx| {
            terminal.queue_launch_command(launch.command, command_directory, ctx);
        });
    }
}

fn open_in_command_group(launch: &TerminalLaunch, ctx: &mut AppContext) -> bool {
    let existing = ctx.window_ids().find_map(|window_id| {
        ctx.views_of_type::<Workspace>(window_id)?
            .iter()
            .find_map(|workspace| {
                workspace
                    .as_ref(ctx)
                    .tab_views()
                    .enumerate()
                    .find_map(|(index, pane_group)| {
                        (pane_group.as_ref(ctx).command_group == launch.command_group)
                            .then(|| (window_id, workspace.clone(), index, pane_group.clone()))
                    })
            })
    });
    let Some((window_id, workspace, index, pane_group)) = existing else {
        return false;
    };
    workspace.update(ctx, |workspace, ctx| workspace.activate_tab(index, ctx));
    pane_group.update(ctx, |pane_group, ctx| {
        queue_group_command(pane_group, launch, ctx)
    });
    if !launch.no_focus {
        ctx.windows().show_window_and_focus_app(window_id);
    }
    true
}

fn queue_group_command(
    pane_group: &mut PaneGroup,
    launch: &TerminalLaunch,
    ctx: &mut ViewContext<PaneGroup>,
) {
    let available = pane_group.pane_ids().find_map(|id| {
        let terminal = pane_group.terminal_view_from_pane_id(id, ctx)?;
        terminal
            .as_ref(ctx)
            .is_available_for_command_launch(ctx)
            .then_some((id, terminal))
    });
    let terminal = if let Some((id, terminal)) = available {
        pane_group.focus_pane_by_id(id, ctx);
        Some(terminal)
    } else {
        let id =
            pane_group.add_terminal_pane_ignoring_default_session_mode(Direction::Right, None, ctx);
        pane_group.terminal_view_from_pane_id(id, ctx)
    };
    if let Some(terminal) = terminal {
        terminal.update(ctx, |terminal, ctx| {
            terminal.queue_launch_command(
                launch.command.clone(),
                Some(launch.working_directory.clone()),
                ctx,
            );
        });
    } else {
        log::error!("Could not create a terminal pane for the command group");
    }
}

/// Serialize literal arguments only after the actual terminal shell is known.
pub fn command_line(args: &[String], shell: ShellType) -> String {
    let mut command = match shell {
        ShellType::Bash | ShellType::Zsh | ShellType::Fish => "command ".to_owned(),
        ShellType::PowerShell => "& ".to_owned(),
    };
    command.push_str(
        &args
            .iter()
            .map(|arg| quote_argument(arg, shell))
            .collect::<Vec<_>>()
            .join(" "),
    );
    command
}

fn quote_argument(arg: &str, shell: ShellType) -> String {
    match shell {
        ShellType::Bash | ShellType::Zsh => format!("'{}'", arg.replace('\'', "'\\''")),
        ShellType::Fish => format!("'{}'", arg.replace('\\', "\\\\").replace('\'', "\\'")),
        ShellType::PowerShell => format!("'{}'", arg.replace('\'', "''")),
    }
}

pub(crate) fn command_line_in_directory(
    args: &[String],
    directory: Option<&Path>,
    shell: ShellType,
) -> String {
    let command = command_line(args, shell);
    let Some(directory) = directory else {
        return command;
    };
    let directory = quote_argument(
        directory.to_str().expect("validated UTF-8 directory"),
        shell,
    );
    match shell {
        ShellType::Bash | ShellType::Zsh => format!("builtin cd -- {directory} && {command}"),
        ShellType::Fish => format!("builtin cd -- {directory}; and {command}"),
        ShellType::PowerShell => {
            format!("Set-Location -LiteralPath {directory} -ErrorAction Stop; {command}")
        }
    }
}

#[cfg(test)]
#[path = "terminal_launch_tests.rs"]
mod tests;
