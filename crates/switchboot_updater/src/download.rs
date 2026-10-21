use std::{io::Read, time::Duration};

use crate::{Error, Result};

const MANIFEST_LIMIT: u64 = 1024 * 1024;
const INSTALLER_LIMIT: u64 = 256 * 1024 * 1024;

pub(crate) struct Client(reqwest::blocking::Client);

impl Client {
    pub fn new(version: &semver::Version) -> Result<Self> {
        Ok(Self(
            reqwest::blocking::Client::builder()
                .https_only(true)
                .connect_timeout(Duration::from_secs(15))
                .user_agent(format!("Switchboot/{version}"))
                .build()?,
        ))
    }

    pub fn manifest(&self, url: &str) -> Result<Vec<u8>> {
        self.download(url, MANIFEST_LIMIT, Duration::from_secs(20), &|_, _| {})
    }

    pub fn installer(
        &self,
        url: reqwest::Url,
        progress: &impl Fn(u64, Option<u64>),
    ) -> Result<Vec<u8>> {
        self.download(
            url.as_str(),
            INSTALLER_LIMIT,
            Duration::from_secs(300),
            progress,
        )
    }

    fn download(
        &self,
        url: &str,
        limit: u64,
        timeout: Duration,
        progress: &impl Fn(u64, Option<u64>),
    ) -> Result<Vec<u8>> {
        let response = self
            .0
            .get(url)
            .timeout(timeout)
            .send()?
            .error_for_status()?;
        let length = response.content_length();
        read_bounded_progress(response, length, limit, progress)
    }
}

#[cfg(test)]
fn read_bounded(reader: impl Read, length: Option<u64>, limit: u64) -> Result<Vec<u8>> {
    read_bounded_progress(reader, length, limit, &|_, _| {})
}

fn read_bounded_progress(
    mut reader: impl Read,
    length: Option<u64>,
    limit: u64,
    progress: &impl Fn(u64, Option<u64>),
) -> Result<Vec<u8>> {
    if length.is_some_and(|size| size > limit) {
        return Err(Error::TooLarge);
    }
    let mut bytes = Vec::new();
    let mut chunk = [0; 64 * 1024];
    progress(0, length);
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > limit {
            return Err(Error::TooLarge);
        }
        bytes.extend_from_slice(&chunk[..count]);
        progress(bytes.len() as u64, length);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_streamed_bytes_with_and_without_content_length() {
        for length in [None, Some(150_000)] {
            let observed = std::cell::RefCell::new(Vec::new());
            let bytes = vec![42; 150_000];
            assert_eq!(
                read_bounded_progress(&bytes[..], length, 200_000, &|downloaded, total| {
                    observed.borrow_mut().push((downloaded, total));
                })
                .unwrap(),
                bytes
            );
            let observed = observed.into_inner();
            assert_eq!(observed.first(), Some(&(0, length)));
            assert_eq!(observed.last(), Some(&(150_000, length)));
            assert!(observed.windows(2).all(|pair| pair[0].0 < pair[1].0));
        }
    }

    #[test]
    fn size_limit_applies_to_declared_and_streamed_bodies() {
        assert!(matches!(
            read_bounded(&b""[..], Some(5), 4),
            Err(Error::TooLarge)
        ));
        for length in [None, Some(1)] {
            assert!(matches!(
                read_bounded(&b"12345"[..], length, 4),
                Err(Error::TooLarge)
            ));
        }
        assert_eq!(read_bounded(&b"1234"[..], None, 4).unwrap(), b"1234");
    }
}
