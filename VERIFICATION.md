# Verification — AudioBridge 0.3 community preview

This report covers local Windows x64 execution. It is not a Bluetooth stability certification, an iPhone compatibility claim, or evidence that GitHub CI ran.

## Local build and tests

- Rust formatting was applied and checked.
- `cargo +stable test --locked --all-targets` passed: 22 successful tests, 3 hardware-environment tests explicitly ignored.
- `cargo +stable test --locked --all-targets -- --include-ignored --test-threads=1` passed: all 25 Rust tests, including the actual Windows enumeration/diagnostic environment checks.
- `python scripts/test_export_licenses.py` passed all 3 policy regressions; offline license export succeeded for the actual locked dependencies.
- `cargo +stable clippy --locked --all-targets -- -D warnings` passed.
- Release builds of `audio-bridge-tray` and `audio-bridge` succeeded.

The tray routing tests cover stable opaque device IDs, busy-action exclusion, Quit availability, error-state handling, and selection of the real embedded 32px frame in the multiframe ICO. Backend tests cover startup acknowledgement, single instance, clean shutdown, invalid IDs, busy snapshots and intent preservation. Platform-boundary tests do not pretend to establish real sound.

## Review-driven fixes

- Notepad is resolved from the Windows system-directory API, not CWD/PATH search; the report remains a separate quoted argument.
- Close/restore intentions are durable across nested popup/About message loops and consumed once. A real popup-open then WM_CLOSE test exited 0 without an additional WM_CANCELMODE request; Stopped and NIM_DELETE were recorded.
- Packaging rejects non-x64 PE binaries and wrong GUI/console subsystems even with SkipBuild, and includes the README's referenced contribution/publication documents.
- License policy tests reject unknown obligations and stale copied directories; export uses offline Cargo metadata.

## Native application and package execution

- Release PE headers inspected: tray is an x64 Windows GUI executable (subsystem 2); the console executable is checked as subsystem 3 before packaging.
- A real tray instance registered its notification icon, received its device inventory with no automatic connection, and rendered the actual French native popup menu.
- The menu's observed labels included devices, refresh, disconnect, rebuild connection, optional recovery, diagnostic, logs, Bluetooth settings, About and Quit. Disconnect/rebuild were disabled with no selected connection; recovery was unchecked.
- The screenshot in assets/screenshots/tray-menu.png is a crop of this real menu, visually inspected to contain only application UI, not the surrounding desktop or private device IDs.
- The QA instance was closed gracefully through its window close path. The process exited with code 0; its log confirmed Stopped and NIM_DELETE success. It was not force-killed.
- The generated portable ZIP was extracted into an isolated temporary folder and its own AudioBridge.exe executed with `--smoke-test-ms 1500`. It exited 0; the new log segment confirmed NIM_ADD, inventory received, Stopped and NIM_DELETE.
- Archive CRC, entries, binary identity against release build outputs and SHA256SUMS were verified programmatically.
- The inspected build imports VCRUNTIME140.dll; the official Microsoft x64 v14 runtime requirement is documented. No runtime or driver installer is bundled/executed.

## What remains manual/unverified

- Audible playback through the new tray command path, Android call return, repeated link loss, sleep/resume, long-running stability and iPhone behavior require their own hardware tests. Earlier console playback was user-validated; it is not substituted for these tests.
- The worker's diagnostic save path was executed by tests. Automatic Notepad opening from the native menu was not successfully exercised by the desktop automation and must be checked manually; no successful UI-open result is claimed.
- Explorer/taskbar recreation support is implemented but was not tested by restarting the user's Explorer session.
- No public repository, remote CI run, tag or release was created. The GitHub workflows are prepared definitions only.

## Final independent review

The final targeted independent review accepted all four previously blocking corrections: trusted Notepad resolution, durable modal control requests, x64/subsystem validation and complete portable documentation links. It identified no remaining blocker in the reviewed scope. This is a code review result, not a remote GitHub CI run or a hardware playback certification.

## Privacy and publication boundary

The source publication set is enumerated with Git's ignore rules and scanned for known private paths/device identifiers and common token formats. No matches were found in the scanned text. This is a bounded check, not a formal security audit. Full desktop screenshots, local diagnostics, logs, executables and build outputs are not included in the source publication set.

The application is an unsigned preview. No auto-start registration, service, certificate trust change, telemetry or updater is implemented. Public publication still requires the owner's approval.
