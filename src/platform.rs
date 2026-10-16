use std::{
    thread,
    time::{Duration, Instant},
};
use windows::{
    Devices::Enumeration::DeviceInformation,
    Foundation::{AsyncStatus, IAsyncInfo, IAsyncOperation},
    Media::Audio::AudioPlaybackConnection,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::{Error, HRESULT, HSTRING, Interface, Result, RuntimeType},
};

pub struct Apartment(std::marker::PhantomData<std::rc::Rc<()>>);
impl Apartment {
    pub fn new() -> Result<Self> {
        // SAFETY: this thread owns one balanced MTA initialization.
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }
        Ok(Self(std::marker::PhantomData))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}
#[derive(Clone)]
pub struct Device {
    pub id: HSTRING,
    pub name: String,
}

pub fn wait(info: &IAsyncInfo, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match info.Status()? {
            AsyncStatus::Completed => return Ok(()),
            AsyncStatus::Error => return Err(Error::from_hresult(info.ErrorCode()?)),
            AsyncStatus::Canceled => {
                return Err(Error::from_hresult(HRESULT(0x800704C7u32 as i32)));
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            let _ = info.Cancel();
            return Err(Error::new(
                HRESULT(0x800705B4u32 as i32),
                "Delai depasse ; annulation demandee",
            ));
        }
        thread::sleep(Duration::from_millis(40));
    }
}
pub fn wait_operation<T: RuntimeType + 'static>(op: &IAsyncOperation<T>) -> Result<T> {
    wait(&op.cast()?, Duration::from_secs(15))?;
    op.GetResults()
}
pub fn list() -> Result<Vec<Device>> {
    let selector = AudioPlaybackConnection::GetDeviceSelector()?;
    let items = wait_operation(&DeviceInformation::FindAllAsyncAqsFilter(&selector)?)?;
    let mut result = Vec::new();
    for index in 0..items.Size()? {
        let item = items.GetAt(index)?;
        result.push(Device {
            id: item.Id()?,
            name: item.Name()?.to_string(),
        });
    }
    result.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.id.to_string().cmp(&b.id.to_string()))
    });
    Ok(result)
}
pub struct Instance(windows::Win32::Foundation::HANDLE);
impl Instance {
    pub fn acquire() -> Result<Self> {
        use windows::{
            Win32::{
                Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError},
                System::Threading::CreateMutexW,
            },
            core::w,
        };
        // A named, unowned mutex is retained for this process lifetime only.
        unsafe {
            let handle = CreateMutexW(None, false, w!("Local\\AudioBridgeRustPrototype_0_1"))?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return Err(Error::new(
                    HRESULT(0x800700B7u32 as i32),
                    "AudioBridge est deja ouvert dans cette session Windows",
                ));
            }
            Ok(Self(handle))
        }
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[derive(Debug)]
pub struct Notice {
    pub generation: u64,
}

pub struct Session {
    connection: AudioPlaybackConnection,
    token: Option<windows::Foundation::EventRegistrationToken>,
}
impl Session {
    pub fn create(
        id: &HSTRING,
        generation: u64,
        tx: std::sync::mpsc::Sender<Notice>,
    ) -> Result<Self> {
        use windows::Foundation::TypedEventHandler;
        let connection = AudioPlaybackConnection::TryCreateFromId(id)?;
        let mut session = Self {
            connection,
            token: None,
        };
        session.token = Some(session.connection.StateChanged(&TypedEventHandler::new(
            move |sender: &Option<AudioPlaybackConnection>, _| {
                // Notification is only a hint; the owner performs one authoritative read.
                if sender.is_some() {
                    let _ = tx.send(Notice { generation });
                }
                Ok(())
            },
        ))?);
        Ok(session)
    }
    pub fn open(&mut self) -> Result<()> {
        use windows::Media::Audio::AudioPlaybackConnectionOpenResultStatus;
        let start = self.connection.StartAsync()?;
        wait(&start.cast()?, Duration::from_secs(15))?;
        start.GetResults()?;
        let result = wait_operation(&self.connection.OpenAsync()?)?;
        match result.Status()? {
            AudioPlaybackConnectionOpenResultStatus::Success => {
                validate_open_state(self.connection.State()?)
            }
            status => {
                let code = result.ExtendedError()?;
                Err(Error::new(
                    if code.is_err() {
                        code
                    } else {
                        HRESULT(0x80004005u32 as i32)
                    },
                    format!("Ouverture refusee : statut {}, code {code:?}", status.0),
                ))
            }
        }
    }
    pub fn state(&self) -> Result<windows::Media::Audio::AudioPlaybackConnectionState> {
        self.connection.State()
    }
    pub fn close(&mut self) -> Result<()> {
        if let Some(token) = self.token.take() {
            let _ = self.connection.RemoveStateChanged(token);
        }
        self.connection.Close()
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn validate_open_state(state: windows::Media::Audio::AudioPlaybackConnectionState) -> Result<()> {
    if state == windows::Media::Audio::AudioPlaybackConnectionState::Opened {
        Ok(())
    } else {
        Err(Error::new(
            HRESULT(0x80004005u32 as i32),
            format!("Ouverture non confirmee : etat Windows {}", state.0),
        ))
    }
}

pub fn print_devices(devices: &[Device]) {
    println!("{} source(s) compatible(s)", devices.len());
    for (n, d) in devices.iter().enumerate() {
        println!("  {}. {}", n + 1, d.name);
    }
    if devices.is_empty() {
        println!("Appairer le telephone dans les parametres Bluetooth Windows, puis taper list.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Media::Audio::AudioPlaybackConnectionState;
    #[test]
    fn success_result_with_closed_state_is_not_an_open_session() {
        assert!(validate_open_state(AudioPlaybackConnectionState::Closed).is_err());
        assert!(validate_open_state(AudioPlaybackConnectionState::Opened).is_ok());
        assert!(validate_open_state(AudioPlaybackConnectionState(42)).is_err());
    }
}
