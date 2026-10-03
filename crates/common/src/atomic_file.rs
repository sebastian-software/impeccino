//! Same-directory atomic file replacement for persisted runtime state.
//!
//! The caller creates the parent directory. Existing regular files are
//! replaced atomically, their basic permissions are retained, and symlink
//! destinations are rejected so a write cannot silently replace the link
//! instead of its target. A failed write removes its temporary file.

use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Atomically write `contents` to `path`, replacing an existing regular file.
///
/// The temporary file is created beside the destination, so the final rename
/// stays on one filesystem. Symlinks and non-file destinations are rejected.
/// On POSIX, a new file is created with owner-only permissions and an
/// existing destination's permission bits are copied to the replacement. On
/// Windows, a new file inherits its directory ACL and `ReplaceFileW` retains
/// an existing destination's ACL and attributes.
pub fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_with(path, contents, replace)
}

fn write_with(
    path: &Path,
    contents: &[u8],
    replace_file: impl FnOnce(&Path, &Path, Option<&Metadata>) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let _ = inspect_target(path)?;

    let (temp, mut file) = create_temp(parent)?;
    let protected_metadata = inspect_target(path)?;
    protect_temp(&temp.path, path, protected_metadata.as_ref())?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);

    let target_metadata = inspect_target(path)?;
    #[cfg(not(windows))]
    if let Some(metadata) = &target_metadata {
        fs::set_permissions(&temp.path, metadata.permissions())?;
    }

    replace_file(&temp.path, path, target_metadata.as_ref())?;
    temp.keep();
    Ok(())
}

fn inspect_target(path: &Path) -> io::Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("refusing to atomically replace symlink {}", path.display()),
                ));
            }
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("refusing to atomically replace non-file {}", path.display()),
                ));
            }
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn create_temp(parent: &Path) -> io::Result<(TempFile, File)> {
    for _ in 0..128 {
        let mut name = OsString::from(".impeccino-atomic-");
        name.push(std::process::id().to_string());
        name.push("-");
        name.push(TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed).to_string());
        name.push(".tmp");
        let path = parent.join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((TempFile { path, keep: false }, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique atomic-write temporary file",
    ))
}

#[cfg(unix)]
fn protect_temp(temp: &Path, _destination: &Path, existing: Option<&Metadata>) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    // Keep secrets out of the temp file while it is populated. If the
    // existing destination is more restrictive than 0600, preserve that
    // restriction too; the already-open write handle remains usable.
    let mode = existing
        .map(|metadata| metadata.mode() & 0o600)
        .unwrap_or(0o600);
    fs::set_permissions(temp, fs::Permissions::from_mode(mode))
}

#[cfg(windows)]
fn protect_temp(temp: &Path, destination: &Path, existing: Option<&Metadata>) -> io::Result<()> {
    if existing.is_none() {
        return Ok(());
    }

    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::{
        GetFileSecurityW, SetFileSecurityW, DACL_SECURITY_INFORMATION,
    };

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let destination_wide = wide(destination);
    let temp_wide = wide(temp);
    let mut needed = 0u32;
    let first = unsafe {
        GetFileSecurityW(
            destination_wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    if first != 0 || needed == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut descriptor = vec![0u8; needed as usize];
    let read = unsafe {
        GetFileSecurityW(
            destination_wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            descriptor.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    let applied = unsafe {
        SetFileSecurityW(
            temp_wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            descriptor.as_mut_ptr().cast(),
        )
    };
    if applied == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

struct TempFile {
    path: PathBuf,
    keep: bool,
}

impl TempFile {
    fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(not(windows))]
fn replace(temp: &Path, destination: &Path, _existing: Option<&Metadata>) -> io::Result<()> {
    fs::rename(temp, destination)
}

#[cfg(windows)]
fn replace(temp: &Path, destination: &Path, existing: Option<&Metadata>) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_WRITE_THROUGH,
    };

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let temp_wide = wide(temp);
    let destination_wide = wide(destination);
    let result = unsafe {
        if existing.is_some() {
            ReplaceFileW(
                destination_wide.as_ptr(),
                temp_wide.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        } else {
            MoveFileExW(
                temp_wide.as_ptr(),
                destination_wide.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        }
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "impeccino-atomic-file-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }

        fn assert_no_temporary_files(&self) {
            assert!(fs::read_dir(&self.0).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".impeccino-atomic-")
            }));
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn creates_and_replaces_a_file_without_leaving_temporary_files() {
        let dir = TempDir::new();
        let path = dir.path("state.json");

        write(&path, b"first").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first");
        write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        dir.assert_no_temporary_files();
    }

    #[cfg(unix)]
    #[test]
    fn replacement_preserves_existing_permission_bits() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new();
        let path = dir.path("state.json");
        fs::write(&path, b"old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

        write(&path, b"new").unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(fs::read(&path).unwrap(), b"new");
        dir.assert_no_temporary_files();
    }

    #[cfg(unix)]
    #[test]
    fn temporary_file_is_private_before_content_is_written() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let dir = TempDir::new();
        let destination = dir.path("settings.json");
        fs::write(&destination, b"secret").unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o400)).unwrap();
        let metadata = inspect_target(&destination).unwrap();
        let (temp, mut file) = create_temp(&dir.0).unwrap();

        protect_temp(&temp.path, &destination, metadata.as_ref()).unwrap();

        assert_eq!(fs::metadata(&temp.path).unwrap().mode() & 0o777, 0o400);
        file.write_all(b"replacement secret").unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert_eq!(fs::read(&temp.path).unwrap(), b"replacement secret");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_destination_without_changing_it_or_its_target() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new();
        let target = dir.path("target.json");
        let link = dir.path("state.json");
        fs::write(&target, b"target contents").unwrap();
        symlink(&target, &link).unwrap();

        let error = write(&link, b"replacement").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&target).unwrap(), b"target contents");
        dir.assert_no_temporary_files();
    }

    #[test]
    fn failure_to_create_the_temporary_file_keeps_the_destination_absent() {
        let dir = TempDir::new();
        let path = dir.path("missing/state.json");

        assert!(write(&path, b"contents").is_err());
        assert!(!path.exists());
        dir.assert_no_temporary_files();
    }

    #[test]
    fn replacement_failure_keeps_the_destination_and_cleans_up_the_temporary_file() {
        let dir = TempDir::new();
        let path = dir.path("state.json");
        fs::write(&path, b"original").unwrap();

        let error = write_with(&path, b"replacement", |_temp, _destination, _existing| {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected replacement failure",
            ))
        })
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&path).unwrap(), b"original");
        dir.assert_no_temporary_files();
    }

    #[test]
    fn rejects_a_directory_destination_without_modifying_it() {
        let dir = TempDir::new();
        let path = dir.path("state.json");
        fs::create_dir(&path).unwrap();

        let error = write(&path, b"contents").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(path.is_dir());
        dir.assert_no_temporary_files();
    }
}
