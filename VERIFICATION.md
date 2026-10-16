# Verification — AudioBridge 0.3 community preview

This report covers local Windows x64 execution and the separately recorded GitHub CI investigation. It is not a Bluetooth stability certification or an iPhone compatibility claim.

## Local build and tests

- Rust formatting was applied and checked.
- `cargo +stable test --locked --all-targets` passed: 22 successful tests, 3 hardware-environment tests explicitly ignored.
- `cargo +stable test --locked --all-targets -- --include-ignored --test-threads=1` passed: all 25 Rust tests, including the actual Windows enumeration/diagnostic environment checks.
- `python scripts/test_export_licenses.py` passed all 4 policy/checkout regressions, including a real Git checkout with `core.autocrlf=true` and a negative control that still detects edited license content; offline license export succeeded for the actual locked dependencies.
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
- Sources were published with the owner's approval to `https://github.com/mustben/AudioBridge`, branch `develop`. The first Windows CI run (38796430697) passed formatting, Rust tests and Clippy, then failed license consistency because Git's Windows checkout changed line endings. That failure was reproduced in a fresh clone. The correction preserves copied license bytes with `.gitattributes` and writes generated notices with explicit LF; the consistency guard remains enabled. The corrected commit `0debb36439bb7acd4cf669978ff9726758773fba` passed [Windows CI run 38798694441](https://github.com/mustben/AudioBridge/actions/runs/38798694441), rechecked through GitHub on 2026-10-16. This result applies only to that commit. No tag or release has been created.

## Final independent review

The final targeted independent review accepted all four previously blocking corrections: trusted Notepad resolution, durable modal control requests, x64/subsystem validation and complete portable documentation links. It identified no remaining blocker in the reviewed scope. This is a code review result, not a remote GitHub CI run or a hardware playback certification.

## Privacy and publication boundary

The source publication set is enumerated with Git's ignore rules and scanned for known private paths/device identifiers and common token formats. No matches were found in the scanned text. This is a bounded check, not a formal security audit. Full desktop screenshots, local diagnostics, logs, executables and build outputs are not included in the source publication set.

The application is an unsigned preview. No auto-start registration, service, certificate trust change, telemetry or updater is implemented. The initial source publication was owner-approved; tags and releases still require separate approval.

## 2026-10-16 review corrections (AB-01–AB-05)

All five findings were confirmed statically against the unchanged baseline `0debb364`; none were claimed reproduced on Bluetooth hardware. See [CORRECTIONS.md](CORRECTIONS.md) for the implementation and test map.

Local Linux validation of this change:

- Rust 1.93.1: `cargo fmt --all -- --check`.
- `cargo check --locked --all-targets --target x86_64-pc-windows-msvc` and Clippy with `-D warnings`: type checking for Windows, not execution of Windows binaries.
- Native standalone Rust test execution of `src/protocol.rs` and `src/logging.rs`: snapshot backlog/control-event preservation and log rotation/retention/foreign-file preservation.
- `python scripts/test_export_licenses.py`: four passing regressions. No dependency or lockfile changes.

The complete Windows suite passed on commit `7a2e8af60f0522c6559d416a016b5a494d7ee467`: [Windows CI run 38875799187](https://github.com/mustben/AudioBridge/actions/runs/38875799187), 30 tests passed and 3 hardware tests intentionally ignored. Formatting, Clippy, dependency license consistency, release compilation and packaging all succeeded. The following documentation/package-manifest update includes `CORRECTIONS.md` in the portable archive; the latest PR CI remains authoritative for that final commit. Hardware playback, repeated state-read failures from a real adapter, disconnect during a real Windows open, sleep/resume, long modal menus and Explorer recreation have not been executed in this Linux session. No measured CPU/RAM benefit is claimed.
