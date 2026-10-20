//! The editable title rules (`title_rules.toml`). The built-in copy is compiled in;
//! a copy in the app's data folder replaces it, so Zoom/Teams titles can be fixed without a rebuild.

use serde::Deserialize;
use std::path::Path;

const EMBEDDED: &str = include_str!("title_rules.toml");
const FILE: &str = "title_rules.toml";

#[derive(Deserialize, Clone, Debug)]
pub struct Rules {
    pub browser: BrowserRules,
    pub sites: SiteRules,
    pub zoom: ZoomRules,
    pub teams: TeamsRules,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BrowserRules {
    pub suffixes: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct SiteRules {
    pub youtube: Vec<String>,
    pub datacamp: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ZoomRules {
    pub meeting_classes: Vec<String>,
    pub meeting_titles: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TeamsRules {
    pub separator: String,
    pub app_name: String,
    pub nav_words: Vec<String>,
}

impl Rules {
    pub fn embedded() -> Self {
        toml::from_str(EMBEDDED).expect("built-in title_rules.toml is valid")
    }

    /// The copy in `dir` if there is one and it parses, otherwise the built-in rules.
    pub fn load(dir: Option<&Path>) -> Self {
        let custom = dir.and_then(|d| std::fs::read_to_string(d.join(FILE)).ok());
        match custom.map(|text| toml::from_str(&text)) {
            Some(Ok(rules)) => rules,
            Some(Err(e)) => {
                eprintln!("title_rules.toml in the data folder is invalid, using the built-in rules: {e}");
                Self::embedded()
            }
            None => Self::embedded(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_rules_parse() {
        let rules = Rules::embedded();
        assert!(!rules.browser.suffixes.is_empty());
        assert_eq!(rules.teams.app_name, "Microsoft Teams");
    }

    #[test]
    fn a_broken_custom_file_falls_back() {
        let dir = std::env::temp_dir().join(format!("nt-rules-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), "this is [not valid").unwrap();
        assert_eq!(Rules::load(Some(&dir)).teams.app_name, "Microsoft Teams");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
