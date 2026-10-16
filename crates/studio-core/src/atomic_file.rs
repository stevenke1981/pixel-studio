//! Commit output only after the complete write has succeeded.
use std::io::Write;
use std::path::Path;

/// Stage in the destination directory so persist can atomically replace an old file.
/// The extension is retained for encoders that infer their output format from it.
pub fn write_with<E: From<std::io::Error>>(
    output: &Path,
    write: impl FnOnce(&Path) -> Result<(), E>,
) -> Result<(), E> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let suffix = output
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    let tmp = tempfile::Builder::new()
        .prefix(".pixel-studio-")
        .suffix(&suffix)
        .tempfile_in(parent)?;
    write(tmp.path())?;
    // A path-based encoder may itself replace the staged file. Sync the file
    // currently at that path, rather than the original tempfile handle.
    std::fs::OpenOptions::new()
        .write(true)
        .open(tmp.path())?
        .sync_all()?;
    tmp.persist(output).map_err(|e| e.error)?;
    Ok(())
}

pub fn write(output: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_with(output, |path| {
        let mut file = std::fs::File::create(path)?;
        file.write_all(bytes)?;
        file.flush()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_writer_preserves_existing_file_and_cleans_staging() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("project.pixel-project");
        std::fs::write(&out, b"original").unwrap();
        let result: std::io::Result<()> = write_with(&out, |temp| {
            std::fs::write(temp, b"partial")?;
            Err(std::io::Error::other("injected write failure"))
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&out).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        write(&out, b"complete").unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"complete");
    }
}
