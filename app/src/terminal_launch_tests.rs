use super::*;

#[test]
fn command_arguments_are_quoted_for_each_shell() {
    let args = vec!["tool".to_owned(), "a'b\\c\n$(touch nope)".to_owned()];
    assert_eq!(
        command_line(&args, ShellType::Bash),
        "command 'tool' 'a'\\''b\\c\n$(touch nope)'"
    );
    assert_eq!(
        command_line(&args, ShellType::Fish),
        "command 'tool' 'a\\'b\\\\c\n$(touch nope)'"
    );
    assert_eq!(
        command_line(&args, ShellType::PowerShell),
        "& 'tool' 'a''b\\c\n$(touch nope)'"
    );
}

#[test]
fn grouped_commands_change_directory_only_if_the_change_succeeds() {
    let args = vec!["tool".to_owned()];
    let directory = Some(Path::new("/path with 'quotes'"));
    assert_eq!(
        command_line_in_directory(&args, directory, ShellType::Bash),
        "builtin cd -- '/path with '\\''quotes'\\''' && command 'tool'"
    );
    assert_eq!(
        command_line_in_directory(&args, directory, ShellType::Fish),
        "builtin cd -- '/path with \\'quotes\\''; and command 'tool'"
    );
    assert_eq!(
        command_line_in_directory(&args, directory, ShellType::PowerShell),
        "Set-Location -LiteralPath '/path with ''quotes''' -ErrorAction Stop; & 'tool'"
    );
}

#[cfg(unix)]
#[test]
fn bash_launch_preserves_literal_arguments_and_working_directory() {
    let directory = tempfile::Builder::new()
        .prefix("dwarp ' directory ")
        .tempdir()
        .unwrap();
    let args = vec![
        "printf".to_owned(),
        "%s\\n".to_owned(),
        "$(touch unintended)".to_owned(),
        "a'b".to_owned(),
    ];
    let line = command_line_in_directory(&args, Some(directory.path()), ShellType::Bash);
    let output = command::blocking::Command::new("bash")
        .args(["--noprofile", "--norc", "-c", &line])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "$(touch unintended)\na'b\n"
    );
    assert!(!directory.path().join("unintended").exists());
}
