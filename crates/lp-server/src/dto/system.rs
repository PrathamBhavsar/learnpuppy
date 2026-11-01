//! `/health` and `/bootstrap`.

use lp_service::accounts::{Account, Status};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::directory::{Banner, BannerLevel, WorkspaceSummary};

/// Which LLM engine the server runs turns with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EngineMode {
    Real,
    Faux,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub ok: bool,
    pub version: String,
    pub engine: EngineMode,
    pub data_dir: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummaryDto {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub icon: String,
    pub hue: String,
    pub archived: bool,
    pub last_opened_at: Option<i64>,
}

impl From<WorkspaceSummary> for WorkspaceSummaryDto {
    fn from(w: WorkspaceSummary) -> Self {
        Self {
            id: w.id,
            name: w.name,
            slug: w.slug,
            icon: w.icon,
            hue: w.hue,
            archived: w.archived,
            last_opened_at: w.last_opened_at,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Ready,
    Cooldown,
    /// Signed out; the user runs the connect command (`howToConnect`).
    AuthExpired,
    Disabled,
    /// No network. Not the account's fault.
    Offline,
    /// Logged in but no model can run.
    NoModel,
    /// The CLI is not installed.
    Missing,
}

/// An account without credentials: provider, identity, health.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummaryDto {
    pub id: String,
    pub provider: String,
    pub email: Option<String>,
    pub tier: Option<String>,
    pub status: AccountStatus,
    /// Epoch ms when a cooldown ends.
    pub cooldown_until: Option<i64>,
    pub cooldown_reason: Option<String>,
}

impl AccountSummaryDto {
    /// The wire status of a plain `Status` (cooldown deadline anchored at `now_ms`).
    pub fn status_of(s: &Status, now_ms: i64) -> (AccountStatus, Option<i64>) {
        match s {
            Status::Ready => (AccountStatus::Ready, None),
            Status::AuthExpired => (AccountStatus::AuthExpired, None),
            Status::Disabled => (AccountStatus::Disabled, None),
            Status::Offline => (AccountStatus::Offline, None),
            Status::NoModel => (AccountStatus::NoModel, None),
            Status::Missing => (AccountStatus::Missing, None),
            Status::Cooldown { secs_left, .. } => (
                AccountStatus::Cooldown,
                Some(now_ms.saturating_add(
                    i64::try_from(secs_left.saturating_mul(1000)).unwrap_or(i64::MAX),
                )),
            ),
        }
    }

    /// `now_ms` anchors the cooldown's remaining seconds to a wall-clock deadline.
    pub fn from_account(a: Account, now_ms: i64) -> Self {
        let (status, cooldown_until, cooldown_reason) = match a.status {
            Status::Ready => (AccountStatus::Ready, None, None),
            Status::AuthExpired => (AccountStatus::AuthExpired, None, None),
            Status::Disabled => (AccountStatus::Disabled, None, None),
            Status::Offline => (AccountStatus::Offline, None, None),
            Status::NoModel => (AccountStatus::NoModel, None, None),
            Status::Missing => (AccountStatus::Missing, None, None),
            Status::Cooldown { secs_left, reason } => (
                AccountStatus::Cooldown,
                Some(now_ms.saturating_add(
                    i64::try_from(secs_left.saturating_mul(1000)).unwrap_or(i64::MAX),
                )),
                Some(reason),
            ),
        };
        Self {
            id: a.id,
            provider: a.provider.to_owned(),
            email: a.email,
            tier: a.tier,
            status,
            cooldown_until,
            cooldown_reason,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BannerLevelDto {
    Info,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BannerDto {
    pub id: String,
    pub level: BannerLevelDto,
    pub message: String,
}

impl From<Banner> for BannerDto {
    fn from(b: Banner) -> Self {
        Self {
            id: b.id,
            level: match b.level {
                BannerLevel::Info => BannerLevelDto::Info,
                BannerLevel::Warning => BannerLevelDto::Warning,
            },
            message: b.message,
        }
    }
}

/// Everything the app needs at start, in one call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub version: String,
    pub server_time: i64,
    pub device_id: String,
    pub engine: EngineMode,
    /// The server runs without a launch token (`serve --dev`).
    pub dev: bool,
    pub workspaces: Vec<WorkspaceSummaryDto>,
    pub last_workspace_id: Option<String>,
    pub accounts: Vec<AccountSummaryDto>,
    pub banners: Vec<BannerDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(v: &T) -> serde_json::Value
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_value(v).unwrap();
        assert_eq!(&serde_json::from_value::<T>(json.clone()).unwrap(), v);
        json
    }

    #[test]
    fn health_round_trips() {
        let j = round_trip(&Health {
            ok: true,
            version: "1".into(),
            engine: EngineMode::Faux,
            data_dir: "/d".into(),
        });
        assert_eq!(j["engine"], "faux");
        assert_eq!(j["dataDir"], "/d");
    }

    #[test]
    fn bootstrap_round_trips_and_accounts_carry_no_paths() {
        let cooldown = Account {
            id: "claude:a@x.com".into(),
            provider: "claude",
            email: Some("a@x.com".into()),
            tier: Some("max".into()),
            config_dir: Some("/secret/dir".into()),
            status: Status::Cooldown {
                secs_left: 30,
                reason: "usage limit".into(),
            },
            requests: 3,
            tokens_in: 0,
            tokens_out: 0,
            resets_at: None,
            models: Vec::new(),
        };
        let acct = AccountSummaryDto::from_account(cooldown, 1_000);
        assert_eq!(acct.status, AccountStatus::Cooldown);
        assert_eq!(acct.cooldown_until, Some(31_000));
        let b = Bootstrap {
            version: "1".into(),
            server_time: 5,
            device_id: "dev1".into(),
            engine: EngineMode::Real,
            dev: false,
            workspaces: vec![WorkspaceSummaryDto::from(WorkspaceSummary {
                id: "ws_a".into(),
                name: "A".into(),
                slug: "a".into(),
                icon: "brain".into(),
                hue: "teal".into(),
                archived: false,
                last_opened_at: Some(1),
            })],
            last_workspace_id: Some("ws_a".into()),
            accounts: vec![acct],
            banners: vec![BannerDto::from(Banner {
                id: "migrated".into(),
                level: BannerLevel::Info,
                message: "m".into(),
            })],
        };
        let j = round_trip(&b);
        assert_eq!(j["accounts"][0]["status"], "cooldown");
        assert_eq!(j["workspaces"][0]["lastOpenedAt"], 1);
        assert_eq!(j["banners"][0]["level"], "info");
        assert!(!j.to_string().contains("/secret/dir"));
    }
}
