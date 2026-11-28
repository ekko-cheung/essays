use semver::Version;
use serde::Deserialize;
use std::time::Duration;

pub const RELEASES_URL: &str = "https://github.com/ekko-cheung/essays/releases";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

pub async fn check() -> Result<Option<Version>, String> {
    let release = reqwest::Client::new()
        .get("https://api.github.com/repos/ekko-cheung/essays/releases/latest")
        .header(
            reqwest::header::USER_AGENT,
            concat!("Essays/", env!("CARGO_PKG_VERSION")),
        )
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?
        .json::<Release>()
        .await
        .map_err(|error| error.to_string())?;
    newer_version(release)
}

fn newer_version(release: Release) -> Result<Option<Version>, String> {
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let tag = release
        .tag_name
        .strip_prefix('v')
        .ok_or("Invalid release tag.")?;
    let version = Version::parse(tag).map_err(|error| error.to_string())?;
    let installed = Version::parse(env!("CARGO_PKG_VERSION")).expect("valid Cargo version");
    Ok((version.pre.is_empty() && version.cmp_precedence(&installed).is_gt()).then_some(version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_newer_stable_releases() {
        let release = |tag: &str, draft, prerelease| {
            newer_version(Release {
                tag_name: tag.into(),
                draft,
                prerelease,
            })
        };
        assert!(release("v0.4.3", false, false).unwrap().is_none());
        assert!(
            release(concat!("v", env!("CARGO_PKG_VERSION")), false, false)
                .unwrap()
                .is_none()
        );
        assert!(release("v999.0.0", false, false).unwrap().is_some());
        assert!(release("v999.0.0", true, false).unwrap().is_none());
        assert!(release("v999.0.0-beta.1", false, true).unwrap().is_none());
        assert!(release("invalid", false, false).is_err());
    }
}
