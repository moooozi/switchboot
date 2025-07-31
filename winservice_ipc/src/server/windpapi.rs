use log::error;
use std::ptr;
use std::slice;
use windows::Win32::Foundation::PWSTR;
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTOAPI_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
};
use windows::Win32::System::Memory::LocalFree;

fn get_data(blob_out: CRYPTOAPI_BLOB) -> Vec<u8> {
    let cb_data = blob_out.cbData as usize;
    let pb_data = blob_out.pbData;
    let buffer = unsafe { slice::from_raw_parts(pb_data, cb_data) };
    let data = buffer.to_vec();
    unsafe { LocalFree(pb_data as isize) };
    data
}

pub fn win32_crypt_protect_data(plain_text: &[u8]) -> Result<Vec<u8>, String> {
    let mut blob_in = CRYPTOAPI_BLOB {
        cbData: plain_text.len() as u32,
        pbData: plain_text.as_ptr() as *mut u8,
    };
    let mut blob_out = CRYPTOAPI_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let result = unsafe {
        CryptProtectData(
            &mut blob_in,
            PWSTR::default(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut blob_out,
        )
    };
    if result.as_bool() {
        Ok(get_data(blob_out))
    } else {
        error!("Failed to encrypt data");
        Err("Failed to encrypt data".into())
    }
}

pub fn win32_crypt_unprotect_data(cipher_text: &[u8]) -> Result<Vec<u8>, String> {
    let mut blob_in = CRYPTOAPI_BLOB {
        cbData: cipher_text.len() as u32,
        pbData: cipher_text.as_ptr() as *mut u8,
    };
    let mut blob_out = CRYPTOAPI_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let result = unsafe {
        CryptUnprotectData(
            &mut blob_in,
            &mut PWSTR::default(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut blob_out,
        )
    };
    if result.as_bool() {
        Ok(get_data(blob_out))
    } else {
        error!("Failed to decrypt data");
        Err("Failed to decrypt data".into())
    }
}

// test the windpapi module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let data = b"Hello, world!";
        let encrypted_data = win32_crypt_protect_data(data).unwrap();
        let decrypted_data = win32_crypt_unprotect_data(&encrypted_data).unwrap();
        // print the decrypted data decoded as utf-8
        println!(
            "Decrypted data: {}",
            String::from_utf8(decrypted_data.clone()).unwrap()
        );
        assert_eq!(data, decrypted_data.as_slice());
    }

    // read the bufer of "C:\Users\zidm\AppData\Roaming\BetterWG\.wgprofiles\DE-Nuermberg.conf.dpapi" and decrypt it
    #[test]
    fn test_decrypt_file() {
        use std::fs::File;
        use std::io::Read;
        let mut file = File::open(
            "C:\\Users\\zidm\\AppData\\Roaming\\BetterWG\\.wgprofiles\\DE-Nuermberg2.conf.dpapi",
        )
        .unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let decrypted_data = win32_crypt_unprotect_data(&buffer).unwrap();
        // print the decrypted data decoded as utf-8
        println!(
            "Decrypted data: {}",
            String::from_utf8(decrypted_data.clone()).unwrap()
        );
    }

    #[test]
    fn test_encrypt_and_decrypt_file() {
        // open the file "C:\\Users\\zidm\\Documents\\DE-Nuermberg.conf" and encrypt it
        use std::fs::File;
        use std::io::Read;
        let mut file = File::open("C:\\Users\\zidm\\Documents\\DE-Nuermberg.conf").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let encrypted_data = win32_crypt_protect_data(&buffer).unwrap();
        // write the encrypted data to a new file "C:\\Users\\zidm\\Documents\\DE-Nuermberg.conf.dpapi"
        use std::io::Write;
        let mut file = File::create("C:\\Users\\zidm\\Documents\\DE-Nuermberg.conf.dpapi").unwrap();
        file.write_all(&encrypted_data).unwrap();
        // read the encrypted file and decrypt it
        let mut file = File::open("C:\\Users\\zidm\\Documents\\DE-Nuermberg.conf.dpapi").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        let decrypted_data = win32_crypt_unprotect_data(&buffer).unwrap();
        // print the decrypted data decoded as utf-8
        println!(
            "Decrypted data: {}",
            String::from_utf8(decrypted_data.clone()).unwrap()
        );
    }
}
