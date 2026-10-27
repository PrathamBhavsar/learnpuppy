//! Local CLI account discovery and the failover pool built on it.

mod discover;
mod pool;

pub use discover::{discover, DiscoverRoots};
pub use pool::{parse_cooldown, AccountHealth, AccountPool, AccountState, Strategy};

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
}
