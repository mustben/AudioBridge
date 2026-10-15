# Privacy

AudioBridge is a local application. It has no account, telemetry, crash-upload service, update query, or cloud backend. It does not record audio samples or create audio files.

Windows and the Bluetooth source still perform their normal OS-level operations; this document describes AudioBridge's own behavior, not every action of the operating system.

Local session logs and diagnostics are stored under `%LOCALAPPDATA%\AudioBridgeRust\logs`. Reports can include endpoint/device names, opaque identifiers, Windows error messages, timestamps and aggregate audio levels. A level measures activity, not audio content, but can still reveal when playback was active.

Reports open locally at the user's request. Nothing is automatically attached to an issue or uploaded. Review and redact reports before sharing. Do not submit crash dumps or private identifiers in public issues.

This preview does not implement log retention/rotation. Delete old logs manually when no longer needed. There is no automatic startup entry. Quitting stops the application; deleting its local data removes its logs.
