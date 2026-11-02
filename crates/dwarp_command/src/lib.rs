//! Native command execution in a visible dWarp terminal, with output returned to its caller.

use std::path::PathBuf;

#[derive(Debug, Clone, clap::Args)]
pub struct RunArgs {
    /// Directory in which to run the command.
    #[arg(long, alias = "working-directory")]
    pub cwd: Option<PathBuf>,
    /// Dedicated tab name; concurrent commands split within that tab.
    #[arg(
        long,
        env = "DWARP_COMMAND_GROUP",
        default_value = "Codex",
        conflicts_with = "new_tab"
    )]
    pub group: String,
    /// Open a separate tab instead of reusing the command group.
    #[arg(long)]
    pub new_tab: bool,
    /// Raise dWarp for interactive user input, including sudo passwords.
    #[arg(long)]
    pub focus: bool,
    /// dWarp launcher executable.
    #[arg(long, default_value = "dwarp")]
    pub warp: PathBuf,
    /// Cancel the command after this many seconds.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub timeout: Option<u64>,
    /// Maximum time to wait for the visible terminal to start.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..))]
    pub launch_timeout: u64,
    #[arg(long, hide = true, conflicts_with = "command")]
    pub worker: Option<PathBuf>,
    /// Program and literal arguments. Use an explicit shell for shell syntax.
    #[arg(last = true, required_unless_present = "worker", num_args = 1..)]
    pub command: Vec<String>,
}

#[cfg(unix)]
mod unix;

/// `worker_prefix` selects the containing executable's subcommand, if any.
pub fn run(args: &RunArgs, worker_prefix: &[&str]) -> anyhow::Result<i32> {
    #[cfg(unix)]
    {
        if let Some(socket) = &args.worker {
            unix::worker(socket)
        } else {
            unix::client(args, worker_prefix)
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (args, worker_prefix);
        anyhow::bail!("Native visible command execution requires a Unix PTY on this platform")
    }
}
