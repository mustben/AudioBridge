use audio_bridge::protocol::{BackendCommand, BackendSnapshot};

pub fn run() {
    native::run();
}
fn route(id: u32, s: &BackendSnapshot, closing: bool) -> Option<BackendCommand> {
    if closing {
        return None;
    }
    if id == 9 {
        return Some(BackendCommand::Quit);
    }
    if id == 2 && (s.connected || s.wanted || s.retry_pending || s.session_owned || s.busy) {
        return Some(BackendCommand::Disconnect);
    }
    if s.busy {
        return None;
    }
    match id {
        1 => Some(BackendCommand::Refresh),
        3 if s.selected_id.is_some() => Some(BackendCommand::Repair),
        4 => Some(BackendCommand::SetAuto(!s.auto_recovery)),
        5 => Some(BackendCommand::Diagnose),
        100..=60000 => s
            .devices
            .get((id - 100) as usize)
            .map(|d| BackendCommand::Connect(d.id.clone())),
        _ => None,
    }
}

fn accept_snapshot(current: &mut BackendSnapshot, failed: &mut bool, next: BackendSnapshot) {
    *failed = next.error.is_some();
    *current = next;
}

fn icon_frame(bytes: &[u8]) -> Result<&[u8], String> {
    if bytes.get(..4) != Some(&[0, 0, 1, 0]) {
        return Err("Icône ICO invalide".into());
    }
    let count = bytes
        .get(4..6)
        .map(|v| u16::from_le_bytes([v[0], v[1]]) as usize)
        .ok_or("En-tête ICO tronqué")?;
    let mut best: Option<(u32, &[u8])> = None;
    for i in 0..count {
        let p = 6 + i * 16;
        let entry = bytes.get(p..p + 16).ok_or("Répertoire ICO tronqué")?;
        let len = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let off = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        let width = if entry[0] == 0 { 256 } else { entry[0] as u32 };
        let height = if entry[1] == 0 { 256 } else { entry[1] as u32 };
        let distance = width.abs_diff(32) + height.abs_diff(32);
        if off < 6 + count * 16 || len == 0 {
            continue;
        }
        if let Some(frame) = off.checked_add(len).and_then(|end| bytes.get(off..end)) {
            if best.is_none_or(|(score, _)| distance < score) {
                best = Some((distance, frame));
            }
        }
    }
    best.map(|(_, frame)| frame)
        .ok_or_else(|| "Aucune image ICO valide".into())
}

struct TooltipState {
    last_applied: String,
    retry_at: Option<std::time::Instant>,
}
impl TooltipState {
    fn new(last_applied: String) -> Self {
        Self {
            last_applied,
            retry_at: None,
        }
    }
    // Returns true only for the first failure in an uninterrupted failure streak.
    fn update(
        &mut self,
        desired: &str,
        now: std::time::Instant,
        apply: impl FnOnce(&str) -> bool,
    ) -> bool {
        if desired == self.last_applied || self.retry_at.is_some_and(|deadline| now < deadline) {
            return false;
        }
        if apply(desired) {
            self.last_applied = desired.to_owned();
            self.retry_at = None;
            false
        } else {
            let first_failure = self.retry_at.is_none();
            self.retry_at = Some(now + std::time::Duration::from_secs(1));
            first_failure
        }
    }
}

const CLOSE: u8 = 1;
const RESTORE: u8 = 2;
#[derive(Default)]
struct ControlIntents(std::cell::Cell<u8>);
impl ControlIntents {
    fn request(&self, intent: u8) {
        self.0.set(self.0.get() | intent);
    }
    fn take(&self) -> u8 {
        self.0.replace(0)
    }
}
fn notepad_command(
    system_directory: impl FnOnce() -> Result<String, String>,
    report: &str,
) -> Result<(String, String), String> {
    let directory = std::path::PathBuf::from(system_directory()?);
    if !directory.is_absolute() {
        return Err("Répertoire système Windows non absolu".into());
    }
    if report.contains(['\"', '\0']) || !std::path::Path::new(report).is_absolute() {
        return Err("Chemin de rapport local invalide".into());
    }
    Ok((
        directory.join("notepad.exe").to_string_lossy().into_owned(),
        format!("\"{report}\""),
    ))
}

mod native {
    use super::*;
    use audio_bridge::{app, protocol::BackendEvent};
    use std::{
        path::PathBuf,
        sync::mpsc::TryRecvError,
        time::{Duration, Instant},
    };
    use windows::{
        Win32::{
            Foundation::*,
            System::LibraryLoader::GetModuleHandleW,
            UI::{Shell::*, WindowsAndMessaging::*},
        },
        core::{PCWSTR, w},
    };
    thread_local! {
        // One tray window per UI thread; never borrowed across a Win32 call.
        static INTENTS: ControlIntents = ControlIntents::default();
    }
    fn system_directory() -> Result<String, String> {
        let mut buffer = vec![0u16; 32768];
        let count = unsafe {
            windows::Win32::System::SystemInformation::GetSystemDirectoryW(Some(&mut buffer))
        } as usize;
        if count == 0 || count >= buffer.len() {
            return Err(format!(
                "GetSystemDirectoryW a échoué : {}",
                windows::core::Error::from_win32()
            ));
        }
        String::from_utf16(&buffer[..count])
            .map_err(|e| format!("Répertoire système invalide : {e}"))
    }
    const CALLBACK: u32 = WM_APP + 1;
    static TASKBAR_CREATED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn logs() -> PathBuf {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("AudioBridgeRust")
            .join("logs")
    }
    fn log(s: &str) {
        let dir = logs();
        // A failed disk must not create repeated diagnostics on each UI update.
        static FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if FAILED.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        if let Err(e) = std::fs::create_dir_all(&dir)
            .and_then(|()| audio_bridge::logging::append(&dir.join("tray.log"), s, true))
        {
            FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
            eprintln!("Journal tray: {e}");
        }
    }
    fn error(s: &str) {
        log(s);
        unsafe {
            let text = wide(s);
            MessageBoxW(
                None,
                PCWSTR(text.as_ptr()),
                w!("Audio Bridge"),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    struct Menu(HMENU);
    impl Drop for Menu {
        fn drop(&mut self) {
            unsafe {
                if let Err(e) = DestroyMenu(self.0) {
                    log(&format!("DestroyMenu: {e}"));
                }
            }
        }
    }
    struct Icon(HICON);
    impl Drop for Icon {
        fn drop(&mut self) {
            unsafe {
                if let Err(e) = DestroyIcon(self.0) {
                    log(&format!("DestroyIcon: {e}"));
                }
            }
        }
    }
    struct Window(HWND);
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                let _ = KillTimer(self.0, 1);
                if let Err(e) = DestroyWindow(self.0) {
                    log(&format!("DestroyWindow: {e}"));
                }
            }
        }
    }
    struct Tray(NOTIFYICONDATAW);
    impl Drop for Tray {
        fn drop(&mut self) {
            unsafe {
                if !Shell_NotifyIconW(NIM_DELETE, &self.0).as_bool() {
                    log("NIM_DELETE failed");
                } else {
                    log("NIM_DELETE success");
                }
            }
        }
    }
    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        // No backend/state pointers in the callback: all ownership stays in the message loop.
        unsafe {
            if msg != 0 && msg == TASKBAR_CREATED.load(std::sync::atomic::Ordering::Relaxed) {
                INTENTS.with(|i| i.request(RESTORE));
                return LRESULT(0);
            }
            if msg == CALLBACK {
                if let Err(e) = PostMessageW(hwnd, WM_APP + 4, wp, lp) {
                    log(&format!("Tray callback PostMessageW: {e}"));
                }
                return LRESULT(0);
            }
            if msg == WM_CLOSE {
                INTENTS.with(|i| i.request(CLOSE));
                // Unwind only this thread's menu and this tray's owned dialog.
                let _ = EndMenu();
                let popup = GetLastActivePopup(hwnd);
                if popup != hwnd && GetWindow(popup, GW_OWNER).ok() == Some(hwnd) {
                    let _ = PostMessageW(popup, WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
    fn embedded_icon() -> Result<Icon, String> {
        let frame = icon_frame(include_bytes!("../assets/audio-bridge.ico"))?;
        unsafe { CreateIconFromResourceEx(frame, true, 0x00030000, 32, 32, IMAGE_FLAGS(0)) }
            .map(Icon)
            .map_err(|e| format!("Impossible de décoder l’icône embarquée : {e}"))
    }
    unsafe fn append(
        menu: HMENU,
        id: u32,
        label: &str,
        enabled: bool,
        checked: bool,
    ) -> Result<(), String> {
        let label = wide(&label.replace('&', "&&"));
        let flags = MF_STRING
            | if enabled { MF_ENABLED } else { MF_GRAYED }
            | if checked { MF_CHECKED } else { MF_UNCHECKED };
        unsafe {
            AppendMenuW(menu, flags, id as usize, PCWSTR(label.as_ptr())).map_err(|e| e.to_string())
        }
    }
    fn popup(hwnd: HWND, s: &BackendSnapshot, closing: bool) -> Result<u32, String> {
        unsafe {
            let root = Menu(CreatePopupMenu().map_err(|e| e.to_string())?);
            append(root.0, 0, &s.message, false, false)?;
            let sub = Menu(CreatePopupMenu().map_err(|e| e.to_string())?);
            for (i, d) in s.devices.iter().take(59901).enumerate() {
                append(
                    sub.0,
                    100 + i as u32,
                    &d.name,
                    !closing && !s.busy,
                    s.selected_id.as_ref() == Some(&d.id),
                )?;
            }
            if s.devices.is_empty() {
                append(sub.0, 0, "Aucun appareil trouvé", false, false)?;
            }
            AppendMenuW(
                root.0,
                MF_POPUP
                    | if s.busy || closing {
                        MF_GRAYED
                    } else {
                        MF_ENABLED
                    },
                sub.0.0 as usize,
                w!("Appareils"),
            )
            .map_err(|e| e.to_string())?;
            std::mem::forget(sub); // ownership transferred to root; DestroyMenu recursively releases it.
            for (id, label) in [
                (1, "Actualiser les appareils"),
                (2, "Déconnecter"),
                (3, "Rétablir la connexion"),
                (4, "Récupération automatique"),
                (5, "Diagnostic audio"),
            ] {
                append(
                    root.0,
                    id,
                    label,
                    route(id, s, closing).is_some(),
                    id == 4 && s.auto_recovery,
                )?;
            }
            for (id, label) in [
                (6, "Ouvrir les journaux"),
                (7, "Paramètres Bluetooth Windows"),
                (8, "À propos"),
                (9, "Quitter"),
            ] {
                append(root.0, id, label, !closing, id == 0)?;
            }
            let mut pos = POINT::default();
            GetCursorPos(&mut pos).map_err(|e| e.to_string())?;
            let _ = SetForegroundWindow(hwnd);
            let id = TrackPopupMenu(
                root.0,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                pos.x,
                pos.y,
                0,
                hwnd,
                None,
            )
            .0 as u32;
            let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
            Ok(id)
        }
    }
    fn open(hwnd: HWND, target: &str, params: Option<&str>) {
        let target = wide(target);
        let params = params.map(wide);
        let result = unsafe {
            ShellExecuteW(
                hwnd,
                w!("open"),
                PCWSTR(target.as_ptr()),
                params
                    .as_ref()
                    .map(|v| PCWSTR(v.as_ptr()))
                    .unwrap_or(PCWSTR::null()),
                None,
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            error(&format!(
                "Impossible d’ouvrir la ressource (ShellExecute: {}).",
                result.0 as isize
            ));
        }
    }
    fn tip(s: &BackendSnapshot, closing: bool, failed: bool) -> String {
        let state = if closing {
            "Fermeture en cours"
        } else if failed {
            "Erreur"
        } else if s.busy {
            "En cours"
        } else if s.connected {
            "Connexion établie (audio non vérifié)"
        } else {
            "Libre"
        };
        format!("Audio Bridge — {state}")
    }
    pub fn run() {
        if let Err(e) = execute() {
            error(&e);
        }
    }
    fn execute() -> Result<(), String> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        let smoke = match args.as_slice() {
            [] => None,
            [flag] if flag == "--smoke-test" => Some(Duration::from_secs(4)),
            [flag, n] if flag == "--smoke-test-ms" => Some(Duration::from_millis(
                n.parse::<u64>()
                    .ok()
                    .filter(|n| (500..=30000).contains(n))
                    .ok_or("Durée smoke: 500 à 30000 ms")?,
            )),
            _ => return Err("Options: --smoke-test ou --smoke-test-ms 500..30000".into()),
        };
        INTENTS.with(|i| {
            i.take();
        });
        let icon = embedded_icon()?;
        let instance = unsafe { GetModuleHandleW(None).map_err(|e| e.to_string())? };
        let class = w!("AudioBridge.NativeTray.03");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&wc) } == 0 {
            return Err(format!(
                "RegisterClassW: {}",
                windows::core::Error::from_win32()
            ));
        }
        let window = Window(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!("Audio Bridge"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                instance,
                None,
            )
            .map_err(|e| e.to_string())?
        });
        let hwnd = window.0;
        let mut tray = Tray(NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: CALLBACK,
            hIcon: icon.0,
            ..Default::default()
        });
        let initial = wide("Audio Bridge — Libre");
        tray.0.szTip[..initial.len()].copy_from_slice(&initial);
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &tray.0) }.as_bool() {
            return Err("Échec d’enregistrement de l’icône (NIM_ADD)".into());
        }
        log("NIM_ADD success");
        if unsafe { SetTimer(hwnd, 1, 100, None) } == 0 {
            return Err("SetTimer a échoué".into());
        }
        let taskbar = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        if taskbar == 0 {
            return Err("RegisterWindowMessageW(TaskbarCreated) a échoué".into());
        }
        TASKBAR_CREATED.store(taskbar, std::sync::atomic::Ordering::Relaxed);
        let backend = app::spawn_worker()?;
        backend
            .commands
            .send(BackendCommand::Refresh)
            .map_err(|e| e.to_string())?;
        let mut snapshot = BackendSnapshot {
            busy: true,
            message: "Inventaire en cours…".into(),
            ..Default::default()
        };
        let mut tooltip = TooltipState::new("Audio Bridge — Libre".to_owned());
        let mut closing = false;
        let mut failed = false;
        let started = Instant::now();
        let mut saw_inventory = false;
        let mut msg = MSG::default();
        loop {
            let result = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
            if result == -1 {
                log("GetMessageW failed; requesting graceful worker shutdown");
                let _ = backend.commands.send(BackendCommand::Quit);
                while let Ok(ev) = backend.events.recv() {
                    if matches!(ev, BackendEvent::Stopped) {
                        break;
                    }
                }
                return Err("GetMessageW a échoué".into());
            }
            if result == 0 {
                let _ = backend.commands.send(BackendCommand::Quit);
                while let Ok(ev) = backend.events.recv() {
                    if matches!(ev, BackendEvent::Stopped) {
                        break;
                    }
                }
                break;
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let mut chosen = 0;
            if msg.hwnd == hwnd && msg.message == WM_APP + 4 && msg.lParam.0 as u32 == WM_RBUTTONUP
            {
                match popup(hwnd, &snapshot, closing) {
                    Ok(id) => chosen = id,
                    Err(e) => error(&e),
                }
            }
            let intents = INTENTS.with(ControlIntents::take);
            if intents & CLOSE != 0 {
                chosen = 9;
            }
            if intents & RESTORE != 0 && !unsafe { Shell_NotifyIconW(NIM_ADD, &tray.0) }.as_bool() {
                log("TaskbarCreated NIM_ADD failed");
            }
            if msg.hwnd == hwnd && msg.message == WM_TIMER {
                loop {
                    match backend.events.try_recv() {
                        Ok(BackendEvent::Snapshot(s)) => {
                            if !s.busy {
                                saw_inventory = true;
                            }
                            if s != snapshot {
                                log(&format!(
                                    "Snapshot devices={} connected={} busy={} auto={} message={}",
                                    s.devices.len(),
                                    s.connected,
                                    s.busy,
                                    s.auto_recovery,
                                    s.message
                                ));
                            }
                            accept_snapshot(&mut snapshot, &mut failed, s);
                        }
                        Ok(BackendEvent::Error(e)) => {
                            log(&format!("Worker error: {e}"));
                            snapshot.busy = false;
                            snapshot.message = format!("Erreur : {e}");
                            failed = true;
                        }
                        Ok(BackendEvent::ReportReady { path }) => {
                            log(&format!("ReportReady {path}"));
                            if !closing {
                                if path.contains('"')
                                    || !std::path::Path::new(&path).is_absolute()
                                    || !std::path::Path::new(&path).is_file()
                                {
                                    error("Chemin de rapport local invalide");
                                } else {
                                    match notepad_command(system_directory, &path) {
                                        Ok((exe, argument)) => open(hwnd, &exe, Some(&argument)),
                                        Err(e) => error(&e),
                                    }
                                }
                            }
                        }
                        Ok(BackendEvent::Stopped) => {
                            log(&format!("Stopped inventory_received={saw_inventory}"));
                            return Ok(());
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            return Err("Le worker s’est arrêté sans événement Stopped".into());
                        }
                    }
                }
                if smoke.is_some_and(|duration| started.elapsed() >= duration) && !closing {
                    log(&format!(
                        "Smoke deadline inventory_received={saw_inventory}"
                    ));
                    chosen = 9;
                }
                let next_tip = tip(&snapshot, closing, failed);
                if tooltip.update(&next_tip, Instant::now(), |desired| {
                    // Keep the restore payload consistent with the last successful update.
                    let mut candidate = tray.0;
                    let text = wide(desired);
                    candidate.szTip.fill(0);
                    let count = text.len().min(candidate.szTip.len() - 1);
                    candidate.szTip[..count].copy_from_slice(&text[..count]);
                    let applied = unsafe { Shell_NotifyIconW(NIM_MODIFY, &candidate) }.as_bool();
                    if applied {
                        tray.0 = candidate;
                    }
                    applied
                }) {
                    log("NIM_MODIFY failed; retrying at most once per second");
                }
            }
            if let Some(cmd) = route(chosen, &snapshot, closing) {
                let quit = matches!(cmd, BackendCommand::Quit);
                match backend.commands.send(cmd) {
                    Ok(()) => {
                        closing |= quit;
                        snapshot.busy = true;
                        if quit {
                            snapshot.message = "Fermeture en cours…".into();
                        }
                    }
                    Err(e) => return Err(format!("Envoi worker: {e}")),
                }
            } else if !closing {
                match chosen {
                    6 => {
                        let dir = logs();
                        if let Err(e) = std::fs::create_dir_all(&dir) {
                            error(&e.to_string());
                        } else {
                            open(hwnd, &dir.to_string_lossy(), None);
                        }
                    }
                    7 => open(hwnd, "ms-settings:bluetooth", None),
                    8 => unsafe {
                        MessageBoxW(
                            hwnd,
                            w!(
                                "Audio Bridge 0.3.0\nRéception Bluetooth Windows.\nConnexion établie ne signifie pas audio audible.\nAucune reconnexion sur silence. Récupération automatique désactivée par défaut.\nAucun service ni démarrage automatique."
                            ),
                            w!("À propos"),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    },
                    _ => (),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tooltip_retry_retains_success_and_bounds_attempts_and_error_logs() {
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let mut policy = TooltipState::new("old".into());
        let mut calls = 0;
        {
            let mut apply = |_: &str| {
                calls += 1;
                false
            };
            assert!(policy.update("new", start, &mut apply));
            assert_eq!(
                policy.last_applied, "old",
                "failed update must not be acknowledged"
            );
            assert!(!policy.update("new", start + Duration::from_millis(999), &mut apply));
            assert!(!policy.update("new", start + Duration::from_secs(1), &mut apply));
            assert!(!policy.update("different", start + Duration::from_millis(1500), &mut apply));
        }
        assert_eq!(
            calls, 2,
            "failures retry at most once per second, even if desired changes"
        );
        assert_eq!(policy.last_applied, "old");
        assert!(!policy.update("new", start + Duration::from_secs(2), |_| {
            calls += 1;
            true
        }));
        assert_eq!(policy.last_applied, "new");
        assert!(
            !policy.update("new", start + Duration::from_secs(3), |_| panic!(
                "redundant Shell call"
            ))
        );
        assert_eq!(calls, 3);
        assert!(
            policy.update("next", start + Duration::from_secs(3), |_| false),
            "success resets error suppression"
        );
    }
    #[test]
    fn routing_preserves_opaque_ids_and_blocks_busy_mutations() {
        let mut s = BackendSnapshot {
            devices: vec![audio_bridge::protocol::DeviceInfo {
                id: "opaque&ID\\x".into(),
                name: "Same".into(),
            }],
            ..Default::default()
        };
        assert_eq!(
            route(100, &s, false),
            Some(BackendCommand::Connect("opaque&ID\\x".into()))
        );
        assert_eq!(route(4, &s, false), Some(BackendCommand::SetAuto(true)));
        s.auto_recovery = true;
        assert_eq!(route(4, &s, false), Some(BackendCommand::SetAuto(false)));
        s.busy = true;
        for id in [1, 3, 4, 5, 100] {
            assert_eq!(route(id, &s, false), None);
        }
        assert_eq!(route(9, &s, true), None);
        assert_eq!(route(101, &BackendSnapshot::default(), false), None);
    }
    #[test]
    fn polling_and_unrelated_busy_operations_preserve_authoritative_error() {
        let mut current = BackendSnapshot::default();
        let mut failed = false;
        for busy in [false, true, false, false] {
            accept_snapshot(
                &mut current,
                &mut failed,
                BackendSnapshot {
                    busy,
                    error: Some("connection error".into()),
                    ..Default::default()
                },
            );
            assert!(failed);
        }
        accept_snapshot(
            &mut current,
            &mut failed,
            BackendSnapshot {
                connected: true,
                ..Default::default()
            },
        );
        assert!(!failed);
    }
    #[test]
    fn disconnect_cancels_waiting_recovery_and_can_be_queued_while_busy() {
        for s in [
            BackendSnapshot {
                wanted: true,
                retry_pending: true,
                ..Default::default()
            },
            BackendSnapshot {
                busy: true,
                ..Default::default()
            },
        ] {
            assert_eq!(route(2, &s, false), Some(BackendCommand::Disconnect));
        }
        assert_eq!(route(2, &BackendSnapshot::default(), false), None);
    }
    #[test]
    fn embedded_multiframe_icon_selects_real_32px_frame() {
        let bytes = include_bytes!("../assets/audio-bridge.ico");
        let frame = icon_frame(bytes).unwrap();
        assert_eq!(&frame[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(frame[16..20].try_into().unwrap()), 32);
        assert_eq!(u32::from_be_bytes(frame[20..24].try_into().unwrap()), 32);
        assert!(icon_frame(&bytes[..10]).is_err());
        assert!(icon_frame(&[]).is_err());
    }
    #[test]
    fn control_intents_survive_modal_dispatch_and_clear_once() {
        let intents = ControlIntents::default();
        intents.request(CLOSE);
        intents.request(RESTORE);
        intents.request(CLOSE);
        // A nested modal loop may discard all wake messages; state remains.
        assert_eq!(intents.take(), CLOSE | RESTORE);
        assert_eq!(intents.take(), 0);
        intents.request(RESTORE);
        assert_eq!(intents.take(), RESTORE);
    }
    #[test]
    fn notepad_uses_absolute_api_directory_and_separate_quoted_argument() {
        let (exe, arg) =
            notepad_command(|| Ok(r"C:\Windows\System32".into()), r"C:\reports\a b.txt").unwrap();
        assert_eq!(exe, r"C:\Windows\System32\notepad.exe");
        assert_eq!(arg, "\"C:\\reports\\a b.txt\"");
        assert!(notepad_command(|| Ok("relative".into()), r"C:\report.txt").is_err());
        assert!(notepad_command(|| Err("API failure".into()), r"C:\report.txt").is_err());
        assert!(notepad_command(|| Ok(r"C:\Windows".into()), "C:\\bad\".txt").is_err());
    }
    #[test]
    fn quit_is_available_while_busy() {
        let s = BackendSnapshot {
            busy: true,
            ..Default::default()
        };
        assert_eq!(route(9, &s, false), Some(BackendCommand::Quit));
    }
}
