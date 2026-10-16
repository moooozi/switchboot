//! Signed installer updates, independent of the GUI and firmware services.
//!
//! Calls block and must run on a worker. The host supplies installed-mode policy
//! through [`Installation`]; no network request precedes that policy check.
mod download;
mod handoff;
mod installer;
mod manifest;
mod signature;

pub use handoff::UpgradeRequest;
pub use installer::InstallSession;
use semver::Version;
use std::path::PathBuf;
use thiserror::Error;

const FEED: &str = "https://moooozi.github.io/switchboot/updates/latest.json";
// The public key shipped in the final Tauri build. Keep the release signing secret unchanged.
const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDI1RjRENjU3NjIwMjIzMDYKUldRR0l3SmlWOWIwSmNoMG9qalpsOGI3UDhIZE15eUF3bjk1M1BDNHk3ODQwaExnbStIRzdhbjcK";

/// A host-validated Windows installation. Return `Unavailable` for portable mode.
/// Called again immediately before starting Setup to revalidate the destination.
pub trait Installation {
    fn directory(&self) -> Result<PathBuf>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub notes: String,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Updates are available only in installed Windows mode")]
    Unavailable,
    #[error("Update request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Unable to read or stage the update: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid update manifest: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("Invalid update version: {0}")]
    Version(#[from] semver::Error),
    #[error("Invalid Windows update artifact: {0}")]
    Artifact(&'static str),
    #[error("Update signature verification failed: {0}")]
    Signature(String),
    #[error("Update response exceeds size limit")]
    TooLarge,
    #[error("No newer update available")]
    NoUpdate,
    #[error("The available update changed. Check for updates again.")]
    UpdateChanged,
    #[error("Windows Setup failed: {0}")]
    Setup(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Uses the existing release feed and signing key. The version belongs to the
/// application, never this crate's independently versioned package metadata.
pub struct Updater {
    current: Version,
}

impl Updater {
    pub fn new(current_version: &str) -> Result<Self> {
        Ok(Self {
            current: Version::parse(current_version)?,
        })
    }

    fn fetch(&self, client: &download::Client) -> Result<Option<manifest::Update>> {
        let bytes = client.manifest(FEED)?;
        manifest::parse(&bytes, &self.current)
    }

    pub fn check(&self, installation: &impl Installation) -> Result<Option<Release>> {
        if !cfg!(windows) {
            return Err(Error::Unavailable);
        }
        installation.directory()?;
        Ok(self
            .fetch(&download::Client::new(&self.current)?)?
            .map(|update| update.release))
    }

    /// Recheck the feed, verify the installer and hand it to Inno Setup.
    /// Success means Setup was launched. Its handoff requests GUI exit after preflight.
    pub fn install(
        &self,
        installation: &impl Installation,
        expected_version: &str,
    ) -> Result<InstallSession> {
        self.install_with_progress(installation, expected_version, |_, _| {})
    }

    pub fn install_with_progress(
        &self,
        installation: &impl Installation,
        expected_version: &str,
        progress: impl Fn(u64, Option<u64>),
    ) -> Result<InstallSession> {
        if !cfg!(windows) {
            return Err(Error::Unavailable);
        }
        let destination = installation.directory()?;
        let client = download::Client::new(&self.current)?;
        let update = self.fetch(&client)?.ok_or(Error::NoUpdate)?;
        if update.release.version != expected_version {
            return Err(Error::UpdateChanged);
        }
        let bytes = client.installer(update.url, &progress)?;
        signature::verify(&bytes, &update.signature, PUBLIC_KEY)?;
        if !bytes.starts_with(b"MZ") {
            return Err(Error::Artifact(
                "signed payload is not a Windows executable",
            ));
        }
        let staged = installer::StagedInstaller::new(&bytes)?;
        if installation.directory()? != destination {
            return Err(Error::Unavailable);
        }
        staged.launch()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Portable;
    impl Installation for Portable {
        fn directory(&self) -> Result<PathBuf> {
            Err(Error::Unavailable)
        }
    }

    #[test]
    fn portable_policy_is_checked_before_any_network_request() {
        let updater = Updater::new("9.8.7").unwrap();
        assert_eq!(updater.current.to_string(), "9.8.7");
        assert!(matches!(updater.check(&Portable), Err(Error::Unavailable)));
        #[cfg(windows)]
        assert!(matches!(
            updater.install(&Portable, "9.8.8"),
            Err(Error::Unavailable)
        ));
    }
}
