//! Settings, catalog and accounts wire types (docs/web/03 §Bootstrap / global).

use std::collections::BTreeMap;

use lp_service::accounts::{Account, Status};
use lp_service::settings::{
    self, LearningStyle, Profile, Settings, UiSettings, CONCURRENCY_RANGE, DEFAULT_PIN,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::system::AccountStatus;
use crate::error::FieldError;

/// Which CLI, model and reasoning effort a role runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    pub provider: String,
    pub model: String,
    pub effort: Option<String>,
    /// Let the account pool fall back to other providers.
    #[serde(default)]
    pub fallback: bool,
}

impl From<Profile> for ProfileDto {
    fn from(p: Profile) -> Self {
        Self {
            provider: p.provider,
            model: p.model,
            effort: p.effort,
            fallback: p.fallback,
        }
    }
}

impl ProfileDto {
    /// Checks provider and effort; the error is the reason.
    pub fn into_profile(self) -> Result<Profile, String> {
        let mut p = Profile::new(&self.provider, &self.model, self.effort.as_deref())
            .map_err(|e| e.to_string())?;
        p.fallback = self.fallback;
        Ok(p)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UiSettingsDto {
    /// `dark`, `light` or `system`.
    pub theme: String,
    /// `ltr`, `rtl` or `auto`.
    pub direction: String,
    /// `comfortable` or `compact`.
    pub density: String,
    pub reduce_motion: bool,
    pub confirm_destructive: bool,
    pub auto_open_browser: bool,
}

impl From<UiSettings> for UiSettingsDto {
    fn from(u: UiSettings) -> Self {
        Self {
            theme: u.theme,
            direction: u.direction,
            density: u.density,
            reduce_motion: u.reduce_motion,
            confirm_destructive: u.confirm_destructive,
            auto_open_browser: u.auto_open_browser,
        }
    }
}

impl From<UiSettingsDto> for UiSettings {
    fn from(u: UiSettingsDto) -> Self {
        Self {
            theme: u.theme,
            direction: u.direction,
            density: u.density,
            reduce_motion: u.reduce_motion,
            confirm_destructive: u.confirm_destructive,
            auto_open_browser: u.auto_open_browser,
        }
    }
}

/// The global settings the UI edits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    /// Role → profile; roles without one are absent.
    pub profiles: BTreeMap<String, ProfileDto>,
    /// Role → `<provider>:auto` or an account id.
    pub pins: BTreeMap<String, String>,
    /// `default` or `visual`.
    pub learning_style: String,
    /// Parallel model calls, `1..=6`.
    pub concurrency_cap: u8,
    pub ui: UiSettingsDto,
}

impl From<&Settings> for SettingsDto {
    fn from(s: &Settings) -> Self {
        Self {
            profiles: s
                .profiles
                .iter()
                .map(|(k, v)| (k.clone(), v.clone().into()))
                .collect(),
            pins: s.pins.clone(),
            learning_style: match s.learning_style {
                LearningStyle::Default => "default",
                LearningStyle::Visual => "visual",
            }
            .into(),
            concurrency_cap: s.concurrency(),
            ui: s.ui.clone().into(),
        }
    }
}

fn field(path: impl Into<String>, message: impl Into<String>) -> FieldError {
    FieldError {
        path: path.into(),
        message: message.into(),
    }
}

impl SettingsDto {
    /// Checks every field against the catalog and the live accounts (`account_ids`) and returns
    /// `current` with this applied. Unrelated settings (device id, Claude profile dirs, disabled
    /// accounts) are kept.
    pub fn apply_to(
        self,
        current: &Settings,
        account_ids: &[String],
    ) -> Result<Settings, Vec<FieldError>> {
        let mut errors = Vec::new();
        let mut out = current.clone();
        out.profiles.clear();
        for (role, dto) in self.profiles {
            let path = format!("profiles.{role}");
            if !settings::PROFILE_ROLES.contains(&role.as_str()) {
                errors.push(field(&path, format!("unknown role `{role}`")));
                continue;
            }
            let (provider, model) = (dto.provider.clone(), dto.model.clone());
            match dto.into_profile() {
                Err(m) if m.contains("provider") => {
                    errors.push(field(format!("{path}.provider"), m));
                }
                Err(m) if m.contains("effort") => {
                    errors.push(field(format!("{path}.effort"), m));
                }
                Err(m) => errors.push(field(format!("{path}.model"), m)),
                Ok(p) => {
                    if !settings::catalog_models(&provider, current).contains(&model) {
                        errors.push(field(
                            format!("{path}.model"),
                            format!("`{model}` is not in the catalog for {provider}"),
                        ));
                    } else {
                        out.profiles.insert(role, p);
                    }
                }
            }
        }
        out.pins.clear();
        for (role, pin) in self.pins {
            let path = format!("pins.{role}");
            if !settings::ROLES.contains(&role.as_str())
                && !settings::PROFILE_ROLES.contains(&role.as_str())
            {
                errors.push(field(&path, format!("unknown role `{role}`")));
            } else if !valid_pin(&pin, account_ids) {
                errors.push(field(
                    &path,
                    format!("`{pin}` is neither `<provider>:auto` nor a known account"),
                ));
            } else {
                out.pins.insert(role, pin);
            }
        }
        match LearningStyle::parse(&self.learning_style) {
            Ok(s) => out.learning_style = s,
            Err(e) => errors.push(field("learningStyle", e.to_string())),
        }
        if CONCURRENCY_RANGE.contains(&self.concurrency_cap) {
            out.concurrency_cap = self.concurrency_cap;
        } else {
            errors.push(field(
                "concurrencyCap",
                format!(
                    "must be between {} and {}",
                    CONCURRENCY_RANGE.start(),
                    CONCURRENCY_RANGE.end()
                ),
            ));
        }
        let ui: UiSettings = self.ui.into();
        errors.extend(ui.validate().into_iter().map(|(p, m)| field(p, m)));
        out.ui = ui;
        if errors.is_empty() {
            Ok(out)
        } else {
            Err(errors)
        }
    }
}

fn valid_pin(pin: &str, account_ids: &[String]) -> bool {
    pin == DEFAULT_PIN
        || account_ids.iter().any(|a| a == pin)
        || pin
            .strip_suffix(":auto")
            .is_some_and(|p| settings::PROVIDERS.contains(&p))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CatalogProviderDto {
    pub id: String,
    pub models: Vec<String>,
}

/// What the settings forms may offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDto {
    pub providers: Vec<CatalogProviderDto>,
    pub efforts: Vec<String>,
    /// Roles that take a profile.
    pub roles: Vec<String>,
    /// Slots that take a pin, in display order.
    pub pin_roles: Vec<String>,
    pub themes: Vec<String>,
    pub directions: Vec<String>,
    pub densities: Vec<String>,
}

impl CatalogDto {
    pub fn from_settings(s: &Settings) -> Self {
        let strs = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        Self {
            providers: settings::PROVIDERS
                .iter()
                .map(|p| CatalogProviderDto {
                    id: (*p).to_owned(),
                    models: settings::catalog_models(p, s),
                })
                .collect(),
            efforts: strs(&settings::EFFORTS),
            roles: strs(&settings::PROFILE_ROLES),
            pin_roles: strs(&settings::ROLES),
            themes: strs(&settings::THEMES),
            directions: strs(&settings::DIRECTIONS),
            densities: strs(&settings::DENSITIES),
        }
    }
}

/// One CLI account as the accounts screen shows it. Never carries tokens or credentials.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub id: String,
    /// `claude`, `codex` or `agy`.
    pub provider: String,
    pub email: Option<String>,
    pub tier: Option<String>,
    pub status: AccountStatus,
    /// Epoch ms when a cooldown ends.
    pub cooldown_until: Option<i64>,
    pub cooldown_reason: Option<String>,
    /// Requests in the current rate-limit window.
    pub requests: u32,
    pub tokens_in: u64,
    pub tokens_out: u64,
    /// The command that renews an expired login.
    pub reauth_hint: Option<String>,
}

impl AccountDto {
    /// `now_ms` anchors a cooldown's remaining seconds to a wall-clock deadline.
    pub fn new(a: Account, now_ms: i64) -> Self {
        let hint = (matches!(a.status, Status::AuthExpired)
            || matches!(&a.status, Status::Cooldown { reason, .. } if reason == "auth expired"))
        .then(|| match (a.provider, &a.config_dir) {
            ("claude", Some(dir)) => {
                format!("CLAUDE_CONFIG_DIR={} claude login", dir.display())
            }
            ("claude", None) => "claude login".to_owned(),
            (p, _) => format!("{p} login"),
        });
        let (requests, tokens_in, tokens_out) = (a.requests, a.tokens_in, a.tokens_out);
        let summary = super::system::AccountSummaryDto::from_account(a, now_ms);
        Self {
            id: summary.id,
            provider: summary.provider,
            email: summary.email,
            tier: summary.tier,
            status: summary.status,
            cooldown_until: summary.cooldown_until,
            cooldown_reason: summary.cooldown_reason,
            requests,
            tokens_in,
            tokens_out,
            reauth_hint: hint,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AccountsDto {
    pub items: Vec<AccountDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RescanDto {
    pub items: Vec<AccountDto>,
    /// Ids of accounts that were not in the pool before.
    pub added: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct AddClaudeProfile {
    /// A `CLAUDE_CONFIG_DIR` holding `.claude.json`.
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct PatchAccount {
    /// Exclude the account from the pool (or put it back).
    pub disabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dto() -> SettingsDto {
        let mut s = Settings::default();
        s.apply_defaults();
        SettingsDto::from(&s)
    }

    #[test]
    fn defaults_round_trip_through_validation() {
        let mut current = Settings::default();
        current.apply_defaults();
        current.device_id = "dev1".into();
        current.disabled_accounts.push("claude:a".into());
        let out = SettingsDto::from(&current).apply_to(&current, &[]).unwrap();
        assert_eq!(out, {
            let mut c = current.clone();
            c.concurrency_cap = 2;
            c
        });
        let j = serde_json::to_value(dto()).unwrap();
        assert_eq!(j["learningStyle"], "visual");
        assert_eq!(j["ui"]["reduceMotion"], false);
        assert_eq!(j["profiles"]["teacher"]["provider"], "agy");
    }

    #[test]
    fn invalid_settings_list_each_bad_field() {
        let mut d = dto();
        d.profiles.insert(
            "nobody".into(),
            ProfileDto {
                provider: "codex".into(),
                model: "gpt-6.1-sol".into(),
                effort: None,
                fallback: false,
            },
        );
        d.profiles.get_mut("teacher").unwrap().provider = "gpt".into();
        d.profiles.get_mut("prober").unwrap().model = "made-up".into();
        d.profiles.get_mut("demo").unwrap().effort = Some("turbo".into());
        d.pins.insert("teacher".into(), "claude:ghost".into());
        d.pins.insert("zzz".into(), "claude:auto".into());
        d.learning_style = "loud".into();
        d.concurrency_cap = 9;
        d.ui.theme = "neon".into();
        let errs = d.apply_to(&Settings::default(), &[]).unwrap_err();
        let paths: Vec<_> = errs.iter().map(|e| e.path.as_str()).collect();
        for want in [
            "profiles.nobody",
            "profiles.teacher.provider",
            "profiles.prober.model",
            "profiles.demo.effort",
            "pins.teacher",
            "pins.zzz",
            "learningStyle",
            "concurrencyCap",
            "ui.theme",
        ] {
            assert!(paths.contains(&want), "{want} missing from {paths:?}");
        }
    }

    #[test]
    fn pins_accept_provider_auto_and_known_accounts() {
        let mut d = dto();
        d.pins.insert("teacher".into(), "agy:auto".into());
        d.pins.insert("prober".into(), "claude:me@x.com".into());
        let out = d
            .apply_to(&Settings::default(), &["claude:me@x.com".to_owned()])
            .unwrap();
        assert_eq!(out.pins["prober"], "claude:me@x.com");
    }

    #[test]
    fn catalog_lists_every_provider_with_models() {
        let c = CatalogDto::from_settings(&Settings::default());
        let ids: Vec<_> = c.providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["codex", "agy", "claude"]);
        assert!(c.providers[0].models.contains(&"gpt-6.1-sol".to_owned()));
        assert!(c.providers[2].models.contains(&"sonnet".to_owned()));
        assert!(c.efforts.contains(&"xhigh".to_owned()));
        assert_eq!(c.roles.len(), 9);
    }

    #[test]
    fn account_dto_hints_reauth_and_carries_no_secrets() {
        let a = Account {
            id: "claude:a@x.com".into(),
            provider: "claude",
            email: Some("a@x.com".into()),
            tier: None,
            config_dir: Some("/home/me/.claude-work".into()),
            status: Status::AuthExpired,
            requests: 3,
            tokens_in: 10,
            tokens_out: 20,
        };
        let d = AccountDto::new(a, 0);
        assert_eq!(
            d.reauth_hint.as_deref(),
            Some("CLAUDE_CONFIG_DIR=/home/me/.claude-work claude login")
        );
        assert_eq!(d.status, AccountStatus::AuthExpired);
        let j = serde_json::to_value(&d).unwrap();
        assert_eq!(
            (j["tokensIn"].as_u64(), j["requests"].as_u64()),
            (Some(10), Some(3))
        );
        assert!(j.get("configDir").is_none());
    }
}
