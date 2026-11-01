//! Local CLI launch requests. Executable requests never enter the public URL handler.

use std::{collections::HashMap, env, path::PathBuf, sync::Arc};

use anyhow::{ensure, Context as _, Result};
use warp_cli::{AppArgs, StartupRequest, TerminalLaunch};
use warpui::AppContext;

use crate::{
    pane_group::{NewTerminalOptions, PanesLayout},
    root_view::{open_new_with_workspace_source, NewWorkspaceSource},
    terminal::shell::ShellType,
    util::path::file_exists_and_is_executable,
    workspace::{active_terminal_in_window, Workspace},
};

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
    let options = NewTerminalOptions::default()
        .with_initial_directory(launch.working_directory)
        .with_homepage_hidden();
    let existing = if launch.new_window {
        None
    } else {
        crate::uri::get_primary_window(ctx.windows().frontmost_window_id(), ctx)
    };
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
    ctx.windows().show_window_and_focus_app(window_id);
    if !launch.command.is_empty() {
        active_terminal_in_window(window_id, ctx, |terminal, ctx| {
            terminal.queue_launch_command(launch.command, ctx);
        });
    }
}

/// Serialize literal arguments only after the actual terminal shell is known.
pub fn command_line(args: &[String], shell: ShellType) -> String {
    let quote = |arg: &str| match shell {
        ShellType::Bash | ShellType::Zsh => format!("'{}'", arg.replace('\'', "'\\''")),
        ShellType::Fish => format!("'{}'", arg.replace('\\', "\\\\").replace('\'', "\\'")),
        ShellType::PowerShell => format!("'{}'", arg.replace('\'', "''")),
    };
    let mut command = match shell {
        ShellType::Bash | ShellType::Zsh | ShellType::Fish => "command ".to_owned(),
        ShellType::PowerShell => "& ".to_owned(),
    };
    command.push_str(
        &args
            .iter()
            .map(|arg| quote(arg))
            .collect::<Vec<_>>()
            .join(" "),
    );
    command
}
