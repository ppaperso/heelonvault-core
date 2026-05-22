#![allow(clippy::disallowed_methods)]

//! Unit-style checks for critical lifecycle log markers.
//! These tests protect startup/logout/shutdown markers used by smoke checks.

fn missing_markers(log_content: &str, required_markers: &[&str]) -> Vec<String> {
    required_markers
        .iter()
        .filter(|marker| !log_content.contains(**marker))
        .map(|marker| (*marker).to_string())
        .collect()
}

#[test]
fn lifecycle_log_markers_exist_in_source_code() {
    let main_rs = include_str!("../../heelonvault-app/src/main.rs");
    let main_window_close = include_str!(
        "../../heelonvault-app/src/ui/windows/main_window/impl_core_parts/new_body.inc"
    );
    let login_close =
        include_str!("../../heelonvault-app/src/ui/dialogs/login_dialog/parts/new_body.inc");

    let startup_markers = [
        "tokio runtime started",
        "all services are initialized and ready",
    ];
    let logout_markers = [
        "main window close requested",
        "main window logout completed, login screen will be presented again",
    ];
    let shutdown_markers = ["application shutdown completed"];

    let missing_startup = missing_markers(main_rs, &startup_markers);
    assert!(
        missing_startup.is_empty(),
        "Missing startup marker(s) in src/main.rs: {:?}",
        missing_startup
    );

    let missing_logout_window = missing_markers(main_window_close, &logout_markers[0..1]);
    assert!(
        missing_logout_window.is_empty(),
        "Missing main-window marker(s) in close handler source: {:?}",
        missing_logout_window
    );

    let missing_logout_main = missing_markers(main_rs, &logout_markers[1..2]);
    assert!(
        missing_logout_main.is_empty(),
        "Missing logout marker(s) in src/main.rs: {:?}",
        missing_logout_main
    );

    let missing_shutdown = missing_markers(main_rs, &shutdown_markers);
    assert!(
        missing_shutdown.is_empty(),
        "Missing shutdown marker(s) in src/main.rs: {:?}",
        missing_shutdown
    );

    let required_login_marker = ["login window close requested"];
    let missing_login = missing_markers(login_close, &required_login_marker);
    assert!(
        missing_login.is_empty(),
        "Missing login close marker(s) in login dialog source: {:?}",
        missing_login
    );
}

#[test]
fn marker_checker_reports_missing_markers() {
    let sample_log = "tokio runtime started\nmain window logout completed\n";
    let required = [
        "tokio runtime started",
        "main window logout completed",
        "application shutdown completed",
    ];

    let missing = missing_markers(sample_log, &required);
    assert_eq!(missing, vec!["application shutdown completed".to_string()]);
}
