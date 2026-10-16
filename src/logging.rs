//! Bounded local logs. Only exact application-owned filenames are retained/pruned.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
pub const MAX_BYTES: u64 = 1024 * 1024;
const SESSION_FILES: usize = 5;

pub fn prune_sessions(root: &Path) -> io::Result<()> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stem) = name
            .strip_prefix("session-")
            .and_then(|s| s.strip_suffix(".log"))
        else {
            continue;
        };
        let Some((stamp, pid)) = stem.split_once('-') else {
            continue;
        };
        if stamp.is_empty()
            || pid.is_empty()
            || !stamp.bytes().all(|c| c.is_ascii_digit())
            || !pid.bytes().all(|c| c.is_ascii_digit())
        {
            continue;
        }
        let metadata = entry.metadata()?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        files.push((
            stamp.parse::<u128>().unwrap_or(0),
            entry.path(),
            metadata.len(),
        ));
    }
    files.sort_by_key(|(stamp, _, _)| std::cmp::Reverse(*stamp));
    for (index, (_, path, bytes)) in files.into_iter().enumerate() {
        if index >= SESSION_FILES || bytes > MAX_BYTES {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// A session keeps its newest segment; tray keeps one previous segment as well.
/// Oversized individual entries are truncated at a UTF-8 boundary.
pub fn append(path: &Path, text: &str, backup: bool) -> io::Result<()> {
    let mut end = text.len().min(MAX_BYTES as usize - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let bytes = end as u64 + 1;
    let current_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let previous = path.with_extension("log.1");
    if backup && fs::metadata(&previous).is_ok_and(|m| m.len() > MAX_BYTES) {
        fs::remove_file(&previous)?;
    }
    if current_bytes + bytes > MAX_BYTES {
        if backup {
            let previous = path.with_extension("log.1");
            match fs::remove_file(&previous) {
                Ok(()) => (),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (),
                Err(e) => return Err(e),
            }
            if current_bytes > MAX_BYTES {
                fs::remove_file(path)?;
            } else {
                fs::rename(path, previous)?;
            }
        } else {
            fs::File::create(path)?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", &text[..end])
}

pub fn session_path(root: &Path, stamp: u128, pid: u32) -> PathBuf {
    root.join(format!("session-{stamp}-{pid}.log"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotation_retention_and_foreign_files() {
        let root = std::env::temp_dir().join(format!(
            "ab-retention-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        for n in 0..9 {
            fs::write(session_path(&root, n, 7), "session").unwrap();
        }
        let foreign = root.join("session-personal-notes.log");
        fs::write(&foreign, "preserve").unwrap();
        prune_sessions(&root).unwrap();
        assert_eq!(fs::read_dir(&root).unwrap().count(), 6);
        assert_eq!(fs::read_to_string(&foreign).unwrap(), "preserve");
        let tray = root.join("tray.log");
        for _ in 0..4 {
            append(&tray, &"é".repeat(MAX_BYTES as usize), true).unwrap();
        }
        assert!(fs::metadata(&tray).unwrap().len() <= MAX_BYTES);
        assert!(fs::metadata(tray.with_extension("log.1")).unwrap().len() <= MAX_BYTES);
        let session = session_path(&root, 8, 7);
        append(&session, &"x".repeat(MAX_BYTES as usize), false).unwrap();
        append(&session, "latest", false).unwrap();
        assert_eq!(fs::read_to_string(session).unwrap(), "latest\n");
        // Legacy oversized tray segments must not defeat the new bound.
        fs::write(&tray, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        fs::write(
            tray.with_extension("log.1"),
            vec![b'x'; MAX_BYTES as usize + 1],
        )
        .unwrap();
        append(&tray, "migrated", true).unwrap();
        assert_eq!(fs::read_to_string(&tray).unwrap(), "migrated\n");
        assert!(!tray.with_extension("log.1").exists());
        assert!(append(&root.join("absent/log"), "failure", false).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
