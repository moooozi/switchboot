// Platform-agnostic utils API and helpers
#[cfg(windows)]
pub mod windows;
#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub use windows::*;
#[cfg(unix)]
pub use unix::*;

// Platform-agnostic helpers (utf16_string_from_bytes, string_to_utf16_bytes, iter_unpack, etc.)
use std::mem;

pub fn utf16_string_from_bytes(raw: &[u8]) -> Result<String, &'static str> {
    if raw.len() % 2 != 0 {
        return Err("Input length is not even");
    }
    let mut end = 0;
    while end + 1 < raw.len() {
        if raw[end] == 0 && raw[end + 1] == 0 {
            break;
        }
        end += 2;
    }
    String::from_utf16(
        &raw[..end]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| "Invalid UTF-16")
}

pub fn string_to_utf16_bytes(s: &str) -> Vec<u8> {
    let mut v: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    v.extend_from_slice(&[0, 0]);
    v
}

pub fn iter_unpack<'a, T: Copy>(buffer: &'a [u8]) -> impl Iterator<Item = T> + 'a {
    buffer.chunks_exact(mem::size_of::<T>()).map(|chunk| {
        let size = mem::size_of::<T>();
        assert_eq!(chunk.len(), size);
        let mut t = std::mem::MaybeUninit::<T>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(chunk.as_ptr(), t.as_mut_ptr() as *mut u8, size);
            t.assume_init()
        }
    })
}
