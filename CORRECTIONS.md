# Review corrections — 2026-10-16

Baseline: `develop`, `0debb36439bb7acd4cf669978ff9726758773fba`. All five paths were confirmed statically on the baseline; hardware reproductions were unavailable. No finding was discarded.

| ID | Change and files | Regression evidence |
| --- | --- | --- |
| AB-01 | `protocol.rs`, `app.rs`, `tray.rs`: expose wanted/retry/owned state; allow Disconnect during recovery or busy operations; preserve command priority and callback generation invalidation. | Routing during busy/retry; cancel pending recovery without changing automatic preference; obsolete notice rejected. |
| AB-02 | `platform.rs`, `app.rs`: callback carries only generation; one authoritative typed state read; first failure removes connected confirmation, three periodic failures close before optional bounded recovery. Failed close blocks further automatic observation/creation until explicit user release. Console shares observation policy and drains queued commands before recovery. | Injected transient/persistent read results, no unwanted recovery, existing failed-close/retained-owner test. |
| AB-03 | `protocol.rs`, `app.rs`: nonblocking single-slot wake channel and queue with at most one snapshot; other events stay ordered. | 100,000 snapshots with a suspended consumer retain only the latest plus Error/ReportReady/Stopped. Receiver timeout/disconnection tests cover waiting and shutdown. |
| AB-04 | `logging.rs`, `app.rs`, `tray.rs`: stable polling emits no busy transition; identical snapshots are not logged; tooltip modified on visible change only; TaskbarCreated still re-adds icon. Logs capped at 1 MiB each, five session logs and two tray segments; logging failure disables its writer after one report. | Real temporary filesystem test for rotation, retention, UTF-8 truncation, foreign-file preservation and inaccessible path. Long modal menus/Explorer restoration still require Windows manual checks. |
| AB-05 | `protocol.rs`, `app.rs`, `tray.rs`: authoritative error in snapshots, independent of busy; only confirmed connection/disconnection resolves operation errors, confirmed Opened resolves state-read error. | Multiple poll/busy snapshots retain error; confirmed resolution clears it; unrelated diagnostic error survives a valid state read. |

## Limits and invariants

- Cancellation cannot interrupt a synchronous Windows call; Disconnect is queued and handled before the next automatic retry.
- Recovery remains off at startup with 2/4/8-second delays and no silence-triggered retry. Failed close retains the sole session owner.
- Only periodic polls (and explicit console status checks) count toward the three-read failure threshold; callback storms do not accelerate recovery.
- Control-event storage is lossless, not artificially capped: the bound concerns repeated snapshots, and repeated state failures are deduplicated. Arbitrarily many explicit user actions can still enqueue control events. A modal UI consumes events after returning to its normal timer loop.
- Retention matches only exact numeric session filenames. Old recognized logs may be removed on startup; foreign files and diagnostic reports are untouched. Session log rollover discards its previous segment; tray retains one previous segment. Access failures are handled without a repeated log-error loop.
- No dependency, telemetry, service, autostart, release or merge is added.

## Validation

See `VERIFICATION.md` for actual local checks and the PR CI for native Windows tests/build/package. Unit injection tests do not constitute Bluetooth hardware evidence. Remaining manual cases: audible audio, controlled loss and cancellation, unavailable Bluetooth, suspend/resume, extended modal windows, closing during open, Explorer recreation.
