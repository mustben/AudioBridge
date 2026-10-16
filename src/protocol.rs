//! Typed boundary between the native tray UI and the serialized Bluetooth owner.
use std::sync::mpsc::Sender;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackendSnapshot {
    pub devices: Vec<DeviceInfo>,
    pub selected_id: Option<String>,
    pub selected_name: Option<String>,
    pub connected: bool,
    pub busy: bool,
    pub wanted: bool,
    pub retry_pending: bool,
    pub session_owned: bool,
    /// Authoritative error, cleared only by confirmed connection/disconnection.
    pub error: Option<String>,
    pub auto_recovery: bool,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendCommand {
    Refresh,
    Connect(String),
    Disconnect,
    Repair,
    SetAuto(bool),
    Diagnose,
    Quit,
}

#[derive(Clone, Debug)]
pub enum BackendEvent {
    Snapshot(BackendSnapshot),
    ReportReady { path: String },
    Error(String),
    Stopped,
}

pub struct BackendHandle {
    pub commands: Sender<BackendCommand>,
    pub events: EventReceiver,
}

// One replaceable snapshot plus lossless ordered control events. A bounded wake
// channel never blocks the session owner, including while the UI is modal.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
type Queue = Arc<Mutex<VecDeque<BackendEvent>>>;
pub struct EventSender {
    queue: Queue,
    wake: mpsc::SyncSender<()>,
}
pub struct EventReceiver {
    queue: Queue,
    wake: mpsc::Receiver<()>,
}
pub fn event_channel() -> (EventSender, EventReceiver) {
    let queue = Queue::default();
    let (tx, rx) = mpsc::sync_channel(1);
    (
        EventSender {
            queue: queue.clone(),
            wake: tx,
        },
        EventReceiver { queue, wake: rx },
    )
}
impl EventSender {
    pub fn send(&self, event: BackendEvent) -> Result<(), mpsc::SendError<()>> {
        let mut queue = self.queue.lock().expect("event queue poisoned");
        if matches!(event, BackendEvent::Snapshot(_)) {
            queue.retain(|e| !matches!(e, BackendEvent::Snapshot(_)));
        }
        queue.push_back(event);
        match self.wake.try_send(()) {
            Ok(()) | Err(mpsc::TrySendError::Full(())) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(())) => Err(mpsc::SendError(())),
        }
    }
}
impl EventReceiver {
    pub fn try_recv(&self) -> Result<BackendEvent, mpsc::TryRecvError> {
        if let Some(event) = self.queue.lock().expect("event queue poisoned").pop_front() {
            return Ok(event);
        }
        match self.wake.try_recv() {
            Err(mpsc::TryRecvError::Disconnected) => Err(mpsc::TryRecvError::Disconnected),
            _ => self
                .queue
                .lock()
                .expect("event queue poisoned")
                .pop_front()
                .ok_or(mpsc::TryRecvError::Empty),
        }
    }
    pub fn recv(&self) -> Result<BackendEvent, mpsc::RecvError> {
        loop {
            match self.try_recv() {
                Ok(event) => return Ok(event),
                Err(mpsc::TryRecvError::Disconnected) => return Err(mpsc::RecvError),
                Err(mpsc::TryRecvError::Empty) => self.wake.recv()?,
            }
        }
    }
    pub fn recv_timeout(&self, timeout: Duration) -> Result<BackendEvent, mpsc::RecvTimeoutError> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.try_recv() {
                Ok(event) => return Ok(event),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(mpsc::RecvTimeoutError::Disconnected);
                }
                Err(mpsc::TryRecvError::Empty) => self
                    .wake
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))?,
            }
        }
    }
    pub fn try_iter(&self) -> impl Iterator<Item = BackendEvent> + '_ {
        std::iter::from_fn(|| self.try_recv().ok())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waiting_receiver_wakes_and_timeout_does_not_lose_shutdown() {
        let (tx, rx) = event_channel();
        assert!(matches!(
            rx.recv_timeout(Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        let worker = std::thread::spawn(move || {
            tx.send(BackendEvent::Snapshot(BackendSnapshot::default()))
                .unwrap();
            tx.send(BackendEvent::Stopped).unwrap();
        });
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            BackendEvent::Snapshot(_)
        ));
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            BackendEvent::Stopped
        ));
        worker.join().unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
    #[test]
    fn missing_consumer_never_blocks_sender() {
        let (tx, rx) = event_channel();
        drop(rx);
        assert!(tx.send(BackendEvent::Stopped).is_err());
    }
    #[test]
    fn suspended_consumer_keeps_latest_state_and_every_control_event() {
        let (tx, rx) = event_channel();
        for n in 0..100_000 {
            tx.send(BackendEvent::Snapshot(BackendSnapshot {
                message: n.to_string(),
                ..Default::default()
            }))
            .unwrap();
            if n == 1 {
                tx.send(BackendEvent::Error("failure".into())).unwrap();
            }
            if n == 2 {
                tx.send(BackendEvent::ReportReady {
                    path: "report".into(),
                })
                .unwrap();
            }
        }
        assert_eq!(tx.queue.lock().unwrap().len(), 3);
        tx.send(BackendEvent::Stopped).unwrap();
        drop(tx);
        assert!(matches!(rx.recv().unwrap(), BackendEvent::Error(_)));
        assert!(matches!(
            rx.recv().unwrap(),
            BackendEvent::ReportReady { .. }
        ));
        assert!(matches!(rx.recv().unwrap(), BackendEvent::Snapshot(s) if s.message == "99999"));
        assert!(matches!(rx.recv().unwrap(), BackendEvent::Stopped));
        assert!(rx.recv().is_err());
    }
}
