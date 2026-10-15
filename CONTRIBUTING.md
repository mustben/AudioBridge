# Contributing

Use small changes with an explicit rationale and regression tests. Keep the Bluetooth owner serialized and keep all Windows UI work on the UI thread. A responsiveness improvement must not silently change connection/recovery policy.

## Windows development

Rust MSVC and Visual C++ Build Tools are required. The repository pins the toolchain in rust-toolchain.toml. Build both the tray application and the optional CLI:

```powershell
cargo fmt --all -- --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release --bin audio-bridge-tray --bin audio-bridge
./scripts/package.ps1 -SkipBuild
```

The default suite is intended for headless Windows CI. Tests explicitly marked ignored require an interactive Windows audio/Bluetooth environment. Close other AudioBridge instances before hardware-dependent tests because the single-instance mutex is deliberate.

```powershell
cargo test --locked --all-targets -- --include-ignored --test-threads=1
```

Do not run hardware connection tests against somebody else's active phone/session. Unit tests may inject fake platform boundaries but must never present their results as real Bluetooth evidence.

## Manual acceptance checklist

- Launch the tray binary: icon appears; no console window or background service.
- Right-click: status and device/actions menu appears; refresh does not freeze the UI.
- Connect a paired source with consent; confirm audible playback separately from connection state.
- Disconnect intentionally; optional recovery does not fight that choice.
- Enable recovery explicitly and verify the bounded retry policy on a controlled loss.
- While a Windows operation runs, the menu remains responsive and duplicate mutations are disabled.
- Request a diagnostic; verify local file creation/opening and no audio recording.
- Open logs and Bluetooth settings from the menu.
- Quit: session closes, tray icon is deleted and process exits. Do not force-kill merely to pass the test.
- Explorer/taskbar recreation should re-register the icon; test separately if practical.
- Sleep/resume, Android call return, output changes, and iPhone require separate hardware validation. Do not infer them from a smoke test.

## Safety and privacy

No auto-start registration, service, driver installer, certificate trust changes, telemetry or updater without a separately reviewed design. Do not reconnect solely because a level meter reads zero. Review logs before sharing; never commit diagnostics or personal device IDs.

Use develop for ongoing work and reviewed pull requests for shared changes. Local tests and a workflow definition are not proof that GitHub CI passed. Document unavailable test environments instead of fabricating output.

## Dependencies and licensing

Cargo.lock is committed. When dependencies change, review their license metadata, run `cargo fetch --locked` separately if required, then update the copies with `python scripts/export_licenses.py`. The exporter runs Cargo metadata offline, rejects unsupported license expressions and flags obsolete license directories for manual review. It copies actual MIT license texts and the additional Unicode license from cached Cargo packages; it is not a legal compatibility scanner for arbitrary new dependencies. Run `python scripts/test_export_licenses.py` for its policy regressions. CI checks that regenerated notices/license files match the committed set.
