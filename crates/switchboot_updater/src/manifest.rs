use crate::{Error, Release, Result};
use semver::Version;
use serde::Deserialize;
use std::collections::HashMap;

pub(crate) struct Update {
    pub release: Release,
    pub url: reqwest::Url,
    pub signature: String,
}

#[derive(Deserialize)]
struct Feed {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    platforms: HashMap<String, Artifact>,
}

#[derive(Deserialize)]
struct Artifact {
    url: String,
    signature: String,
}

pub(crate) fn parse(bytes: &[u8], current: &Version) -> Result<Option<Update>> {
    let mut feed: Feed = serde_json::from_slice(bytes)?;
    let version = Version::parse(feed.version.strip_prefix('v').unwrap_or(&feed.version))?;
    if !version.cmp_precedence(current).is_gt() || !version.pre.is_empty() {
        return Ok(None);
    }
    let artifact = feed
        .platforms
        .remove("windows-x86_64")
        .ok_or(Error::Artifact("missing Windows installer"))?;
    let url = reqwest::Url::parse(&artifact.url).map_err(|_| Error::Artifact("invalid URL"))?;
    if url.scheme() != "https" || artifact.signature.is_empty() {
        return Err(Error::Artifact("HTTPS and a signature are required"));
    }
    Ok(Some(Update {
        release: Release {
            version: version.to_string(),
            notes: feed.notes.unwrap_or_default(),
        },
        url,
        signature: artifact.signature,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn feed(version: &str, url: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"version": version, "platforms": {
            "windows-x86_64": {"url": url, "signature": "fixture"}
        }}))
        .unwrap()
    }

    #[test]
    fn versions_and_transport_fail_closed() {
        assert!(
            parse(
                &feed("0.4.0", "https://example.com/setup.exe"),
                &Version::parse("0.3.0").unwrap()
            )
            .unwrap()
            .is_some()
        );
        for version in ["0.2.0", "0.3.0", "0.3.0+build.2", "0.4.0-beta.1"] {
            assert!(
                parse(
                    &feed(version, "https://example.com/setup.exe"),
                    &Version::parse("0.3.0").unwrap()
                )
                .unwrap()
                .is_none()
            );
        }
        assert!(
            parse(
                &feed("0.4.0", "http://example.com/setup.exe"),
                &Version::parse("0.3.0").unwrap()
            )
            .is_err()
        );
        assert!(
            parse(
                &feed("bad", "https://example.com/setup.exe"),
                &Version::parse("0.3.0").unwrap()
            )
            .is_err()
        );
        assert!(
            parse(
                br#"{"version":"0.4.0","platforms":{}}"#,
                &Version::parse("0.3.0").unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn legacy_versions_are_normalized_and_null_notes_are_accepted() {
        let mut data: serde_json::Value =
            serde_json::from_slice(&feed("v0.4.0", "https://example.com/setup.exe")).unwrap();
        data["notes"] = serde_json::Value::Null;
        let update = parse(
            &serde_json::to_vec(&data).unwrap(),
            &Version::parse("0.3.0").unwrap(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(update.release.version, "0.4.0");
        assert!(update.release.notes.is_empty());
    }
}
