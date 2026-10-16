use crate::platform::{self, Device, Notice, Session};
use crate::protocol::{
    BackendCommand, BackendEvent, BackendHandle, BackendSnapshot, DeviceInfo, EventSender,
    event_channel,
};
use crate::{Policy, Recovery, SessionSlot};
use std::{
    fs::{self, File},
    io::{self, BufRead},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Log {
    file: PathBuf,
    #[cfg(test)]
    cleanup_root: Option<PathBuf>,
    log_failed: bool,
    console: bool,
    message: String,
    error: Option<String>,
}
impl Log {
    fn new() -> Result<Self, String> {
        Self::create(false)
    }
    fn create(console: bool) -> Result<Self, String> {
        let root = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA absent")?)
            .join("AudioBridgeRust")
            .join("logs");
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path = crate::logging::session_path(&root, stamp, std::process::id());
        File::create(&path).map_err(|e| e.to_string())?;
        crate::logging::prune_sessions(&root).map_err(|e| e.to_string())?;
        if console {
            println!("Journal local : {}", path.display());
        }
        Ok(Self {
            file: path,
            #[cfg(test)]
            cleanup_root: None,
            log_failed: false,
            console,
            message: String::new(),
            error: None,
        })
    }
    fn note(&mut self, text: &str) {
        self.message = text.to_owned();
        if self.console {
            println!("{text}");
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        if self.log_failed {
            return;
        }
        if let Err(e) = crate::logging::append(&self.file, &format!("{stamp} {text}"), false) {
            self.log_failed = true;
            self.error = Some(format!("Journal indisponible : {e}"));
            if self.console {
                eprintln!("Journal indisponible : {e}");
            }
        }
    }
}
#[cfg(test)]
impl Drop for Log {
    fn drop(&mut self) {
        if let Some(root) = &self.cleanup_root {
            let _ = fs::remove_dir_all(root);
        }
    }
}

struct App {
    devices: Vec<Device>,
    connected: bool,
    target: Option<Device>,
    session: SessionSlot<Session>,
    policy: Policy,
    recovery: Recovery,
    active_error: Option<String>,
    state_failures: u8,
    close_blocked: bool,
    automatic: bool,
    retry_at: Option<Instant>,
    tx: mpsc::Sender<Notice>,
    log: Log,
}
impl App {
    fn snapshot(&self, busy: bool, events: &EventSender) {
        let _ = events.send(BackendEvent::Snapshot(BackendSnapshot {
            devices: self
                .devices
                .iter()
                .map(|d| DeviceInfo {
                    id: d.id.to_string(),
                    name: d.name.clone(),
                })
                .collect(),
            selected_id: self.target.as_ref().map(|d| d.id.to_string()),
            selected_name: self.target.as_ref().map(|d| d.name.clone()),
            connected: self.connected,
            wanted: self.policy.wanted,
            retry_pending: self.retry_at.is_some(),
            session_owned: self.session.0.is_some(),
            error: self.active_error.clone(),
            busy,
            auto_recovery: self.automatic,
            message: self.log.message.clone(),
        }));
    }
    fn flush_error(&mut self, events: &EventSender) {
        if let Some(error) = self.log.error.take() {
            self.active_error = Some(error.clone());
            let _ = events.send(BackendEvent::Error(error));
        }
    }
    fn backend_command(&mut self, command: BackendCommand, events: &EventSender) -> bool {
        self.snapshot(true, events);
        let mut running = true;
        match command {
            BackendCommand::Refresh => match platform::list() {
                Ok(devices) => {
                    self.devices = devices;
                    self.log.note("Inventaire actualise");
                }
                Err(e) => {
                    self.log.error = Some(format!("Enumeration impossible : {e}"));
                }
            },
            BackendCommand::Connect(id) => {
                if let Some(device) = self.devices.iter().find(|d| d.id == id).cloned() {
                    if self.session.0.is_some() {
                        self.log.error = Some(
                            "Session existante : deconnecter avant de changer d'appareil.".into(),
                        );
                    } else {
                        self.target = Some(device);
                        self.recovery.reset();
                        self.connect();
                    }
                } else {
                    self.log.error =
                        Some("Appareil absent de l'inventaire. Actualiser la liste.".into());
                }
            }
            BackendCommand::Disconnect => {
                self.command("disconnect");
            }
            BackendCommand::Repair => {
                if self.target.is_none() {
                    self.log.error = Some(
                        "Aucun appareil selectionne. Actualiser puis choisir un appareil.".into(),
                    );
                } else {
                    self.command("repair");
                }
            }
            BackendCommand::SetAuto(on) => {
                self.command(if on { "auto on" } else { "auto off" });
            }
            BackendCommand::Diagnose => match crate::diagnostic_report::save() {
                Ok(path) => {
                    let _ = events.send(BackendEvent::ReportReady {
                        path: path.to_string_lossy().into_owned(),
                    });
                    self.log.note("Rapport audio enregistre");
                }
                Err(e) => self.log.error = Some(e),
            },
            BackendCommand::Quit => {
                self.release(true);
                running = false;
            }
        }
        if let Some(error) = &self.log.error {
            self.log.message = error.clone();
        }
        self.flush_error(events);
        self.snapshot(false, events);
        running
    }

    fn release(&mut self, voluntary: bool) -> bool {
        self.retry_at = None;
        self.close_blocked = false;
        self.connected = false;
        self.state_failures = 0;
        // Invalidate callbacks before calling into Windows; callbacks never mutate app state.
        self.policy.disconnect();
        if !voluntary {
            self.policy.wanted = true;
        }
        if let Err(e) = self.session.release(Session::close) {
            self.close_blocked = true;
            self.log.note(&format!(
                "Fermeture en echec : {e}. Aucune nouvelle session ne sera creee."
            ));
            self.log.error = Some(self.log.message.clone());
            return false;
        }
        if voluntary {
            self.active_error = None;
        }
        true
    }
    fn schedule(&mut self) {
        if !self.automatic || !self.policy.wanted {
            return;
        }
        if let Some(delay) = self.recovery.next_delay() {
            self.retry_at = Some(Instant::now() + delay);
            self.log
                .note(&format!("Recuperation prevue dans {} s", delay.as_secs()));
        } else {
            self.log
                .note("Limite de recuperation atteinte. Taper repair pour reessayer.");
        }
    }
    fn connect(&mut self) {
        if self.session.0.is_some() {
            self.log
                .note("Session existante : utiliser repair ou disconnect.");
            return;
        }
        let Some(target) = self.target.clone() else {
            self.log
                .note("Aucun appareil selectionne. Taper list puis connect N.");
            return;
        };
        let Some(generation) = self.policy.begin() else {
            self.log.note("Operation deja en cours.");
            return;
        };
        self.retry_at = None;
        self.log
            .note(&format!("Ouverture de session, generation {generation}"));
        match self.session.open(
            || Session::create(&target.id, generation, self.tx.clone()),
            Session::open,
        ) {
            Ok(()) => {
                self.connected = true;
                self.state_failures = 0;
                self.active_error = None;
                self.log.note(
                    "Session ouverte. Etat de connexion uniquement : son audible non verifie.",
                );
            }
            Err(e) => {
                self.policy.busy = false;
                self.log.note(&format!("Echec de connexion : {e}"));
                self.log.error = Some(self.log.message.clone());
                if self.release(false) {
                    self.schedule();
                }
            }
        }
    }
    fn command(&mut self, command: &str) -> bool {
        let parts: Vec<_> = command.split_whitespace().collect();
        match parts.as_slice() {
            [] => {}
            ["quit" | "exit"] => return false,
            ["help"] => print!("{}", crate::HELP),
            ["list"] => match platform::list() {
                Ok(d) => {
                    self.devices = d;
                    platform::print_devices(&self.devices);
                }
                Err(e) => self.log.note(&format!("Enumeration impossible : {e}")),
            },
            ["connect", index] => {
                let device = index
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|n| self.devices.get(n))
                    .cloned();
                if let Some(device) = device {
                    if self.session.0.is_some() {
                        self.log.note("Session existante : utiliser repair ou disconnect avant de changer d'appareil.");
                    } else {
                        self.target = Some(device);
                        self.recovery.reset();
                        self.connect();
                    }
                } else {
                    self.log.note("Numero invalide. Taper list puis connect N.");
                }
            }
            ["repair"] => {
                if self.target.is_none() {
                    self.log
                        .note("Aucun appareil selectionne. Taper list puis connect N.");
                } else if self.release(false) {
                    self.recovery.reset();
                    // Let Windows release the previous session before creating another.
                    std::thread::sleep(Duration::from_millis(700));
                    self.connect();
                }
            }
            ["disconnect"] => {
                if self.release(true) {
                    self.log
                        .note("Deconnexion volontaire. Aucune recuperation automatique.");
                }
            }
            ["auto", "on"] => {
                self.automatic = true;
                self.log.note(
                    "Recuperation automatique : activee (3 essais maximum par demande utilisateur)",
                );
                if self.session.0.is_none() && self.retry_at.is_none() {
                    self.schedule();
                }
            }
            ["auto", "off"] => {
                self.automatic = false;
                self.retry_at = None;
                self.log.note("Recuperation automatique : desactivee");
            }
            ["diagnose"] => match crate::diagnostic_report::lines() {
                Ok(lines) => {
                    for line in lines {
                        self.log.note(&line);
                    }
                }
                Err(error) => self.log.note(&error),
            },
            ["status"] => {
                self.poll_state(true);
                let status = if self.close_blocked {
                    "Fermeture non confirmee ; intervention utilisateur necessaire"
                } else if self.connected {
                    "Etat Windows : ouvert. Son audible non verifie."
                } else if self.session.0.is_some() {
                    "Etat Windows indisponible ; connexion non confirmee"
                } else {
                    "Aucune session"
                };
                self.log.note(status);
                self.log.note(&format!(
                    "Auto : {} ; recuperation en attente : {}",
                    self.automatic,
                    self.retry_at.is_some()
                ));
            }
            _ => self.log.note("Commande inconnue. Taper help."),
        }
        true
    }
    fn observe(&mut self, notice: Notice) {
        if self.policy.accepts(notice.generation) {
            // Stale notifications cannot override the current Windows state.
            self.poll_state(false);
        }
    }
    fn poll_state(&mut self, count_failure: bool) {
        // A failed close requires explicit user action; never loop close/create.
        if self.close_blocked {
            return;
        }
        let Some(session) = &self.session.0 else {
            return;
        };
        let state = session.state().map_err(|e| e.to_string());
        self.apply_state(state, count_failure);
    }
    fn apply_state(
        &mut self,
        state: Result<windows::Media::Audio::AudioPlaybackConnectionState, String>,
        count_failure: bool,
    ) {
        use windows::Media::Audio::AudioPlaybackConnectionState as State;
        match state {
            Ok(State::Opened) => {
                self.state_failures = 0;
                self.connected = true;
                // Only a state-read error is resolved by a successful observation.
                if self
                    .active_error
                    .as_ref()
                    .is_some_and(|e| e.starts_with("Etat Windows indisponible"))
                {
                    self.active_error = None;
                    self.log
                        .note("Etat Windows confirme ouvert. Son audible non verifie.");
                }
            }
            Ok(State::Closed) => {
                self.state_failures = 0;
                self.connected = false;
                self.log.note("Fermeture inattendue signalee par Windows");
                let wanted = self.policy.wanted;
                if self.release(!wanted) && wanted {
                    self.schedule();
                }
            }
            other => {
                self.connected = false;
                let detail = match other {
                    Err(error) => error,
                    Ok(state) => format!("etat inconnu {}", state.0),
                };
                if self.state_failures == 0
                    && !self
                        .active_error
                        .as_ref()
                        .is_some_and(|e| e.starts_with("Etat Windows indisponible"))
                {
                    let error = format!("Etat Windows indisponible : {detail}");
                    self.log.note(&error);
                    self.log.error = Some(error.clone());
                    self.active_error = Some(error);
                }
                if count_failure {
                    self.state_failures = self.state_failures.saturating_add(1);
                    // Three consecutive periodic failures: close before bounded recovery.
                    if self.state_failures == 3 && self.policy.wanted && self.release(false) {
                        self.schedule();
                    }
                }
            }
        }
    }
}

/// Starts the serialized Bluetooth owner. Startup acknowledgement never enumerates devices.
pub fn spawn_worker() -> Result<BackendHandle, String> {
    let (commands, input) = mpsc::channel();
    let (events, output) = event_channel();
    let (ready, startup) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("AudioBridge backend".into())
        .spawn(move || {
            let initialized = (|| {
                let apartment = platform::Apartment::new().map_err(|e| e.to_string())?;
                let instance = platform::Instance::acquire().map_err(|e| e.to_string())?;
                let (tx, notices) = mpsc::channel();
                let app = App {
                    devices: vec![],
                    connected: false,
                    target: None,
                    session: SessionSlot(None),
                    policy: Policy::default(),
                    recovery: Recovery::default(),
                    active_error: None,
                    state_failures: 0,
                    close_blocked: false,
                    automatic: false,
                    retry_at: None,
                    tx,
                    log: Log::new()?,
                };
                Ok::<_, String>((apartment, instance, notices, app))
            })();
            let (_apartment, _instance, notices, mut app) = match initialized {
                Ok(value) => value,
                Err(error) => {
                    let _ = ready.send(Err(error));
                    return;
                }
            };
            if ready.send(Ok(())).is_err() {
                return;
            }
            app.log.note("Pret. Recuperation automatique desactivee.");
            app.snapshot(false, &events);
            let mut next_poll = Instant::now();
            loop {
                match input.recv_timeout(Duration::from_millis(100)) {
                    Ok(command) => {
                        if !app.backend_command(command, &events) {
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        app.snapshot(true, &events);
                        app.release(true);
                        app.flush_error(&events);
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                // Drain queued user intent before attempting automatic recovery.
                if !input
                    .try_iter()
                    .all(|command| app.backend_command(command, &events))
                {
                    break;
                }
                for notice in notices.try_iter() {
                    app.observe(notice);
                    app.flush_error(&events);
                    app.snapshot(false, &events);
                }
                if Instant::now() >= next_poll {
                    next_poll = Instant::now() + Duration::from_secs(1);
                    if app.session.0.is_some() {
                        app.poll_state(true);
                        app.flush_error(&events);
                        app.snapshot(false, &events);
                    }
                }
                if app.retry_at.is_some_and(|when| Instant::now() >= when) {
                    app.snapshot(true, &events);
                    app.connect();
                    app.flush_error(&events);
                    app.snapshot(false, &events);
                }
            }
            drop(app);
            drop(_instance);
            drop(_apartment);
            let _ = events.send(BackendEvent::Stopped);
        })
        .map_err(|e| format!("Demarrage backend impossible : {e}"))?;
    startup
        .recv()
        .map_err(|e| format!("Backend interrompu au demarrage : {e}"))??;
    Ok(BackendHandle {
        commands,
        events: output,
    })
}

pub fn run() -> Result<(), String> {
    let _instance = platform::Instance::acquire().map_err(|e| e.to_string())?;
    let _apartment = platform::Apartment::new().map_err(|e| e.to_string())?;
    let (event_tx, event_rx) = mpsc::channel();
    let (input_tx, input_rx) = mpsc::channel();
    let mut app = App {
        devices: Vec::new(),
        connected: false,
        target: None,
        session: SessionSlot(None),
        policy: Policy::default(),
        recovery: Recovery::default(),
        active_error: None,
        state_failures: 0,
        close_blocked: false,
        automatic: false,
        retry_at: None,
        tx: event_tx,
        log: Log::create(true)?,
    };
    print!("{}", crate::HELP);
    app.log
        .note("Pret. Taper list puis connect N. Recuperation automatique desactivee.");
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if input_tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let mut next_poll = Instant::now();
    'console: loop {
        // User commands take precedence over pending automatic recovery.
        match input_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(command) => {
                if !app.command(&command) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        for command in input_rx.try_iter() {
            if !app.command(&command) {
                break 'console;
            }
        }
        for notice in event_rx.try_iter() {
            app.observe(notice);
        }
        if Instant::now() >= next_poll {
            next_poll = Instant::now() + Duration::from_secs(1);
            app.poll_state(true);
        }
        if app.retry_at.is_some_and(|when| Instant::now() >= when) {
            app.connect();
        }
    }
    if !app.release(true) {
        return Err("La fermeture Windows a echoue ; arret du processus.".into());
    }
    app.log.note("Arret propre");
    Ok(())
}

#[cfg(test)]
mod backend_tests {
    use super::*;
    #[test]
    fn unknown_id_is_rejected_without_changing_intent_and_finishes_idle() {
        let mut app = test_app();
        let (tx, rx) = event_channel();
        assert!(app.backend_command(
            crate::protocol::BackendCommand::Connect("stale-id".into()),
            &tx
        ));
        let events: Vec<_> = rx.try_iter().collect();
        assert!(
            matches!(events.last().unwrap(), crate::protocol::BackendEvent::Snapshot(s) if !s.busy && !s.connected && s.selected_id.is_none())
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, crate::protocol::BackendEvent::Error(_)))
        );
        assert!(!app.policy.wanted);
        assert_eq!(app.policy.generation, 0);
    }
    #[test]
    fn repair_without_target_reports_error_without_changing_intent() {
        let mut app = test_app();
        let (tx, rx) = event_channel();
        app.backend_command(BackendCommand::Repair, &tx);
        assert!(rx.try_iter().any(|e| matches!(e, BackendEvent::Error(_))));
        assert!(!app.policy.wanted);
        assert_eq!(app.policy.generation, 0);
        assert!(app.target.is_none());
        assert!(!app.automatic);
    }
    #[test]
    fn worker_startup_ack_quit_and_single_instance_without_bluetooth() {
        static WORKER_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = WORKER_TEST.lock().unwrap();
        let handle = spawn_worker().expect("worker startup");
        let first = handle.events.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            matches!(first, BackendEvent::Snapshot(s) if s.devices.is_empty() && !s.connected && !s.auto_recovery && !s.busy)
        );
        assert!(spawn_worker().is_err(), "second owner must fail startup");
        handle.commands.send(BackendCommand::Quit).unwrap();
        let mut stopped = false;
        while let Ok(event) = handle.events.recv_timeout(Duration::from_secs(5)) {
            if matches!(event, BackendEvent::Stopped) {
                stopped = true;
            }
        }
        assert!(stopped);
        let next = spawn_worker().expect("mutex released after shutdown");
        next.commands.send(BackendCommand::Quit).unwrap();
        while next.events.recv_timeout(Duration::from_secs(5)).is_ok() {}
    }
    #[test]
    fn auto_and_repair_commands_never_invent_connection_intent() {
        let mut app = test_app();
        let (tx, _) = event_channel();
        for command in [
            BackendCommand::SetAuto(true),
            BackendCommand::SetAuto(false),
            BackendCommand::Repair,
        ] {
            assert!(app.backend_command(command, &tx));
            assert!(!app.policy.wanted);
            assert_eq!(app.policy.generation, 0);
            assert!(app.retry_at.is_none());
            assert!(!app.connected);
        }
    }
    #[test]
    fn cancel_pending_recovery_invalidates_callbacks_without_toggling_preference() {
        let mut app = test_app();
        app.automatic = true;
        let old = app.policy.begin().unwrap();
        app.schedule();
        assert!(app.retry_at.is_some());
        let (tx, rx) = event_channel();
        app.backend_command(BackendCommand::Disconnect, &tx);
        assert!(app.retry_at.is_none());
        assert!(!app.policy.wanted);
        assert!(app.automatic);
        app.observe(Notice { generation: old });
        assert!(!app.connected);
        assert!(!app.policy.wanted);
        let snapshot = rx
            .try_iter()
            .find_map(|event| match event {
                BackendEvent::Snapshot(s) => Some(s),
                _ => None,
            })
            .unwrap();
        assert!(!snapshot.wanted && !snapshot.retry_pending);
    }
    #[test]
    fn transient_state_failure_is_visible_then_resolved_without_recovery() {
        use windows::Media::Audio::AudioPlaybackConnectionState as State;
        let mut app = test_app();
        app.policy.begin();
        app.connected = true;
        app.automatic = true;
        app.apply_state(Err("read failed".into()), true);
        assert!(!app.connected);
        let (tx, _) = event_channel();
        app.flush_error(&tx);
        assert!(app.active_error.is_some());
        app.apply_state(Ok(State::Opened), true);
        assert!(app.connected);
        assert!(app.active_error.is_none());
        assert!(app.retry_at.is_none());
        app.active_error = Some("diagnostic failure".into());
        app.apply_state(Ok(State::Opened), true);
        assert_eq!(app.active_error.as_deref(), Some("diagnostic failure"));
    }
    #[test]
    fn persistent_failures_close_before_bounded_recovery_and_respect_user_policy() {
        for automatic in [false, true] {
            let mut app = test_app();
            app.policy.begin();
            app.automatic = automatic;
            for n in 1..=3 {
                app.apply_state(Err("read failed".into()), true);
                if n < 3 {
                    assert!(app.retry_at.is_none());
                }
            }
            assert_eq!(app.retry_at.is_some(), automatic);
            assert!(!app.connected);
            assert!(app.session.0.is_none());
            assert!(app.policy.wanted);
        }
        let mut app = test_app();
        app.automatic = true;
        for _ in 0..10 {
            app.apply_state(Err("read failed".into()), true);
        }
        assert!(app.retry_at.is_none());
        assert!(!app.policy.wanted);
        app.apply_state(
            Ok(windows::Media::Audio::AudioPlaybackConnectionState::Closed),
            true,
        );
        assert!(!app.policy.wanted);
        assert!(app.retry_at.is_none());
    }
    fn test_app() -> App {
        let (tx, _) = mpsc::channel();
        App {
            devices: vec![],
            connected: false,
            target: None,
            session: SessionSlot(None),
            policy: Policy::default(),
            recovery: Recovery::default(),
            active_error: None,
            state_failures: 0,
            close_blocked: false,
            automatic: false,
            retry_at: None,
            tx,
            log: {
                // Each fixture owns a private root: production retention must not
                // remove another concurrently running test's open log on Windows.
                let root = std::env::temp_dir().join(format!(
                    "audio-bridge-fixture-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                fs::create_dir_all(&root).unwrap();
                let file = root.join("test.log");
                File::create(&file).unwrap();
                Log {
                    file,
                    cleanup_root: Some(root),
                    log_failed: false,
                    console: false,
                    message: String::new(),
                    error: None,
                }
            },
        }
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    #[ignore = "requires interactive Windows audio/Bluetooth environment"]
    fn diagnostic_command_logs_report_without_changing_connection_intent() {
        let _apartment = platform::Apartment::new().unwrap();
        let root = std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!(
                "audio-diagnostic-unit-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("diagnostic.log");
        let (tx, _rx) = mpsc::channel();
        let mut app = App {
            devices: Vec::new(),
            connected: false,
            target: None,
            session: SessionSlot(None),
            policy: Policy::default(),
            recovery: Recovery::default(),
            active_error: None,
            state_failures: 0,
            close_blocked: false,
            automatic: false,
            retry_at: None,
            tx,
            log: Log {
                cleanup_root: None,
                file: {
                    File::create(&path).unwrap();
                    path.clone()
                },
                log_failed: false,
                console: false,
                message: String::new(),
                error: None,
            },
        };
        assert!(app.command("diagnose"));
        let (events, received) = event_channel();
        app.backend_command(BackendCommand::Diagnose, &events);
        let emitted: Vec<_> = received.try_iter().collect();
        let report_path = emitted
            .iter()
            .find_map(|e| match e {
                BackendEvent::ReportReady { path } => Some(PathBuf::from(path)),
                _ => None,
            })
            .expect("saved report event");
        assert!(report_path.is_absolute());
        assert!(
            fs::read_to_string(&report_path)
                .unwrap()
                .contains("Diagnostic audio en lecture seule")
        );
        fs::remove_file(report_path).unwrap();
        assert!(
            matches!(emitted.last().unwrap(), BackendEvent::Snapshot(s) if !s.busy && !s.connected)
        );
        let report = fs::read_to_string(&path).unwrap();
        assert!(
            report.contains("Diagnostic audio en lecture seule"),
            "{report}"
        );
        assert!(app.session.0.is_none());
        assert!(!app.policy.wanted);
        assert!(!app.automatic);
        assert!(app.retry_at.is_none());
        assert_eq!(app.policy.generation, 0);
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }
}
