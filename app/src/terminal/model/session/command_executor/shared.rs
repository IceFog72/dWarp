use async_channel::Sender;
use warp_util::path::ShellFamily;

use crate::terminal::{
    model::{
        session::command_executor::{InBandCommand, InBandCommandCancelledEvent},
        tmux::commands::TmuxCommand,
    },
    shell::ShellType,
};

/// Set of events sent by command executors.
pub enum ExecutorCommandEvent {
    /// The command should be executed.
    ExecuteCommand {
        command: InBandCommand,
        /// A Sender that can be used to signal that the command has been cancelled.
        /// Lets us unblock the command in the executor.
        cancel_tx: Sender<InBandCommandCancelledEvent>,
    },
    ExecuteTmuxCommand(TmuxCommand),
    /// The command identified by `id` should be cancelled.
    CancelCommand {
        id: String,
    },
}

pub fn shell_escape_single_quotes(command: &str, shell_type: ShellType) -> String {
    match shell_type {
        ShellType::Fish => {
            // Backslash-escape single quotes for Fish.
            command.replace('\'', r"\'")
        }
        ShellType::PowerShell => {
            // In powershell we escape single quotes using two single quotes ''
            command.replace('\'', "''")
        }
        _ => {
            // For Bash and Zsh, replace each single quote with a '"'"' sequence.
            // The first single quote completes the single quoted string to the left,
            // the next three characters: "'" evaluate to a literal single quote in
            // bash/zsh, and then the final single quote starts a new single-quoted
            // string to the right. Effectively, this concatenates the left
            // single-quoted string, a literal single quote char, and the right
            // single-quoted string.
            command.replace('\'', r#"'"'"'"#)
        }
    }
}

/// Serializes `name=value` assignments so they can be prefixed to a command run in `shell_type`.
pub fn serialize_variables_for_shell<'s, I: IntoIterator<Item = (&'s str, &'s str)>>(
    pairs: I,
    shell_type: ShellType,
) -> String {
    // Prefix, separator between name and value, and postfix of each assignment.
    let (prefix, separator, postfix) = match shell_type {
        // Warp doesn't support newlines in fish so we can't use env syntax
        ShellType::Fish => ("set -x ", " ", ";"),
        ShellType::Bash | ShellType::Zsh => ("", "=", ""),
        ShellType::PowerShell => ("$env:", " = ", ";"),
    };
    let shell_family = ShellFamily::from(shell_type);
    pairs
        .into_iter()
        .map(|(name, value)| {
            let value = match shell_family {
                ShellFamily::Posix => shell_family.escape(value).into_owned(),
                ShellFamily::PowerShell => format!("'{}'", value.replace('\'', "''")),
            };
            format!(
                "{prefix}{}{separator}{value}{postfix}",
                shell_family.escape(name)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}
