use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, PWSTR};
use windows::Win32::Storage::FileSystem::{CreateFileW, ReadFile, WriteFile, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING};
use windows::Win32::System::Pipes::{PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, SetNamedPipeHandleState};

use crate::ipc_messaging::{ ServerResponse};

pub struct IPCClient {
    handle: Arc<Mutex<HANDLE>>,
}

unsafe impl Send for IPCClient {}
unsafe impl Sync for IPCClient {}


impl IPCClient {
    pub fn connect(pipe_name: &str) -> io::Result<Self> {
        let pipe_name_wide: Vec<u16> = OsStr::new(pipe_name)
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect();

        let handle = unsafe {
            CreateFileW(
                PWSTR(pipe_name_wide.as_ptr() as *mut _),
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                0,
                null_mut(),
                OPEN_EXISTING,
                0,
                HANDLE(0),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        // Set pipe to message mode
        let mut mode = PIPE_READMODE_MESSAGE | PIPE_TYPE_MESSAGE;
        unsafe {
            SetNamedPipeHandleState(handle, &mut mode, null_mut(), null_mut()).ok()?;
        }

        Ok(IPCClient {
            handle: Arc::new(Mutex::new(handle)),
        })
    }

    pub fn send_request(&self, payload: Vec<u8>) -> io::Result<ServerResponse> {
        let data = payload;
        let handle = self.handle.lock().unwrap();
        let mut bytes_written = 0;
        let result = unsafe {
            WriteFile(
                *handle,
                data.as_ptr() as *const _,
                data.len() as u32,
                &mut bytes_written,
                null_mut(),
            )
        };
        if !result.as_bool() {
            return Err(io::Error::last_os_error());
        }

        // Read response
        let mut buf = vec![0u8; 4096];
        let mut bytes_read = 0;
        let result = unsafe {
            ReadFile(
                *handle,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut bytes_read,
                null_mut(),
            )
        };
        if !result.as_bool() {
            return Err(io::Error::last_os_error());
        }
        buf.truncate(bytes_read as usize);
        let resp: ServerResponse = bincode::deserialize(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(resp)
    }
}

impl Drop for IPCClient {
    fn drop(&mut self) {
        // Optionally close handle if needed
    }
}