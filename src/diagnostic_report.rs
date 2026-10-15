use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn lines() -> Result<Vec<String>, String> {
    let mut lines = vec![
        "Diagnostic audio en lecture seule — aucun son enregistre, aucun reglage modifie"
            .to_owned(),
        "Les niveaux mesures ne prouvent ni le son audible ni la cause du retard.".to_owned(),
    ];
    lines.extend(
        crate::audio_diagnostics::collect()
            .map_err(|e| format!("Diagnostic Core Audio indisponible : {e}"))?,
    );
    Ok(lines)
}

pub fn standalone() -> Result<(), String> {
    let _apartment = crate::platform::Apartment::new().map_err(|e| e.to_string())?;
    let path = save()?;
    print!("{}", fs::read_to_string(&path).map_err(|e| e.to_string())?);
    println!("Rapport local : {}", path.display());
    Ok(())
}

/// Saves a read-only report; caller owns the Windows apartment.
pub fn save() -> Result<PathBuf, String> {
    let lines = lines()?;
    let root = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA absent")?)
        .join("AudioBridgeRust/logs");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = root.join(format!("diagnostic-{stamp}-{}.txt", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    for line in lines {
        writeln!(file, "{stamp} {line}").map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(path)
}
