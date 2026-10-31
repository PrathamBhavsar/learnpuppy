//! Workspace wire types (docs/web/02, 03 §Workspaces).

use std::collections::BTreeMap;
use std::path::PathBuf;

use lp_service::workspace::{
    DamagedWorkspace, NewWorkspace, Overrides, Sources, TrashEntry, WorkspaceMeta, WorkspacePatch,
    WorkspaceStats,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::settings::ProfileDto;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourcesDto {
    pub claude: bool,
    pub codex: bool,
    pub agy: bool,
    pub git: bool,
    /// Project directories whose sessions belong to this workspace; empty disables ingest.
    pub allow: Vec<String>,
    pub since_days: u32,
}

impl From<Sources> for SourcesDto {
    fn from(s: Sources) -> Self {
        Self {
            claude: s.claude,
            codex: s.codex,
            agy: s.agy,
            git: s.git,
            allow: s.allow.iter().map(|p| p.display().to_string()).collect(),
            since_days: s.since_days,
        }
    }
}

impl From<SourcesDto> for Sources {
    fn from(s: SourcesDto) -> Self {
        Self {
            claude: s.claude,
            codex: s.codex,
            agy: s.agy,
            git: s.git,
            allow: s.allow.into_iter().map(PathBuf::from).collect(),
            since_days: s.since_days,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverridesDto {
    /// Role → profile, replacing the global one in this workspace.
    pub profiles: BTreeMap<String, ProfileDto>,
}

impl From<Overrides> for OverridesDto {
    fn from(o: Overrides) -> Self {
        Self {
            profiles: o.profiles.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

impl OverridesDto {
    pub fn into_overrides(self) -> Result<Overrides, (String, String)> {
        let mut profiles = BTreeMap::new();
        for (role, p) in self.profiles {
            let path = format!("overrides.profiles.{role}");
            if !lp_service::settings::PROFILE_ROLES.contains(&role.as_str()) {
                return Err((path, format!("unknown role `{role}`")));
            }
            let profile = p.into_profile().map_err(|m| (path, m))?;
            profiles.insert(role, profile);
        }
        Ok(Overrides { profiles })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatsDto {
    pub nodes: usize,
    pub chains: usize,
    pub chats: usize,
    pub due_cards: usize,
    pub last_activity_at: Option<i64>,
}

impl From<WorkspaceStats> for WorkspaceStatsDto {
    fn from(s: WorkspaceStats) -> Self {
        Self {
            nodes: s.nodes,
            chains: s.chains,
            chats: s.chats,
            due_cards: s.due_cards,
            last_activity_at: s.last_activity_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    /// `ws_` + 8 hex digits; routing always uses this.
    pub id: String,
    pub name: String,
    pub slug: String,
    /// A lucide icon name from the fixed allow-list.
    pub icon: String,
    /// One of the 8 palette accents.
    pub hue: String,
    pub description: String,
    pub created_at: i64,
    pub last_opened_at: i64,
    /// Archived workspaces are read-only: mutations answer `409 workspace_archived`.
    pub archived: bool,
    pub sources: SourcesDto,
    pub overrides: OverridesDto,
    /// Only in listings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<WorkspaceStatsDto>,
}

impl WorkspaceDto {
    pub fn new(m: WorkspaceMeta, stats: Option<WorkspaceStats>) -> Self {
        Self {
            id: m.id,
            name: m.name,
            slug: m.slug,
            icon: m.icon,
            hue: m.hue,
            description: m.description,
            created_at: m.created_at,
            last_opened_at: m.last_opened_at,
            archived: m.archived,
            sources: m.sources.into(),
            overrides: m.overrides.into(),
            stats: stats.map(Into::into),
        }
    }
}

impl From<WorkspaceMeta> for WorkspaceDto {
    fn from(m: WorkspaceMeta) -> Self {
        Self::new(m, None)
    }
}

/// A workspace directory whose `workspace.json` is missing or corrupt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DamagedWorkspaceDto {
    pub id: String,
    /// Where the directory is, for "Reveal".
    pub path: String,
    pub reason: String,
}

impl From<DamagedWorkspace> for DamagedWorkspaceDto {
    fn from(d: DamagedWorkspace) -> Self {
        Self {
            id: d.id,
            path: d.path.display().to_string(),
            reason: d.reason,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListDto {
    /// Most recently opened first.
    pub items: Vec<WorkspaceDto>,
    pub damaged: Vec<DamagedWorkspaceDto>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListWorkspacesQuery {
    /// Include archived workspaces.
    #[serde(default)]
    pub archived: bool,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspace {
    pub name: String,
    pub icon: Option<String>,
    pub hue: Option<String>,
    pub description: Option<String>,
    pub sources: Option<SourcesDto>,
}

impl From<CreateWorkspace> for NewWorkspace {
    fn from(c: CreateWorkspace) -> Self {
        Self {
            name: c.name,
            icon: c.icon,
            hue: c.hue,
            description: c.description,
            sources: c.sources.map(Into::into),
        }
    }
}

/// Fields left out stay as they are.
#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatchWorkspace {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub hue: Option<String>,
    pub description: Option<String>,
    pub sources: Option<SourcesDto>,
    pub overrides: Option<OverridesDto>,
}

impl PatchWorkspace {
    pub fn into_patch(self) -> Result<WorkspacePatch, (String, String)> {
        Ok(WorkspacePatch {
            name: self.name,
            icon: self.icon,
            hue: self.hue,
            description: self.description,
            sources: self.sources.map(Into::into),
            overrides: self
                .overrides
                .map(OverridesDto::into_overrides)
                .transpose()?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteWorkspace {
    /// The workspace's exact name (its id for a damaged one).
    pub confirm_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrashEntryDto {
    /// Key for restore and purge.
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub deleted_at: i64,
    /// Bytes the trashed workspace takes on disk.
    pub bytes: u64,
}

impl TrashEntryDto {
    pub fn new(e: TrashEntry, bytes: u64) -> Self {
        Self {
            id: e.key,
            workspace_id: e.id,
            name: e.name,
            deleted_at: e.deleted_at,
            bytes,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TrashListDto {
    pub items: Vec<TrashEntryDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_dto_is_camel_case_and_round_trips() {
        let m: WorkspaceMeta = serde_json::from_value(serde_json::json!({
            "id": "ws_a", "name": "A", "last_opened_at": 5
        }))
        .unwrap();
        let dto = WorkspaceDto::new(m, None);
        let j = serde_json::to_value(&dto).unwrap();
        assert_eq!(j["lastOpenedAt"], 5);
        assert_eq!(j["sources"]["sinceDays"], 30);
        assert!(j.get("stats").is_none());
        assert_eq!(serde_json::from_value::<WorkspaceDto>(j).unwrap(), dto);
    }

    #[test]
    fn patch_validates_override_roles_and_profiles() {
        let p: PatchWorkspace = serde_json::from_value(serde_json::json!({
            "overrides": {"profiles": {"nobody": {"provider": "codex", "model": "m"}}}
        }))
        .unwrap();
        assert_eq!(p.into_patch().unwrap_err().0, "overrides.profiles.nobody");
        let p: PatchWorkspace = serde_json::from_value(serde_json::json!({
            "overrides": {"profiles": {"teacher": {"provider": "gpt", "model": "m"}}}
        }))
        .unwrap();
        assert_eq!(p.into_patch().unwrap_err().0, "overrides.profiles.teacher");
        let p: PatchWorkspace = serde_json::from_value(serde_json::json!({"name": "X"})).unwrap();
        assert_eq!(p.into_patch().unwrap().name.as_deref(), Some("X"));
    }
}
