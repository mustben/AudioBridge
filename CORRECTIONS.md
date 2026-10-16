# Review corrections — 2026-10-16

Baseline: `develop`, `0debb36439bb7acd4cf669978ff9726758773fba`. All five paths were confirmed statically on the baseline; hardware reproductions were unavailable. No finding was discarded.

| ID | Change and files | Regression evidence |
| --- | --- | --- |
| AB-01 | `protocol.rs`, `app.rs`, `tray.rs`: expose wanted/retry/owned state; allow Disconnect during recovery or busy operations; preserve command priority and callback generation invalidation. | Routing during busy/retry; cancel pending recovery without changing automatic preference; obsolete notice rejected. |
| AB-02 | `platform.rs`, `app.rs`: callback carries only generation; one authoritative typed state read; first failure removes connected confirmation, three periodic failures close before optional bounded recovery. Failed close blocks further automatic observation/creation until explicit user release. Console shares observation policy and drains queued commands before recovery. | Injected transient/persistent read results, no unwanted recovery, existing failed-close/retained-owner test. |
| AB-03 | `protocol.rs`, `app.rs`: nonblocking single-slot wake channel and queue with at most one snapshot; other events stay ordered. | 100,000 snapshots with a suspended consumer retain only the latest plus Error/ReportReady/Stopped. Receiver timeout/disconnection tests cover waiting and shutdown. |
| AB-04 | `logging.rs`, `app.rs`, `tray.rs`: stable polling emits no busy transition; identical snapshots are not logged; tooltip updates skip already-applied stable states and retry failures on a bounded timer; TaskbarCreated still re-adds icon. Logs capped at 1 MiB each, five session logs and two tray segments; logging failure disables its writer after one report. | Real temporary filesystem test for rotation, retention, UTF-8 truncation, foreign-file preservation and inaccessible path. Long modal menus/Explorer restoration still require Windows manual checks. |
| AB-05 | `protocol.rs`, `app.rs`, `tray.rs`: authoritative error in snapshots, independent of busy; only confirmed connection/disconnection resolves operation errors, confirmed Opened resolves state-read error. | Multiple poll/busy snapshots retain error; confirmed resolution clears it; unrelated diagnostic error survives a valid state read. |

## Limits and invariants

- Cancellation cannot interrupt a synchronous Windows call; Disconnect is queued and handled before the next automatic retry.
- Recovery remains off at startup with 2/4/8-second delays and no silence-triggered retry. Failed close retains the sole session owner.
- Only periodic polls (and explicit console status checks) count toward the three-read failure threshold; callback storms do not accelerate recovery.
- Control-event storage is lossless, not artificially capped: the bound concerns repeated snapshots, and repeated state failures are deduplicated. Arbitrarily many explicit user actions can still enqueue control events. A modal UI consumes events after returning to its normal timer loop.
- Retention matches only exact numeric session filenames. Old recognized logs may be removed on startup; foreign files and diagnostic reports are untouched. Session log rollover discards its previous segment; tray retains one previous segment. Access failures are handled without a repeated log-error loop.
- No dependency, telemetry, service, autostart, release or merge is added.

## Follow-up review corrections (R1–R3)

The AB-01–AB-05 implementation and original commits are preserved. The following corrections address the independent review of `66e72e09`:

| ID | Correction | Regression coverage |
| --- | --- | --- |
| R1 | An old log that cannot be removed does not prevent creation of a working backend. Cleanup failure is surfaced once rather than treated as fatal audio initialization. Failure to create the current log is still an error. | A Windows file handle denying delete sharing on a recognized old session log, with the current log writable. |
| R2 | Retention reserves a place for the active session and excludes it from deletion, even if other filenames have future timestamps. | Future-timestamp fixtures plus a real append; the current log remains and the normal session count is at most five. No system clock changes. |
| R3 | Tooltip state acknowledges only successful shell updates. Failed updates are retried on a bounded timer; stable successful updates and repeated identical failure messages are suppressed. | Pure policy coverage of failure, deferred retry, success and unchanged state; no deliberate GUI failure on the user's desktop. |

When permissions or sharing prevent old logs from being deleted, the normal total storage target cannot be guaranteed. Audio startup remains available if its current log can be created and written. Hardware playback and long-running Windows UI tests remain separate from these regressions.

## Validation

See `VERIFICATION.md` for actual local checks and the PR CI for native Windows tests/build/package. Unit injection tests do not constitute Bluetooth hardware evidence. Remaining manual cases: audible audio, controlled loss and cancellation, unavailable Bluetooth, suspend/resume, extended modal windows, closing during open, Explorer recreation.
