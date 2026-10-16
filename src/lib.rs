pub mod app;
pub mod audio_diagnostics;
pub mod diagnostic_report;
pub mod logging;
pub mod platform;
pub mod protocol;
pub const HELP: &str = include_str!("help.txt");

/// Owns a created session until explicit closure succeeds.
pub struct SessionSlot<T>(pub Option<T>);
impl<T> SessionSlot<T> {
    pub fn open<E>(
        &mut self,
        create: impl FnOnce() -> Result<T, E>,
        open: impl FnOnce(&mut T) -> Result<(), E>,
    ) -> Result<(), E> {
        assert!(self.0.is_none(), "session already owned");
        self.0 = Some(create()?);
        open(self.0.as_mut().expect("created session"))
    }
    pub fn release<E>(&mut self, close: impl FnOnce(&mut T) -> Result<(), E>) -> Result<(), E> {
        if let Some(session) = self.0.as_mut() {
            close(session)?;
        }
        self.0 = None;
        Ok(())
    }
}

#[derive(Default)]
pub struct Policy {
    pub generation: u64,
    pub wanted: bool,
    pub busy: bool,
}
impl Policy {
    pub fn begin(&mut self) -> Option<u64> {
        if self.busy {
            return None;
        }
        self.generation += 1;
        self.wanted = true;
        self.busy = true;
        Some(self.generation)
    }
    pub fn disconnect(&mut self) {
        self.generation += 1;
        self.wanted = false;
        self.busy = false;
    }
    pub fn accepts(&self, generation: u64) -> bool {
        self.generation == generation
    }
}
#[derive(Default)]
pub struct Recovery {
    attempts: usize,
}
impl Recovery {
    pub fn next_delay(&mut self) -> Option<std::time::Duration> {
        let seconds = *[2, 4, 8].get(self.attempts)?;
        self.attempts += 1;
        Some(std::time::Duration::from_secs(seconds))
    }
    pub fn reset(&mut self) {
        self.attempts = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_is_spaced_bounded_and_can_be_explicitly_reset() {
        let mut r = Recovery::default();
        for seconds in [2, 4, 8] {
            assert_eq!(
                r.next_delay(),
                Some(std::time::Duration::from_secs(seconds))
            );
        }
        assert_eq!(r.next_delay(), None);
        assert_eq!(r.next_delay(), None);
        r.reset();
        assert_eq!(r.next_delay(), Some(std::time::Duration::from_secs(2)));
    }
    #[test]
    fn disconnect_invalidates_events_and_stops_desired_connection() {
        let mut p = Policy::default();
        let old = p.begin().unwrap();
        p.disconnect();
        assert!(!p.accepts(old));
        assert!(!p.wanted);
        assert!(!p.busy);
        assert!(p.begin().unwrap() > old);
    }
    #[test]
    fn admits_one_connection_and_rejects_overlap() {
        let mut p = Policy::default();
        assert_eq!(p.begin(), Some(1));
        assert_eq!(p.begin(), None);
        assert!(p.wanted);
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    #[test]
    fn aborted_open_retains_session_until_close_is_confirmed() {
        let mut slot = SessionSlot(None);
        assert_eq!(
            slot.open(|| Ok(7), |_| Err("start/open")),
            Err("start/open")
        );
        assert_eq!(slot.0, Some(7), "aborted opening must remain owned");
        assert_eq!(slot.release(|_| Err("close")), Err("close"));
        assert_eq!(slot.0, Some(7));
        let mut created = false;
        let blocked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = slot.open(
                || {
                    created = true;
                    Ok::<_, &str>(8)
                },
                |_| Ok(()),
            );
        }));
        assert!(blocked.is_err());
        assert!(!created, "no new creation before confirmed close");
        assert_eq!(slot.0, Some(7));
        assert_eq!(slot.release(|_| Ok::<_, &str>(())), Ok(()));
        assert_eq!(slot.0, None);
        assert_eq!(slot.open(|| Ok::<_, &str>(8), |_| Ok(())), Ok(()));
        assert_eq!(slot.0, Some(8));
    }
}
