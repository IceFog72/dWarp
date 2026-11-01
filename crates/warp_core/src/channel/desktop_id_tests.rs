use super::Channel;

#[test]
fn desktop_file_id_matches_oss_launcher() {
    let desktop = include_str!("../../../../app/channels/oss/dev.warp.dWarp.desktop");
    let id = Channel::Oss.desktop_file_id();
    assert!(desktop
        .lines()
        .any(|line| line == format!("StartupWMClass={id}")));
    assert!(desktop.lines().any(|line| line == format!("Icon={id}")));
    assert_eq!(id, "dev.warp.dWarp");
}
