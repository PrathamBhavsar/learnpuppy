//! Global `settings.json`: per-role profiles (provider, model, effort), slot pins, learning style.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Roles that take a profile: the loop agents plus the workbench sub-agents.
pub const PROFILE_ROLES: [&str; 9] = [
    "prober",
    "teacher",
    "practice",
    "extractor",
    "auditor",
    "research",
    "visual",
    "demo",
    "pdf",
];
pub const PROVIDERS: [&str; 3] = ["codex", "agy", "claude"];
pub const EFFORTS: [&str; 6] = ["low", "medium", "high", "xhigh", "max", "ultra"];

/// Model slots, in the order the accounts screen lists them.
pub const ROLES: [&str; 6] = [
    "prober",
    "teacher",
    "practice",
    "extractor",
    "auditor",
    "distiller",
];
/// The pin of a slot nobody pinned.
pub const DEFAULT_PIN: &str = "claude:auto";

/// Which CLI, model and reasoning effort a role runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub provider: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Let the account pool fall back to other providers (with this model dropped).
    #[serde(default)]
    pub fallback: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LearningStyle {
    #[default]
    Default,
    Visual,
}

fn one_of(what: &str, v: &str, ok: &[&str]) -> anyhow::Result<()> {
    anyhow::ensure!(
        ok.contains(&v),
        "unknown {what} `{v}` (one of: {})",
        ok.join(", ")
    );
    Ok(())
}

impl Profile {
    pub fn new(provider: &str, model: &str, effort: Option<&str>) -> anyhow::Result<Self> {
        one_of("provider", provider, &PROVIDERS)?;
        anyhow::ensure!(!model.trim().is_empty(), "model is empty");
        if let Some(e) = effort {
            one_of("effort", e, &EFFORTS)?;
        }
        Ok(Self {
            provider: provider.into(),
            model: model.into(),
            effort: effort.map(Into::into),
            fallback: false,
        })
    }
}

impl LearningStyle {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "visual" => Ok(Self::Visual),
            "default" => Ok(Self::Default),
            _ => anyhow::bail!("unknown style `{s}` (one of: visual, default)"),
        }
    }
}

/// Model ids per provider, for the catalog: the starter table, every saved profile, plus the
/// Claude aliases the CLI accepts. Sorted, no duplicates.
pub fn catalog_models(provider: &str, saved: &Settings) -> Vec<String> {
    let mut out: Vec<String> = DEFAULT_PROFILES
        .iter()
        .filter(|(_, p, _, _)| *p == provider)
        .map(|(_, _, m, _)| (*m).to_owned())
        .collect();
    if provider == "claude" {
        out.extend(
            [
                "auto",
                "opus",
                "sonnet",
                "haiku",
                "claude-fable-5-1",
                "claude-opus-5-5",
                "claude-sonnet-5-5",
                "claude-haiku-5-5",
            ]
            .map(str::to_owned),
        );
    }
    out.extend(
        saved
            .profiles
            .values()
            .filter(|p| p.provider == provider)
            .map(|p| p.model.clone()),
    );
    out.sort();
    out.dedup();
    out
}

/// The coordinator's starter table: role, provider, model, effort.
const DEFAULT_PROFILES: [(&str, &str, &str, Option<&str>); 9] = [
    ("prober", "codex", "gpt-6.1-sol", Some("low")),
    ("teacher", "agy", "gemini-4-argon-high", None),
    ("practice", "codex", "gpt-6-astra", Some("medium")),
    ("extractor", "codex", "gpt-6.1-terra", Some("high")),
    ("auditor", "agy", "claude-opus-5-5-high", None),
    ("research", "codex", "gpt-6-sol", Some("high")),
    ("visual", "agy", "gemini-4-argon-high", None),
    ("demo", "codex", "gpt-6.1-sol", Some("medium")),
    ("pdf", "agy", "gemini-3.8-flash-medium", None),
];

pub const THEMES: [&str; 3] = ["dark", "light", "system"];
pub const DIRECTIONS: [&str; 3] = ["ltr", "rtl", "auto"];
pub const DENSITIES: [&str; 2] = ["comfortable", "compact"];
/// Parallel model calls the server allows (docs/web/06 Settings → Behavior).
pub const CONCURRENCY_RANGE: std::ops::RangeInclusive<u8> = 1..=6;
pub const DEFAULT_CONCURRENCY: u8 = 2;

/// Appearance and behavior preferences the web UI reads at start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// One of [`THEMES`].
    pub theme: String,
    /// One of [`DIRECTIONS`].
    pub direction: String,
    /// One of [`DENSITIES`].
    pub density: String,
    pub reduce_motion: bool,
    pub confirm_destructive: bool,
    pub auto_open_browser: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            direction: "ltr".into(),
            density: "comfortable".into(),
            reduce_motion: false,
            confirm_destructive: true,
            auto_open_browser: true,
        }
    }
}

impl UiSettings {
    pub fn validate(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        for (path, v, ok) in [
            ("ui.theme", &self.theme, &THEMES[..]),
            ("ui.direction", &self.direction, &DIRECTIONS[..]),
            ("ui.density", &self.density, &DENSITIES[..]),
        ] {
            if !ok.contains(&v.as_str()) {
                out.push((path, format!("must be one of: {}", ok.join(", "))));
            }
        }
        out
    }
}

/// Model slot pins, extra Claude profiles and allowlisted repos.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Role -> `claude:auto` | account id.
    pub pins: BTreeMap<String, String>,
    /// Role -> provider, model and effort; a role without one keeps its pin and the CLI defaults.
    pub profiles: BTreeMap<String, Profile>,
    pub learning_style: LearningStyle,
    pub claude_dirs: Vec<PathBuf>,
    pub repos: Vec<PathBuf>,
    /// This install's id in the FSRS replication logs; minted on first use.
    pub device_id: String,
    /// Appearance and behavior preferences.
    pub ui: UiSettings,
    /// Parallel model calls; 0 (unset) means [`DEFAULT_CONCURRENCY`].
    pub concurrency_cap: u8,
    /// Accounts the user excluded from the pool; re-applied at start.
    pub disabled_accounts: Vec<String>,
}

impl Settings {
    fn path(root: &Path) -> PathBuf {
        crate::paths::settings_json(root)
    }

    /// The pin serving `role`: its saved pin, else `DEFAULT_PIN`.
    pub fn pin(&self, role: &str) -> &str {
        self.pins.get(role).map_or(DEFAULT_PIN, String::as_str)
    }

    /// The effective concurrency cap, `1..=6`.
    pub fn concurrency(&self) -> u8 {
        match self.concurrency_cap {
            0 => DEFAULT_CONCURRENCY,
            n => n.clamp(*CONCURRENCY_RANGE.start(), *CONCURRENCY_RANGE.end()),
        }
    }

    pub fn load(root: &Path) -> anyhow::Result<Self> {
        match std::fs::read(Self::path(root)) {
            Ok(b) => serde_json::from_slice(&b).context("settings.json"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_profile(&mut self, role: &str, p: Profile) -> anyhow::Result<()> {
        one_of("role", role, &PROFILE_ROLES)?;
        self.profiles.insert(role.into(), p);
        Ok(())
    }

    /// Writes the starter profile table and the visual learning style.
    pub fn apply_defaults(&mut self) {
        for (role, provider, model, effort) in DEFAULT_PROFILES {
            let p = Profile {
                provider: provider.into(),
                model: model.into(),
                effort: effort.map(Into::into),
                fallback: false,
            };
            self.profiles.insert(role.into(), p);
        }
        self.learning_style = LearningStyle::Visual;
    }

    /// The replication device id, minting and saving one the first time.
    pub fn ensure_device(&mut self, root: &Path) -> anyhow::Result<&str> {
        if self.device_id.is_empty() {
            self.device_id = format!("dev{:x}{:x}", crate::now_ms(), std::process::id());
            self.save(root)?;
        }
        Ok(&self.device_id)
    }

    /// Atomic write: temp file, fsync, rename.
    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(root)?;
        let path = Self::path(root);
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(self)?)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_default_when_missing() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(d.path()).unwrap(), Settings::default());
        let mut s = Settings::default();
        s.pins.insert("teacher".into(), "agy:default".into());
        s.save(d.path()).unwrap();
        assert_eq!(Settings::load(d.path()).unwrap(), s);
    }

    #[test]
    fn old_settings_without_profiles_still_load() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"pins":{"teacher":"agy:auto"}}"#,
        )
        .unwrap();
        let s = Settings::load(d.path()).unwrap();
        assert!(s.profiles.is_empty());
        assert_eq!(s.learning_style, LearningStyle::Default);
        assert_eq!(s.pins["teacher"], "agy:auto");
    }

    #[test]
    fn profile_validation_and_starter_table() {
        assert!(Profile::new("gpt", "m", None).is_err());
        assert!(Profile::new("codex", "m", Some("turbo")).is_err());
        assert!(Profile::new("codex", " ", None).is_err());
        let mut s = Settings::default();
        assert!(s
            .set_profile("nobody", Profile::new("codex", "m", None).unwrap())
            .is_err());
        s.apply_defaults();
        assert_eq!(s.profiles.len(), PROFILE_ROLES.len());
        let t = &s.profiles["teacher"];
        assert_eq!(
            (t.provider.as_str(), t.model.as_str(), t.effort.clone()),
            ("agy", "gemini-4-argon-high", None)
        );
        assert_eq!(s.profiles["prober"].effort.as_deref(), Some("low"));
        assert_eq!(s.learning_style, LearningStyle::Visual);
        // The starter table passes the same validation as user input.
        for p in s.profiles.values() {
            assert_eq!(
                &Profile::new(&p.provider, &p.model, p.effort.as_deref()).unwrap(),
                p
            );
        }
        let d = tempfile::tempdir().unwrap();
        s.save(d.path()).unwrap();
        assert_eq!(Settings::load(d.path()).unwrap(), s);
    }

    #[test]
    fn unpinned_roles_use_the_default_pin() {
        let mut s = Settings::default();
        assert_eq!(s.pin("teacher"), DEFAULT_PIN);
        s.pins.insert("teacher".into(), "agy:auto".into());
        assert_eq!(s.pin("teacher"), "agy:auto");
    }

    #[test]
    fn device_id_is_minted_once_and_persisted() {
        let d = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        let id = s.ensure_device(d.path()).unwrap().to_owned();
        assert!(id.starts_with("dev"));
        assert_eq!(Settings::load(d.path()).unwrap().device_id, id);
        assert_eq!(s.ensure_device(d.path()).unwrap(), id);
    }
}
