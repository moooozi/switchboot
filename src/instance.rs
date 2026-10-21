//! GUI ownership and activation use private local files, never a network listener.
//! Keep lock files in place: deleting one can split ownership between two inodes.
use fs4::TryLockError;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const STATE_SIZE: u64 = 12;

pub enum Launch {
    Primary(Primary),
    ActivatedExisting,
}

/// Dropping the guard releases ownership, including when the UI fails to open.
pub struct Primary {
    _lock: File,
    mailbox: Arc<Mutex<Mailbox>>,
}

struct Mailbox {
    file: File,
    last_request: u64,
}

impl Primary {
    pub fn activation_source(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        let mailbox = Arc::clone(&self.mailbox);
        Arc::new(move || {
            let Ok(mut mailbox) = mailbox.lock() else {
                return false;
            };
            let Ok((_, request)) = read_state(&mut mailbox.file) else {
                return false;
            };
            let changed = request != mailbox.last_request;
            mailbox.last_request = request;
            changed
        })
    }
}

pub fn acquire(demo: bool) -> io::Result<Launch> {
    acquire_in(&user_directory()?, if demo { "demo" } else { "gui" })
}

fn acquire_in(directory: &Path, name: &str) -> io::Result<Launch> {
    ensure_private_directory(directory)?;
    let lock = private_file(&directory.join(format!("{name}.lock")))?;
    match fs4::FileExt::try_lock(&lock) {
        Ok(()) => {
            let mut file = private_file(&directory.join(format!("{name}.activation")))?;
            file.set_len(STATE_SIZE)?;
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&std::process::id().to_le_bytes())?;
            file.write_all(&0_u64.to_le_bytes())?;
            Ok(Launch::Primary(Primary {
                _lock: lock,
                mailbox: Arc::new(Mutex::new(Mailbox {
                    file,
                    last_request: 0,
                })),
            }))
        }
        Err(TryLockError::WouldBlock) => {
            let mut file = private_file(&directory.join(format!("{name}.activation")))?;
            // A simultaneous launch may arrive before the owner initializes its mailbox.
            let mut state = None;
            for _ in 0..100 {
                if let Ok(value) = read_state(&mut file) {
                    state = Some(value);
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let (pid, previous) = state.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "The running GUI has not initialized activation",
                )
            })?;
            allow_foreground(pid);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos() as u64;
            let request = (timestamp ^ u64::from(std::process::id())).max(1);
            // Coalesce repeat launches without an unbounded queue or persistent worker.
            let request = if request == previous {
                request.wrapping_add(1)
            } else {
                request
            };
            file.seek(SeekFrom::Start(4))?;
            file.write_all(&request.to_le_bytes())?;
            Ok(Launch::ActivatedExisting)
        }
        Err(TryLockError::Error(error)) => Err(error),
    }
}

fn read_state(file: &mut File) -> io::Result<(u32, u64)> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = [0; STATE_SIZE as usize];
    file.read_exact(&mut bytes)?;
    let pid = u32::from_le_bytes(bytes[..4].try_into().expect("fixed PID length"));
    if pid == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Activation owner is missing",
        ));
    }
    Ok((
        pid,
        u64::from_le_bytes(bytes[4..].try_into().expect("fixed request length")),
    ))
}

fn ensure_private_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    let mut builder = fs::DirBuilder::new();
    #[cfg(not(unix))]
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "GUI state directory is not a private directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "GUI state directory must be owned by this user with mode 0700",
            ));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "GUI state directory cannot be a reparse point",
            ));
        }
    }
    Ok(())
}

fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "GUI state must be a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "GUI state file is not private to this user",
            ));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "GUI state file cannot be a reparse point",
            ));
        }
    }
    Ok(file)
}

#[cfg(target_os = "linux")]
fn user_directory() -> io::Result<PathBuf> {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let runtime = PathBuf::from(runtime);
        if runtime.is_absolute() && ensure_private_directory(&runtime).is_ok() {
            return Ok(runtime.join("switchboot"));
        }
    }
    // /tmp is sticky; the owner/mode checks reject another user's pre-created path.
    Ok(PathBuf::from(format!("/tmp/switchboot-{}", unsafe {
        libc::geteuid()
    })))
}

#[cfg(windows)]
fn user_directory() -> io::Result<PathBuf> {
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath},
    };
    let path = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }
        .map_err(io::Error::other)?;
    let result = unsafe { path.to_string() }.map_err(io::Error::other);
    unsafe { CoTaskMemFree(Some(path.0.cast())) };
    Ok(PathBuf::from(result?).join("Switchboot"))
}

#[cfg(windows)]
fn allow_foreground(pid: u32) {
    // The newly launched foreground process transfers its right to the GUI owner.
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid) };
}

#[cfg(not(windows))]
fn allow_foreground(_pid: u32) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "switchboot-instance-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn primary(directory: &Path, name: &str) -> Primary {
        match acquire_in(directory, name).unwrap() {
            Launch::Primary(guard) => guard,
            Launch::ActivatedExisting => panic!("Expected ownership"),
        }
    }

    #[test]
    fn repeat_launch_activates_owner_without_taking_its_lock() {
        let directory = Directory::new();
        let owner = primary(&directory.0, "gui");
        let poll = owner.activation_source();
        assert!(!poll());
        assert!(matches!(
            acquire_in(&directory.0, "gui").unwrap(),
            Launch::ActivatedExisting
        ));
        assert!(poll());
        assert!(!poll());
        assert!(matches!(
            acquire_in(&directory.0, "gui").unwrap(),
            Launch::ActivatedExisting
        ));
        assert!(poll());
    }

    #[test]
    fn drop_releases_owner_and_resets_stale_activation() {
        let directory = Directory::new();
        let owner = primary(&directory.0, "gui");
        assert!(matches!(
            acquire_in(&directory.0, "gui").unwrap(),
            Launch::ActivatedExisting
        ));
        drop(owner);
        let next = primary(&directory.0, "gui");
        assert!(!next.activation_source()());
    }

    #[test]
    fn demo_and_real_gui_have_independent_ownership() {
        let directory = Directory::new();
        let real = primary(&directory.0, "gui");
        let demo = primary(&directory.0, "demo");
        assert!(matches!(
            acquire_in(&directory.0, "demo").unwrap(),
            Launch::ActivatedExisting
        ));
        assert!(!real.activation_source()());
        assert!(demo.activation_source()());
    }

    #[cfg(unix)]
    #[test]
    fn world_accessible_directory_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let directory = Directory::new();
        fs::create_dir(&directory.0).unwrap();
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            acquire_in(&directory.0, "gui").err().unwrap().kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_or_hardlinked_lock_file_is_rejected() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = Directory::new();
        ensure_private_directory(&directory.0).unwrap();
        let target = directory.0.join("target");
        fs::write(&target, "untouched").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        let lock = directory.0.join("gui.lock");
        symlink(&target, &lock).unwrap();
        assert!(acquire_in(&directory.0, "gui").is_err());
        fs::remove_file(&lock).unwrap();
        fs::hard_link(&target, &lock).unwrap();
        assert!(acquire_in(&directory.0, "gui").is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "untouched");
    }

    #[test]
    fn crashed_process_releases_lock_for_a_new_gui() {
        use std::process::{Command, Stdio};
        let directory = Directory::new();
        ensure_private_directory(&directory.0).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "instance::tests::process_fixture", "--ignored"])
            .env("SWITCHBOOT_INSTANCE_TEST_DIR", &directory.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let ready = directory.0.join("ready");
        for _ in 0..200 {
            if ready.exists() {
                break;
            }
            if let Some(status) = child.try_wait().unwrap() {
                panic!("Ownership fixture exited early: {status}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !ready.exists() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Ownership fixture did not initialize");
        }
        assert!(matches!(
            acquire_in(&directory.0, "gui").unwrap(),
            Launch::ActivatedExisting
        ));
        child.kill().unwrap();
        child.wait().unwrap();
        let next = primary(&directory.0, "gui");
        assert!(!next.activation_source()());
    }

    #[test]
    #[ignore = "subprocess lifecycle fixture"]
    fn process_fixture() {
        let directory = PathBuf::from(
            std::env::var_os("SWITCHBOOT_INSTANCE_TEST_DIR").expect("fixture directory"),
        );
        let _owner = primary(&directory, "gui");
        fs::write(directory.join("ready"), "owned").unwrap();
        // Parent keeps stdin open while testing ownership, then terminates us.
        let mut byte = [0];
        let _ = std::io::stdin().read_exact(&mut byte);
    }
}
