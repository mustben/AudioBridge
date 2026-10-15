//! Typed boundary between the native tray UI and the serialized Bluetooth owner.
use std::sync::mpsc::{Receiver, Sender};

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
    pub events: Receiver<BackendEvent>,
}
