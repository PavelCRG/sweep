use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Debug)]
pub struct Release {
    pub version: String,
    tag: String,
    draft: bool,
    prerelease: bool,
}

pub enum UpdateStatus {
    Current,
    NoRelease,
    Available { version: String, url: String },
}

fn parse_release(json: &str) -> Result<Release, serde_json::Error> {
    let release: ApiRelease = serde_json::from_str(json)?;
    Ok(Release {
        version: release.tag_name.trim_start_matches('v').to_owned(),
        tag: release.tag_name,
        draft: release.draft,
        prerelease: release.prerelease,
    })
}

fn update_available(current: &str, release: &Release) -> Result<bool, semver::Error> {
    Ok(!release.draft
        && !release.prerelease
        && semver::Version::parse(current)? < semver::Version::parse(&release.version)?)
}

pub fn check(current: &str) -> Result<UpdateStatus, Box<dyn std::error::Error>> {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .https_only(true)
            .build(),
    );
    let mut response = match agent
        .get("https://api.github.com/repos/PavelCRG/sweep/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(UpdateStatus::NoRelease),
        Err(error) => return Err(error.into()),
    };
    let body = response.body_mut().read_to_string()?;
    let release = parse_release(&body)?;
    if update_available(current, &release)? {
        Ok(UpdateStatus::Available {
            url: format!(
                "https://github.com/PavelCRG/sweep/releases/tag/{}",
                release.tag
            ),
            version: release.version,
        })
    } else {
        Ok(UpdateStatus::Current)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_release, update_available};

    #[test]
    fn accepts_release_tags_with_or_without_v_prefix() {
        let release = parse_release(
            r#"{"tag_name":"v0.1.2","html_url":"https://github.com/PavelCRG/sweep/releases/tag/v0.1.2","draft":false,"prerelease":false}"#,
        )
        .unwrap();
        assert_eq!(release.version, "0.1.2");
        assert!(update_available("0.1.0", &release).unwrap());
        assert!(!update_available("0.1.2", &release).unwrap());
    }

    #[test]
    fn ignores_drafts_and_prereleases() {
        let release = parse_release(
            r#"{"tag_name":"v1.0.0","html_url":"https://github.com/PavelCRG/sweep/releases/tag/v1.0.0","draft":false,"prerelease":true}"#,
        )
        .unwrap();
        assert!(release.prerelease);
        assert!(!update_available("0.1.0", &release).unwrap());
    }
}
