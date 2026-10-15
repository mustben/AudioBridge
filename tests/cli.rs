use std::io::Write;
use std::process::{Command, Stdio};
static INTERACTIVE_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn interactive_status_and_clean_exit_without_connecting() {
    let _guard = INTERACTIVE_TEST.lock().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_audio-bridge"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"status\nquit\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Aucune session"), "{text}");
    assert!(text.contains("Arret propre"), "{text}");
}

#[test]
fn repair_without_target_is_safe_and_commands_are_explicit() {
    let _guard = INTERACTIVE_TEST.lock().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_audio-bridge"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"repair\nconnect 0\nauto on\nauto off\ndisconnect\nquit\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Aucun appareil selectionne"), "{text}");
    assert!(text.contains("Numero invalide"), "{text}");
    assert!(
        text.contains("Recuperation automatique : activee"),
        "{text}"
    );
    assert!(
        text.contains("Recuperation automatique : desactivee"),
        "{text}"
    );
    assert!(text.contains("Deconnexion volontaire"), "{text}");
}
#[test]
#[ignore = "requires interactive Windows audio/Bluetooth environment"]
fn enumeration_calls_windows_and_reports_devices_without_connecting() {
    let out = Command::new(env!("CARGO_BIN_EXE_audio-bridge"))
        .arg("--list")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("source(s) compatible(s)"));
}
#[test]
fn second_interactive_instance_is_rejected_while_first_is_alive() {
    use std::io::{BufRead, BufReader};
    let _guard = INTERACTIVE_TEST.lock().unwrap();
    let mut first = Command::new(env!("CARGO_BIN_EXE_audio-bridge"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(first.stdout.take().unwrap());
    let mut ready = String::new();
    reader.read_line(&mut ready).unwrap();
    assert!(ready.contains("Journal local"), "{ready}");
    let second = Command::new(env!("CARGO_BIN_EXE_audio-bridge"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    first.stdin.take().unwrap().write_all(b"quit\n").unwrap();
    assert!(first.wait().unwrap().success());
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("deja ouvert"));
}
#[test]
fn unknown_arguments_are_rejected_without_interactive_session() {
    let out = Command::new(env!("CARGO_BIN_EXE_audio-bridge"))
        .arg("--invalid")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Argument inconnu"));
}
#[test]
#[ignore = "requires interactive Windows audio/Bluetooth environment"]
fn standalone_audio_diagnostic_does_not_need_a_bluetooth_session() {
    let out = Command::new(env!("CARGO_BIN_EXE_audio-bridge"))
        .arg("--diagnose")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Diagnostic audio en lecture seule"), "{text}");
    assert!(text.contains("Rapport local :"), "{text}");
    assert!(!text.contains("Ouverture de session"), "{text}");
}
#[test]
fn help_describes_repair_without_starting_bluetooth_session() {
    let out = Command::new(env!("CARGO_BIN_EXE_audio-bridge"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("repair"));
    assert!(text.contains("connect"));
    assert!(text.contains("--list"));
}
