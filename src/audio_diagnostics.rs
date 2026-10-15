//! Lecture seule des endpoints Core Audio. L'appelant doit avoir initialisé COM/WinRT MTA.
use std::{thread, time::Duration};
use windows::{
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Media::Audio::{
            DEVICE_STATE_ACTIVE,
            Endpoints::{IAudioEndpointVolume, IAudioMeterInformation},
            IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eMultimedia, eRender,
        },
        System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ},
    },
    core::Result,
};

fn interpretation() -> &'static str {
    "Limites : le pic sortie peut venir d'une autre application ; zéro ne prouve pas une panne ; un pic n'est pas preuve de son audible. A2DP SNK est un candidat nommé, pas une attribution certaine au téléphone et pas garantie d'exhaustivité. Le volume endpoint capture n'est pas preuve du flux."
}

fn display<T: std::fmt::Display>(value: Result<T>) -> String {
    value.map_or_else(|e| format!("indisponible ({e})"), |v| v.to_string())
}

fn friendly_name(device: &IMMDevice) -> Result<String> {
    // SAFETY: valid COM endpoint; property store opened read-only. PROPVARIANT owns its value.
    unsafe {
        Ok(device
            .OpenPropertyStore(STGM_READ)?
            .GetValue(&PKEY_Device_FriendlyName)?
            .to_string())
    }
}

struct Endpoint {
    label: String,
    meter: Option<IAudioMeterInformation>,
    peak: PeakSummary,
    first_error: Option<String>,
}

fn inspect(device: &IMMDevice, label: String, lines: &mut Vec<String>) -> Endpoint {
    // SAFETY: caller initialized MTA; only getters and interface activation, no stream creation.
    unsafe {
        lines.push(format!("{label} : {}", display(friendly_name(device))));
        let id = device.GetId().and_then(|id| {
            let text = id.to_string();
            CoTaskMemFree(Some(id.0.cast()));
            Ok(text?)
        });
        lines.push(format!("  ID : {}", display(id)));
        match device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
            Ok(volume) => {
                lines.push(format!(
                    "  Volume endpoint (0..1) : {}",
                    display(volume.GetMasterVolumeLevelScalar())
                ));
                lines.push(format!(
                    "  Muet : {}",
                    display(volume.GetMute().map(|v| v.as_bool()))
                ));
            }
            Err(e) => lines.push(format!("  Volume/muet indisponibles : {e}")),
        }
        let (meter, first_error) = match device.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None)
        {
            Ok(meter) => (Some(meter), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Endpoint {
            label,
            meter,
            peak: PeakSummary::default(),
            first_error,
        }
    }
}

/// Diagnostic synchrone en lecture seule (~1,4 s de mesures communes aux endpoints).
/// Précondition : COM/WinRT MTA initialisé sur ce thread par l'appelant.
/// Seul l'échec de création de l'énumérateur est fatal ; les autres erreurs sont des lignes.
pub fn collect() -> Result<Vec<String>> {
    let mut lines = vec![
        "Core Audio — lecture seule, aucun enregistrement du son ni changement de routage.".into(),
        interpretation().into(),
    ];
    // SAFETY: MTA provided by caller. All subsequent calls read endpoint metadata/meters only.
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let mut endpoints = Vec::new();
    match unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) } {
        Ok(device) => endpoints.push(inspect(
            &device,
            "Sortie par défaut eRender/eMultimedia".into(),
            &mut lines,
        )),
        Err(e) => lines.push(format!("Sortie par défaut indisponible : {e}")),
    }
    let captures = (|| -> Result<()> {
        let devices = unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)? };
        let count = unsafe { devices.GetCount()? };
        let mut candidates = 0;
        for index in 0..count {
            match unsafe { devices.Item(index) } {
                Ok(device) => match friendly_name(&device) {
                    Ok(name) if name.to_ascii_uppercase().contains("A2DP SNK") => {
                        candidates += 1;
                        endpoints.push(inspect(
                            &device,
                            format!("Candidat capture A2DP SNK actif #{candidates}"),
                            &mut lines,
                        ));
                    }
                    Ok(_) => {}
                    Err(e) => lines.push(format!(
                        "Capture active #{index} : nom indisponible, filtrage impossible ({e})"
                    )),
                },
                Err(e) => lines.push(format!("Capture active #{index} indisponible : {e}")),
            }
        }
        lines.push(format!(
            "Candidats capture actifs nommés A2DP SNK : {candidates} (filtre de nom uniquement)."
        ));
        Ok(())
    })();
    if let Err(e) = captures {
        lines.push(format!("Énumération capture indisponible : {e}"));
    }
    lines.push(
        "Fenêtre de niveau : 15 lectures espacées de 100 ms ; seul le maximum agrégé est conservé."
            .into(),
    );
    for index in 0..15 {
        if index > 0 {
            thread::sleep(Duration::from_millis(100));
        }
        for endpoint in &mut endpoints {
            if let Some(meter) = &endpoint.meter {
                let value = unsafe { meter.GetPeakValue() }.map_err(|e| e.to_string());
                if let Err(e) = &value {
                    endpoint.first_error.get_or_insert_with(|| e.clone());
                }
                endpoint.peak.record(value);
            }
        }
    }
    for endpoint in endpoints {
        lines.push(format!("{} : {}", endpoint.label, endpoint.peak.describe()));
        if let Some(error) = endpoint.first_error {
            lines.push(format!("  Erreur niveau (première) : {error}"));
        }
    }
    Ok(lines)
}

#[derive(Default)]
struct PeakSummary {
    maximum: Option<f32>,
    successful: usize,
    failed: usize,
}
impl PeakSummary {
    fn record(&mut self, value: std::result::Result<f32, String>) {
        match value {
            Ok(v) if v.is_finite() && (0.0..=1.0).contains(&v) => {
                self.maximum = Some(self.maximum.map_or(v, |old| old.max(v)));
                self.successful += 1;
            }
            _ => self.failed += 1,
        }
    }
    fn describe(&self) -> String {
        let peak = self
            .maximum
            .map_or_else(|| "indisponible".into(), |v| format!("{v:.6}"));
        format!(
            "pic maximal={peak} ; {} mesures valides ; {} échec(s)",
            self.successful, self.failed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interpretation_never_attributes_or_proves_audibility() {
        let text = interpretation();
        for required in [
            "autre application",
            "zéro ne prouve pas",
            "pas preuve de son audible",
            "pas garantie d'exhaustivité",
            "pas preuve du flux",
            "pas une attribution certaine",
        ] {
            assert!(text.contains(required), "limite absente: {required}");
        }
    }
    #[test]
    fn unavailable_is_not_a_measured_zero_and_partial_errors_survive() {
        let mut missing = PeakSummary::default();
        missing.record(Err("API absente".into()));
        assert!(missing.describe().contains("indisponible"));
        assert!(!missing.describe().contains("0.000000"));
        let mut zero = PeakSummary::default();
        zero.record(Ok(0.0));
        zero.record(Err("déconnecté".into()));
        assert!(zero.describe().contains("0.000000"));
        assert!(zero.describe().contains("1 échec"));
    }
}
