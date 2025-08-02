use std::ffi::OsStr;
use std::io::{self};
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{BOOL, HANDLE, INVALID_HANDLE_VALUE, PWSTR};
use windows::Win32::Security::{
    InitializeSecurityDescriptor, SetSecurityDescriptorDacl, SECURITY_ATTRIBUTES,
    SECURITY_DESCRIPTOR,
};
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile, PIPE_ACCESS_DUPLEX};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, SetNamedPipeHandleState, PIPE_NOWAIT,
    PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
use windows::Win32::System::SystemServices::SECURITY_DESCRIPTOR_REVISION;
/// IPC struct representing a named pipe server.
pub struct IPCServer {
    handle: Arc<Mutex<HANDLE>>,
    is_client_connected: Arc<Mutex<bool>>,
}

unsafe impl Send for IPCServer {}
unsafe impl Sync for IPCServer {}

impl IPCServer {
    /// Creates a new IPC server with the specified pipe name.
    pub fn new(pipe_name: &str) -> Self {
        let pipe_name_wide: Vec<u16> = OsStr::new(pipe_name)
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect();

        // Initialize security attributes to allow all users to join
        let mut security_attributes: SECURITY_ATTRIBUTES = unsafe { std::mem::zeroed() };
        let mut security_descriptor: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };

        unsafe {
            InitializeSecurityDescriptor(
                &mut security_descriptor as *mut _ as *mut _,
                SECURITY_DESCRIPTOR_REVISION,
            )
            .unwrap();
            SetSecurityDescriptorDacl(
                &mut security_descriptor as *mut _ as *mut _,
                BOOL(1),
                std::ptr::null_mut(),
                BOOL(0),
            )
            .unwrap();
        }

        security_attributes.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
        security_attributes.lpSecurityDescriptor = &mut security_descriptor as *mut _ as *mut _;
        security_attributes.bInheritHandle = true.into();

        let handle: HANDLE = unsafe {
            CreateNamedPipeW(
                PWSTR(pipe_name_wide.as_ptr() as *mut _),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                1,
                1024 * 16,
                1024 * 16,
                0,
                &mut security_attributes,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            panic!(
                "Failed to create named pipe: {}",
                io::Error::last_os_error()
            );
        }

        IPCServer {
            handle: Arc::new(Mutex::new(handle)),
            is_client_connected: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_non_blocking(&self) {
        let handle = self.handle.lock().unwrap();
        let mut mode = PIPE_NOWAIT;
        unsafe {
            SetNamedPipeHandleState(*handle, &mut mode, null_mut(), null_mut()).unwrap();
        }
    }

    /// Waits for a client to connect to the named pipe.
    pub fn wait_for_client(&self) -> bool {
        let handle = self.handle.lock().unwrap();
        let connected = unsafe { ConnectNamedPipe(*handle, null_mut()).as_bool() };
        if !connected {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(windows::Win32::Foundation::ERROR_PIPE_CONNECTED as i32) {
                *self.is_client_connected.lock().unwrap() = true;
                return true;
            } else if err.raw_os_error()
                == Some(windows::Win32::Foundation::ERROR_PIPE_LISTENING as i32)
            {
                // Pipe is still waiting for a client to connect
                return false;
            } else if err.raw_os_error() == Some(windows::Win32::Foundation::ERROR_NO_DATA as i32) {
                println!("No data available, pipe is being closed. Waiting for a new client...");
                *self.is_client_connected.lock().unwrap() = false;
                unsafe {
                    DisconnectNamedPipe(*handle).unwrap();
                }
                return false;
            } else {
                *self.is_client_connected.lock().unwrap() = false;
                panic!("Failed to connect named pipe: {}", err);
            }
        }
        println!("Client connected!");
        *self.is_client_connected.lock().unwrap() = true;
        true
    }

    /// Sends a message through the named pipe.
    pub fn send_message(&self, message: &[u8]) -> bool {
        let handle = self.handle.lock().unwrap();
        unsafe {
            let len = (message.len() as u32).to_le_bytes();
            let mut bytes_written = 0;
            let result = WriteFile(
                *handle,
                len.as_ptr() as *const _,
                len.len() as u32,
                &mut bytes_written,
                null_mut(),
            )
            .as_bool();
            if !result {
                return false;
            }
            let result = WriteFile(
                *handle,
                message.as_ptr() as *const _,
                message.len() as u32,
                &mut bytes_written,
                null_mut(),
            )
            .as_bool();
            if !result {
                return false;
            }
            true
        }
    }

    /// Receives a message from the named pipe.
    pub fn receive_message(&self, buffer: &mut Vec<u8>) -> bool {
        let handle = self.handle.lock().unwrap();
        unsafe {
            let mut len_buf = [0u8; 4];
            let mut bytes_read = 0;
            let result = ReadFile(
                *handle,
                len_buf.as_mut_ptr() as *mut _,
                4,
                &mut bytes_read,
                null_mut(),
            )
            .as_bool();
            if !result || bytes_read != 4 {
                return false;
            }
            let msg_len = u32::from_le_bytes(len_buf) as usize;
            buffer.resize(msg_len, 0);
            let mut bytes_read = 0;
            let result = ReadFile(
                *handle,
                buffer.as_mut_ptr() as *mut _,
                msg_len as u32,
                &mut bytes_read,
                null_mut(),
            )
            .as_bool();
            if !result || bytes_read != msg_len as u32 {
                return false;
            }
            true
        }
    }

    /// Returns the client connection status.
    pub fn is_client_connected(&self) -> bool {
        *self.is_client_connected.lock().unwrap()
    }
}

impl Drop for IPCServer {
    fn drop(&mut self) {
        let handle = self.handle.lock().unwrap();
        unsafe {
            DisconnectNamedPipe(*handle).unwrap();
        }
    }
}

pub fn pipe_server<H>(
    should_stop: Arc<AtomicBool>,
    ipc: Arc<IPCServer>,
    handle_client_request: H,
    timeout: Option<Duration>,
) where
    H: Fn(&IPCServer, &[u8]),
{
    let mut last_client_connect_attempt = Instant::now();
    println!("Pipe server started.");

    loop {
        if should_stop.load(Ordering::SeqCst) {
            println!("Stopping server as should_stop is set to true.");
            break;
        }

        // Only check timeout if set
        if let Some(timeout_duration) = timeout {
            if last_client_connect_attempt.elapsed() >= timeout_duration {
                println!(
                    "No client connected for {:?}. Stopping server.",
                    timeout_duration
                );
                should_stop.store(true, Ordering::SeqCst);
                break;
            }
        }

        // Wait for a client is now non-blocking
        if !ipc.wait_for_client() {
            continue;
        }

        // Reset the timer as a client has connected
        last_client_connect_attempt = Instant::now();

        let mut buffer = Vec::new();
        if ipc.receive_message(&mut buffer) {
            handle_client_request(&ipc, &buffer);
        }
        sleep(Duration::from_millis(20));
    }
}
