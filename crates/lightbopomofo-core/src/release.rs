//! Which release is newest, from GitHub's "latest release" JSON
//! (`/repos/nvlznn/light-bopomofo/releases/latest`). Only the apps use this
//! to offer updates; the input method never goes online.

/// The latest release's version without the leading "v" ("2.0"), if it
/// already carries `asset` (the Windows installer is attached a few minutes
/// after the release is published).
pub fn latest_version(json: &str, asset: &str) -> Option<String> {
    let tag = string_values(json, "tag_name").into_iter().next()?;
    let has_asset = string_values(json, "name").iter().any(|name| name == asset);
    has_asset.then(|| tag.trim_start_matches('v').to_string())
}

/// Whether `candidate` is a later version than `current`: numbers compared
/// part by part, missing parts are 0 ("2.0" > "1.0" > "0.2.0", "1.0" = "1.0.0").
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.trim()
            .trim_start_matches('v')
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parts(candidate), parts(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

/// Every string value of `"key": "…"` in the JSON. The values read here
/// (tags, file names) never contain escapes.
fn string_values(json: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let mut values = Vec::new();
    let mut rest = json;
    while let Some(i) = rest.find(&needle) {
        rest = &rest[i + needle.len()..];
        let Some(value) = rest.trim_start().strip_prefix(':') else {
            continue;
        };
        let Some(value) = value.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = value.find('"') {
            values.push(value[..end].to_string());
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from a real response.
    const LATEST: &str = r#"{
  "html_url": "https://github.com/nvlznn/light-bopomofo/releases/tag/v2.0",
  "tag_name": "v2.0",
  "name": "輕注音 2.0",
  "author": { "login": "nvlznn" },
  "assets": [
    {
      "name": "LightBopomofo-Setup.exe",
      "size": 5473541,
      "browser_download_url": "https://github.com/nvlznn/light-bopomofo/releases/download/v2.0/LightBopomofo-Setup.exe"
    },
    {
      "name": "LightBopomofo.dmg",
      "size": 8329913
    }
  ],
  "body": "Shift + \"name\": \"x\""
}"#;

    #[test]
    fn reads_the_version_when_the_asset_is_there() {
        assert_eq!(
            latest_version(LATEST, "LightBopomofo.dmg").as_deref(),
            Some("2.0")
        );
        assert_eq!(
            latest_version(LATEST, "LightBopomofo-Setup.exe").as_deref(),
            Some("2.0")
        );
    }

    #[test]
    fn waits_for_the_asset() {
        let dmg_only = LATEST.replace("LightBopomofo-Setup.exe", "other.zip");
        assert_eq!(latest_version(&dmg_only, "LightBopomofo-Setup.exe"), None);
        assert_eq!(latest_version("{}", "LightBopomofo.dmg"), None);
        assert_eq!(latest_version("not json", "LightBopomofo.dmg"), None);
    }

    #[test]
    fn compact_json() {
        let json = r#"{"tag_name":"v3.0","assets":[{"name":"LightBopomofo.dmg"}]}"#;
        assert_eq!(
            latest_version(json, "LightBopomofo.dmg").as_deref(),
            Some("3.0")
        );
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("2.0", "1.0"));
        assert!(is_newer("1.0", "0.2.0"));
        assert!(is_newer("v10.0", "9.0"));
        assert!(is_newer("1.0.1", "1.0"));
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(!is_newer("1.0", "1.0"));
        assert!(!is_newer("0.1.0", "1.0"));
        assert!(!is_newer("2.0", "10.0"));
    }
}
