# Changelog

## 0.3.0 — community preview

- Native Windows notification-area application with a right-click control menu.
- A serialized backend worker keeps Bluetooth and diagnostics off the UI thread.
- Typed device IDs and snapshots separate menu selection from device enumeration order.
- Optional console application retained for troubleshooting.
- Local Core Audio diagnostic retained with explicit interpretation limits.
- Portable packaging, dependency license texts, community documentation and Windows CI/draft-release definitions.

The tray UI does not replace the Windows Bluetooth stack. Audible playback, phone call recovery and long-term stability must be tested separately; delayed or silent resume can still occur. No automatic startup, service, telemetry or updater.

## 0.2.0 — local diagnostic prototype

Added endpoint volume/mute and aggregate peak-level diagnostics. An A2DP SNK name filter was useful for discovery but is not a reliable presence/absence test for a working Bluetooth stream.

## 0.1.0 — local console prototype

Implemented supported WinRT device enumeration, session lifecycle, explicit repair, optional bounded recovery and local logs. Initial Android playback was user-validated; no broad stability or compatibility guarantee.
