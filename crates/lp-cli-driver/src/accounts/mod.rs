//! Local CLI account discovery and the failover pool built on it.

mod discover;
mod pool;

pub use discover::{discover, DiscoverRoots};
pub use pool::{
    parse_cooldown, AccountHealth, AccountPool, AccountState, AccountStatus, AccountStatusRow,
    ModelState, ModelStatusRow, Pick, Strategy,
};

use std::path::PathBuf;

use uuid::Uuid;

/// Stable account key, e.g. `claude:me@example.com` or `agy:default`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AccountId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    ClaudeCli,
    CodexCli,
    AgyCli,
    /// `kiro-cli`; account ids `kiro:<email>`.
    KiroCli,
    /// `opencode`; account ids `opencode:<provider>` (`opencode:opencode` is the free tier).
    OpencodeCli,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 5] = [
        Self::ClaudeCli,
        Self::CodexCli,
        Self::AgyCli,
        Self::KiroCli,
        Self::OpencodeCli,
    ];
}

/// One model an account can run (kiro and opencode expose several per account).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    /// Costs nothing (opencode `*-free` models).
    pub free: bool,
    /// Kiro: credits charged per unit of work, relative to `auto` = 1.0.
    pub credit_multiplier: Option<f64>,
}

/// Identity read from a CLI's on-disk login; no keychain access, no network.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredAccount {
    pub id: AccountId,
    pub provider: ProviderKind,
    pub email: Option<String>,
    pub account_uuid: Option<Uuid>,
    pub config_dir: Option<PathBuf>,
    pub rate_limit_tier: Option<String>,
    pub auth_expired: bool,
    /// Usable models, empty for CLIs that pick their own.
    pub models: Vec<ModelInfo>,
}
