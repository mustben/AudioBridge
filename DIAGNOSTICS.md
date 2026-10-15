# Audio diagnostics and honest interpretation

Start media on the phone and select the PC as its output. If the tray reports an open connection but playback is silent, request a diagnostic from the tray menu. The worker saves a local text report and the UI opens it. No audio recording, volume adjustment, reconnection or output switching is part of the diagnostic.

For a meaningful comparison, collect one report while silent and another when playback is working, with the same source media. Record whether you actually heard the sound and whether any other PC application was playing audio. A delayed return can happen without a reconnect; do not repeatedly destroy the session while investigating it.

## What is measured

The default multimedia render endpoint, its volume/mute state, and aggregate peak-meter readings over roughly 1.4 seconds. Active capture endpoints whose friendly name contains A2DP SNK are reported as candidates only.

## What is NOT established

- No candidate matching A2DP SNK is **not proof of missing Bluetooth audio**. This filter can return zero during working playback.
- A render peak can come from another application. It cannot by itself be attributed to the phone.
- Zero during a short sample is not proof of a fault; the media may be silent or paused.
- Unavailable is not zero. Partial errors are preserved.
- An open connection or nonzero peak is not proof of audible output.
- The tool does not observe Android call state, record samples, or automatically measure the exact recovery delay.

Windows/driver failures and connection lifecycle errors are separate from level observations. Keep their original error codes when reporting. Do not change drivers or disable devices based only on a zero level.

Logs are local under `%LOCALAPPDATA%\AudioBridgeRust\logs`. Review device names and IDs before sharing. GitHub issues must not include passwords, tokens, private identifiers or unreviewed dumps.
