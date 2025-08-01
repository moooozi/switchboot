use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, PWSTR};
// use windows::core::PCWSTR; // Not available in this version, use *const u16 instead
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Pipes::{SetNamedPipeHandleState, PIPE_READMODE_MESSAGE};

pub struct IPCClient {
    handle: Arc<Mutex<HANDLE>>,
}

unsafe impl Send for IPCClient {}
unsafe impl Sync for IPCClient {}

impl IPCClient {
    /// For demo/testing: get a clone of the handle Arc<Mutex<HANDLE>>
    pub fn get_handle(&self) -> Arc<Mutex<HANDLE>> {
        self.handle.clone()
    }

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
        let mut mode = PIPE_READMODE_MESSAGE;
        unsafe {
            SetNamedPipeHandleState(handle, &mut mode, null_mut(), null_mut()).ok()?;
        }

        Ok(IPCClient {
            handle: Arc::new(Mutex::new(handle)),
        })
    }

    pub fn send_request(&self, payload: Vec<u8>) -> io::Result<Vec<u8>> {
        let data = payload;
        let handle = self.handle.lock().unwrap();

        // Prefix message with length
        let len = (data.len() as u32).to_le_bytes();
        let mut bytes_written = 0;
        unsafe {
            WriteFile(
                *handle,
                len.as_ptr() as *const _,
                len.len() as u32,
                &mut bytes_written,
                null_mut(),
            );
            WriteFile(
                *handle,
                data.as_ptr() as *const _,
                data.len() as u32,
                &mut bytes_written,
                null_mut(),
            );
        }

        // Read response length
        let mut len_buf = [0u8; 4];
        let mut bytes_read = 0;
        unsafe {
            ReadFile(
                *handle,
                len_buf.as_mut_ptr() as *mut _,
                4,
                &mut bytes_read,
                null_mut(),
            );
        }
        let resp_len = u32::from_le_bytes(len_buf) as usize;
        let mut buf = vec![0u8; resp_len];
        let mut bytes_read = 0;
        unsafe {
            ReadFile(
                *handle,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut bytes_read,
                null_mut(),
            );
        }
        buf.truncate(bytes_read as usize);
        Ok(buf)
    }
}

impl Drop for IPCClient {
    fn drop(&mut self) {
        // Optionally close handle if needed
    }
}
