//! `workspace.json`: the metadata of one workspace (docs/web/02 §Metadata) and its validation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{fsx, Result, WorkspaceError};
use crate::settings::Profile;

/// The `workspace.json` schema this build reads and writes.
pub const SCHEMA: u32 = 1;

/// The metadata file inside every workspace root.
pub const META_FILE: &str = "workspace.json";

/// The fixed icon allow-list (lucide-react names, docs/web/04-preset-facts).
pub const ICONS: [&str; 24] = [
    "book-open",
    "brain",
    "code",
    "cpu",
    "database",
    "flask-conical",
    "globe",
    "graduation-cap",
    "lightbulb",
    "layers",
    "network",
    "rocket",
    "server",
    "shield",
    "sparkles",
    "terminal",
    "wrench",
    "puzzle",
    "notebook-pen",
    "atom",
    "calculator",
    "compass",
    "git-branch",
    "palette",
];

/// The 8 workspace accents, in `--cluster-1..8` order (web `HUE_NAMES`).
pub const HUES: [&str; 8] = [
    "violet", "teal", "orange", "blue", "sprout", "rose", "gold", "lilac",
];

pub const DEFAULT_ICON: &str = "book-open";
pub const DEFAULT_HUE: &str = "violet";
/// Name length bounds, in characters.
pub const NAME_MAX: usize = 60;
pub const DESCRIPTION_MAX: usize = 500;

/// One workspace's `workspace.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    /// `ws_` + 8 lowercase hex digits; also the directory name. Routing always uses this.
    pub id: String,
    pub name: String,
    /// URL-safe, unique, display only.
    #[serde(default)]
    pub slug: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_hue")]
    pub hue: String,
    #[serde(default)]
    pub description: String,
    /// Epoch ms.
    #[serde(default)]
    pub created_at: i64,
    /// Epoch ms.
    #[serde(default)]
    pub last_opened_at: i64,
    /// Archived workspaces are read-only and hidden from the default list.
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub sources: Sources,
    #[serde(default)]
    pub overrides: Overrides,
    #[serde(default = "default_schema")]
    pub schema: u32,
}

fn default_icon() -> String {
    DEFAULT_ICON.into()
}
fn default_hue() -> String {
    DEFAULT_HUE.into()
}
fn default_schema() -> u32 {
    SCHEMA
}

/// Which transcripts this workspace ingests (docs/web/02 §Ingest scoping).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sources {
    pub claude: bool,
    pub codex: bool,
    pub agy: bool,
    pub git: bool,
    /// Project directories whose sessions belong here. Empty disables ingest.
    pub allow: Vec<PathBuf>,
    pub since_days: u32,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            claude: true,
            codex: true,
            agy: true,
            git: true,
            allow: vec![],
            since_days: 30,
        }
    }
}

/// Per-workspace overrides of the global settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Overrides {
    /// Role -> profile, overriding the global profile table for this workspace.
    pub profiles: BTreeMap<String, Profile>,
}

/// Input of [`WorkspaceRegistry::create`](super::WorkspaceRegistry::create).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewWorkspace {
    pub name: String,
    /// Defaults to [`DEFAULT_ICON`].
    pub icon: Option<String>,
    /// Defaults to [`DEFAULT_HUE`].
    pub hue: Option<String>,
    pub description: Option<String>,
    /// Seed sources, e.g. from detected project dirs; defaults to all tools on, empty allow-list.
    pub sources: Option<Sources>,
}

/// A PATCH of the editable fields; `None` leaves a field unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspacePatch {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub hue: Option<String>,
    pub description: Option<String>,
    pub sources: Option<Sources>,
    pub overrides: Option<Overrides>,
}

impl WorkspaceMeta {
    /// Fresh metadata for a new workspace; `name` must already be validated.
    pub(crate) fn new(id: String, name: String, now: i64) -> Self {
        Self {
            slug: slugify(&name),
            id,
            name,
            icon: default_icon(),
            hue: default_hue(),
            description: String::new(),
            created_at: now,
            last_opened_at: now,
            archived: false,
            sources: Sources::default(),
            overrides: Overrides::default(),
            schema: SCHEMA,
        }
    }

    /// Read `<dir>/workspace.json`, checking it belongs to `dir`. Any problem is
    /// [`WorkspaceError::Damaged`] (or `NotFound` when the directory is gone).
    pub fn load(dir: &Path) -> Result<Self> {
        let id = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if !dir.is_dir() {
            return Err(WorkspaceError::NotFound(id));
        }
        let damaged = |reason: String| WorkspaceError::Damaged {
            id: id.clone(),
            reason,
        };
        let mut meta: Self = match fsx::read_json(&dir.join(META_FILE)) {
            Ok(Some(m)) => m,
            Ok(None) => return Err(damaged(format!("{META_FILE} is missing"))),
            Err(e) => return Err(damaged(format!("{META_FILE} is unreadable: {e}"))),
        };
        meta.created_at = lp_core::ports::epoch_ms(meta.created_at);
        meta.last_opened_at = lp_core::ports::epoch_ms(meta.last_opened_at);
        if meta.id != id {
            return Err(damaged(format!("{META_FILE} names id {:?}", meta.id)));
        }
        if meta.schema > SCHEMA {
            return Err(damaged(format!(
                "schema {} is newer than this build ({SCHEMA})",
                meta.schema
            )));
        }
        Ok(meta)
    }

    /// Atomic write of `<dir>/workspace.json`.
    pub fn save(&self, dir: &Path) -> Result<()> {
        fsx::write_json(&dir.join(META_FILE), self)
    }

    /// Apply a validated patch (names are checked for uniqueness by the registry).
    pub(crate) fn apply(&mut self, patch: WorkspacePatch) -> Result<()> {
        if let Some(name) = patch.name {
            self.name = validate_name(&name)?;
        }
        if let Some(icon) = patch.icon {
            self.icon = validate_icon(&icon)?;
        }
        if let Some(hue) = patch.hue {
            self.hue = validate_hue(&hue)?;
        }
        if let Some(d) = patch.description {
            self.description = validate_description(&d)?;
        }
        if let Some(s) = patch.sources {
            self.sources = validate_sources(s)?;
        }
        if let Some(o) = patch.overrides {
            self.overrides = o;
        }
        Ok(())
    }
}

/// A fresh random workspace id, `ws_` + 8 hex digits.
pub fn new_id() -> String {
    format!("ws_{:08x}", rand::random::<u32>())
}

/// Whether `id` has the `ws_<lowercase alnum>` shape (and so is safe as a directory name).
pub fn is_valid_id(id: &str) -> bool {
    id.strip_prefix("ws_").is_some_and(|rest| {
        !rest.is_empty()
            && rest.len() <= 32
            && rest
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// Trimmed name of 1–60 characters without control characters.
pub fn validate_name(name: &str) -> Result<String> {
    let name = name.trim();
    let len = name.chars().count();
    if len == 0 {
        return Err(WorkspaceError::validation("name", "must not be empty"));
    }
    if len > NAME_MAX {
        return Err(WorkspaceError::validation(
            "name",
            format!("must be at most {NAME_MAX} characters"),
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(WorkspaceError::validation(
            "name",
            "must not contain control characters",
        ));
    }
    Ok(name.to_owned())
}

pub fn validate_icon(icon: &str) -> Result<String> {
    if ICONS.contains(&icon) {
        Ok(icon.to_owned())
    } else {
        Err(WorkspaceError::validation(
            "icon",
            format!("unknown icon {icon:?}"),
        ))
    }
}

pub fn validate_hue(hue: &str) -> Result<String> {
    if HUES.contains(&hue) {
        Ok(hue.to_owned())
    } else {
        Err(WorkspaceError::validation(
            "hue",
            format!("unknown hue {hue:?} (one of: {})", HUES.join(", ")),
        ))
    }
}

pub fn validate_description(d: &str) -> Result<String> {
    let d = d.trim();
    if d.chars().count() > DESCRIPTION_MAX {
        return Err(WorkspaceError::validation(
            "description",
            format!("must be at most {DESCRIPTION_MAX} characters"),
        ));
    }
    Ok(d.to_owned())
}

/// Allow-list paths must be absolute; duplicates are dropped, order kept.
pub fn validate_sources(mut s: Sources) -> Result<Sources> {
    let mut seen = Vec::with_capacity(s.allow.len());
    for p in std::mem::take(&mut s.allow) {
        if !p.is_absolute() {
            return Err(WorkspaceError::validation(
                "sources",
                format!("allow path {} must be absolute", p.display()),
            ));
        }
        if !seen.contains(&p) {
            seen.push(p);
        }
    }
    s.allow = seen;
    Ok(s)
}

/// Case-insensitive name comparison key.
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Lowercase ASCII words joined by `-`; `workspace` when nothing is left.
pub fn slugify(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "workspace".into()
    } else {
        slug.chars()
            .take(48)
            .collect::<String>()
            .trim_end_matches('-')
            .into()
    }
}

/// `base`, else `base-2`, `base-3`, … whichever is not in `taken`.
pub(crate) fn unique_slug(base: &str, taken: &[String]) -> String {
    (1..)
        .map(|n| {
            if n == 1 {
                base.to_owned()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|s| !taken.contains(s))
        .unwrap_or_else(|| base.to_owned())
}

/// `name`, else `name (2)`, `name (3)`, … whichever is free (case-insensitive) in `taken`,
/// trimming `name` so the result stays within [`NAME_MAX`].
pub(crate) fn unique_name(name: &str, taken: &[String]) -> String {
    let taken: Vec<String> = taken.iter().map(|n| name_key(n)).collect();
    if !taken.contains(&name_key(name)) {
        return name.to_owned();
    }
    (2..)
        .map(|n| {
            let suffix = format!(" ({n})");
            let room = NAME_MAX.saturating_sub(suffix.chars().count());
            let stem: String = name.chars().take(room).collect();
            format!("{}{suffix}", stem.trim_end())
        })
        .find(|c| !taken.contains(&name_key(c)))
        .unwrap_or_else(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_example_round_trips() {
        let json = r#"{
          "id": "ws_a1b2c3d4", "name": "Rust async", "slug": "rust-async",
          "icon": "book-open", "hue": "violet",
          "description": "", "created_at": 1761700000000, "last_opened_at": 1761700000000,
          "archived": false,
          "sources": { "claude": true, "codex": true, "agy": true, "git": true,
                       "allow": ["/Users/pratham/code/foo"], "since_days": 30 },
          "overrides": { "profiles": {} },
          "schema": 1
        }"#;
        let m: WorkspaceMeta = serde_json::from_str(json).unwrap();
        assert_eq!(m.slug, "rust-async");
        assert_eq!(m.sources.allow, [PathBuf::from("/Users/pratham/code/foo")]);
        let back: WorkspaceMeta =
            serde_json::from_value(serde_json::to_value(&m).unwrap()).unwrap();
        assert_eq!(back, m);
        // Minimal files from older builds fill in defaults.
        let m: WorkspaceMeta = serde_json::from_str(r#"{"id":"ws_1","name":"x"}"#).unwrap();
        assert_eq!(
            (m.icon.as_str(), m.hue.as_str()),
            (DEFAULT_ICON, DEFAULT_HUE)
        );
        assert_eq!(m.sources, Sources::default());
        assert_eq!(m.schema, SCHEMA);
    }

    #[test]
    fn second_based_workspace_times_load_as_milliseconds() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("ws_sec");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(META_FILE),
            r#"{"id":"ws_sec","name":"x","created_at":1793131000,"last_opened_at":1793131171}"#,
        )
        .unwrap();
        let m = WorkspaceMeta::load(&dir).unwrap();
        assert_eq!(
            (m.created_at, m.last_opened_at),
            (1_793_131_000_000, 1_793_131_171_000)
        );
    }

    #[test]
    fn names_slugs_icons_and_hues_are_validated() {
        assert_eq!(validate_name("  Rust async ").unwrap(), "Rust async");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(61)).is_err());
        assert!(validate_name(&"é".repeat(60)).is_ok());
        assert!(validate_name("a\nb").is_err());
        assert_eq!(slugify("Rust async!! (v2)"), "rust-async-v2");
        assert_eq!(slugify("日本語"), "workspace");
        assert_eq!(unique_slug("a", &["a".into(), "a-2".into()]), "a-3");
        assert_eq!(unique_name("Notes", &["notes".into()]), "Notes (2)");
        assert_eq!(
            unique_name("Notes", &["NOTES".into(), "notes (2)".into()]),
            "Notes (3)"
        );
        assert_eq!(
            unique_name(&"n".repeat(60), &["n".repeat(60)])
                .chars()
                .count(),
            60
        );
        assert_eq!(ICONS.len(), 24);
        assert!(validate_icon("rocket").is_ok() && validate_icon("🚀").is_err());
        assert!(validate_hue("teal").is_ok() && validate_hue("red").is_err());
        assert!(validate_sources(Sources {
            allow: vec!["rel/path".into()],
            ..Sources::default()
        })
        .is_err());
        let s = validate_sources(Sources {
            allow: vec!["/a".into(), "/a".into(), "/b".into()],
            ..Sources::default()
        })
        .unwrap();
        assert_eq!(s.allow, [PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn ids_are_directory_safe() {
        let id = new_id();
        assert!(is_valid_id(&id) && id.len() == 11, "{id}");
        for bad in [
            "ws_", "ws_../x", "WS_abc", "ws_ABC", "abc", "ws_a/b", ".trash",
        ] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }
}
