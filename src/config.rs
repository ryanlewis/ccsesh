use std::path::PathBuf;

use serde::Deserialize;

/// Color preference for terminal output.
#[derive(Debug, Clone, PartialEq)]
pub enum ColorSetting {
    /// Force colors on
    Always,
    /// Force colors off
    Never,
    /// Auto-detect (TTY + NO_COLOR check) — default behavior
    Auto,
}

impl Default for ColorSetting {
    fn default() -> Self {
        ColorSetting::Auto
    }
}

/// Custom deserializer for color setting: accepts bool or string "auto".
impl<'de> Deserialize<'de> for ColorSetting {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct ColorVisitor;

        impl<'de> de::Visitor<'de> for ColorVisitor {
            type Value = ColorSetting;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a boolean or the string \"auto\"")
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(if v {
                    ColorSetting::Always
                } else {
                    ColorSetting::Never
                })
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                match v {
                    "auto" => Ok(ColorSetting::Auto),
                    "true" => Ok(ColorSetting::Always),
                    "false" => Ok(ColorSetting::Never),
                    _ => Err(de::Error::invalid_value(
                        de::Unexpected::Str(v),
                        &"true, false, or \"auto\"",
                    )),
                }
            }
        }

        deserializer.deserialize_any(ColorVisitor)
    }
}

/// Configuration loaded from a TOML config file.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    /// Number of sessions to display (default: 5)
    pub limit: Option<usize>,

    /// Default arguments to append on every ccsesh invocation
    pub default_args: Option<Vec<String>>,

    /// Default arguments to pass through to Claude Code when resuming
    pub claude_code_args: Option<Vec<String>>,

    /// Color preference (true, false, or "auto")
    pub colors: Option<ColorSetting>,
}

/// Locate the config file. Checks XDG path first, then legacy fallback.
fn find_config_file() -> Option<PathBuf> {
    // XDG: ~/.config/ccsesh/config.toml
    if let Some(config_dir) = dirs::config_dir() {
        let xdg_path = config_dir.join("ccsesh").join("config.toml");
        if xdg_path.is_file() {
            return Some(xdg_path);
        }
    }

    // Legacy fallback: ~/.ccsesh.toml
    if let Some(home_dir) = dirs::home_dir() {
        let legacy_path = home_dir.join(".ccsesh.toml");
        if legacy_path.is_file() {
            return Some(legacy_path);
        }
    }

    None
}

/// Load configuration from the config file. Returns default config if no file
/// exists or the file is invalid (fail gracefully).
pub fn load_config() -> Config {
    load_config_from_path(find_config_file())
}

/// Load config from a specific path (used by tests and by `load_config`).
fn load_config_from_path(path: Option<PathBuf>) -> Config {
    let Some(path) = path else {
        return Config::default();
    };

    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Config::default(),
    };

    match toml::from_str(&contents) {
        Ok(config) => config,
        Err(_) => Config::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_config(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("config.toml");
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parse_full_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(
            dir.path(),
            r#"
limit = 10
default_args = ["--format", "short"]
claude_code_args = ["--dangerously-skip-permissions"]
colors = "auto"
"#,
        );

        let config = load_config_from_path(Some(path));
        assert_eq!(config.limit, Some(10));
        assert_eq!(
            config.default_args,
            Some(vec!["--format".into(), "short".into()])
        );
        assert_eq!(
            config.claude_code_args,
            Some(vec!["--dangerously-skip-permissions".into()])
        );
        assert_eq!(config.colors, Some(ColorSetting::Auto));
    }

    #[test]
    fn parse_partial_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), "limit = 20\n");

        let config = load_config_from_path(Some(path));
        assert_eq!(config.limit, Some(20));
        assert!(config.default_args.is_none());
        assert!(config.claude_code_args.is_none());
        assert!(config.colors.is_none());
    }

    #[test]
    fn parse_empty_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), "");

        let config = load_config_from_path(Some(path));
        assert!(config.limit.is_none());
    }

    #[test]
    fn invalid_toml_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), "this is not valid toml [[[");

        let config = load_config_from_path(Some(path));
        assert!(config.limit.is_none());
    }

    #[test]
    fn missing_file_returns_default() {
        let config = load_config_from_path(Some(PathBuf::from("/nonexistent/config.toml")));
        assert!(config.limit.is_none());
    }

    #[test]
    fn no_path_returns_default() {
        let config = load_config_from_path(None);
        assert!(config.limit.is_none());
    }

    #[test]
    fn colors_true_bool() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), "colors = true\n");

        let config = load_config_from_path(Some(path));
        assert_eq!(config.colors, Some(ColorSetting::Always));
    }

    #[test]
    fn colors_false_bool() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), "colors = false\n");

        let config = load_config_from_path(Some(path));
        assert_eq!(config.colors, Some(ColorSetting::Never));
    }

    #[test]
    fn colors_auto_string() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), "colors = \"auto\"\n");

        let config = load_config_from_path(Some(path));
        assert_eq!(config.colors, Some(ColorSetting::Auto));
    }

    #[test]
    fn colors_invalid_string_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path(), "colors = \"invalid\"\n");

        // Invalid color value makes the whole config fail gracefully
        let config = load_config_from_path(Some(path));
        assert!(config.colors.is_none());
    }
}
