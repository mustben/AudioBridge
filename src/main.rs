use audio_bridge::{HELP, app, diagnostic_report, platform};
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] || args == ["-h"] {
        print!("{HELP}");
        return Ok(());
    }
    if args == ["--diagnose"] {
        return diagnostic_report::standalone();
    }
    if args == ["--list"] {
        let _apartment = platform::Apartment::new().map_err(|e| e.to_string())?;
        let devices = platform::list().map_err(|e| e.to_string())?;
        platform::print_devices(&devices);
        return Ok(());
    }
    if !args.is_empty() {
        return Err("Argument inconnu. Utiliser --help".into());
    }
    app::run()
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Erreur : {e}");
        std::process::exit(1);
    }
}
