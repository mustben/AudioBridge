# AudioBridge 0.3 — early community preview

A portable Rust Windows receiver controlled from a notification-area icon and right-click menu. Includes a separate CLI, local diagnostics, documentation and dependency license texts.

## Important limits

- Windows x64 build; normal use does not require administrator privileges.
- Unsigned development binary. No certificate installation or security bypass is required by the project.
- Uses Microsoft's AudioPlaybackConnection API and the installed Bluetooth driver.
- Connected-but-silent and delayed resume remain possible. Open state and meter readings do not prove audible output.
- Automatic recovery is off at startup and bounded when enabled; no recovery triggered merely by silence.
- No microphone/call transport, output selector, auto-start, background service, updater or telemetry.
- Android playback has local user evidence; iPhone and broad adapter compatibility are not verified.

Review the build/test report and perform hardware acceptance before describing a specific release as stable. This file is input for a draft prerelease, not evidence that anything has been published.
