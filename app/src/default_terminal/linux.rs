use std::{env, path::PathBuf};

use command::blocking::Command;
use warp_core::channel::ChannelState;

use crate::util::path::resolve_executable;

fn helper_file() -> Option<PathBuf> {
    let mut directories = vec![dirs::data_local_dir()?];
    directories.extend(env::split_paths(
        &env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
    ));
    let name = format!("{}.desktop", ChannelState::channel().desktop_file_id());
    directories
        .into_iter()
        .map(|directory| directory.join("xfce4/helpers").join(&name))
        .find(|path| path.is_file())
}

pub fn can_become_default_terminal() -> bool {
    env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| {
        desktop
            .split(':')
            .any(|desktop| desktop.eq_ignore_ascii_case("xfce"))
    }) && helper_file().is_some()
        && resolve_executable("xfce4-mime-settings").is_some()
}

pub fn is_warp_default_terminal() -> bool {
    if !can_become_default_terminal() {
        return false;
    }
    let helper = if resolve_executable("xfce4-mime-helper").is_some() {
        "xfce4-mime-helper"
    } else if resolve_executable("exo-helper").is_some() {
        "exo-helper"
    } else {
        return false;
    };
    Command::new(helper)
        .args(["--query", "TerminalEmulator"])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).trim()
                    == ChannelState::channel().desktop_file_id()
        })
}

/// Open the desktop's chooser so the user controls the system preference.
pub fn set_warp_as_default_terminal() -> Result<(), String> {
    if !can_become_default_terminal() {
        return Err(
            "The installed desktop does not provide a supported terminal chooser".to_owned(),
        );
    }
    Command::new("xfce4-mime-settings")
        .spawn()
        .map(|mut child| {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        })
        .map_err(|err| err.to_string())
}
