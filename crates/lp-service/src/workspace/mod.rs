//! Multiple isolated workspaces (docs/web/02): metadata, the registry of open handles, trash,
//! export/import, the legacy single-store migration, `state.json` and `server.lock`.
//!
//! Everything here is synchronous filesystem work; `lp-server` calls it from
//! `tokio::task::spawn_blocking` and maps [`WorkspaceError::code`] to its error envelope.

pub mod export;
pub mod lock;
pub mod meta;
pub mod migrate;
pub mod registry;
pub mod state;
pub mod trash;

mod fsx;

#[cfg(test)]
mod tests;

pub use export::{ExportCounts, ExportReport, ImportReport, Manifest, ManifestFile};
pub use lock::{LockStatus, ServerLock, ServerLockInfo};
pub use meta::{
    NewWorkspace, Overrides, Sources, WorkspaceMeta, WorkspacePatch, HUES, ICONS, SCHEMA,
};
pub use migrate::{MarkerState, MigrationMarker, MigrationOutcome, Move, RevertReport};
pub use registry::{
    BusyGuard, DamagedWorkspace, EventSink, NoopSink, RegistryConfig, WorkspaceEvent,
    WorkspaceHandle, WorkspaceList, WorkspaceRegistry, WorkspaceStats, WorkspaceSummary,
};
pub use state::State;
pub use trash::TrashEntry;

use lp_core::ports::StoreError;

/// Every way a workspace operation can fail. [`code`](Self::code) is the stable wire code from
/// docs/web/03 (`not_found`, `validation`, `conflict`, `workspace_archived`, `storage_full`,
/// `internal`).
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace not found: {0}")]
    NotFound(String),
    #[error("invalid {field}: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },
    #[error("a workspace named {0:?} already exists")]
    NameTaken(String),
    #[error("workspace {0} is archived and read-only")]
    Archived(String),
    #[error("confirmation does not match the workspace name")]
    ConfirmMismatch,
    #[error("workspace {id} is damaged: {reason}")]
    Damaged { id: String, reason: String },
    #[error("{0}")]
    Conflict(String),
    #[error("disk full: {0}")]
    StorageFull(String),
    #[error("invalid workspace archive: {0}")]
    InvalidArchive(String),
    #[error("{0}")]
    Internal(String),
}

impl WorkspaceError {
    /// The docs/web/03 error code for this failure.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::Validation { .. } | Self::ConfirmMismatch | Self::InvalidArchive(_) => {
                "validation"
            }
            Self::NameTaken(_) | Self::Conflict(_) => "conflict",
            Self::Archived(_) => "workspace_archived",
            Self::StorageFull(_) => "storage_full",
            Self::Damaged { .. } | Self::Internal(_) => "internal",
        }
    }

    pub(crate) fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            field,
            message: message.into(),
        }
    }
}

/// `ENOSPC` or `EDQUOT`.
fn is_full_errno(code: i32) -> bool {
    const EDQUOT: i32 = if cfg!(target_os = "linux") { 122 } else { 69 };
    code == 28 || code == EDQUOT
}

impl From<std::io::Error> for WorkspaceError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::StorageFull
            || e.kind() == std::io::ErrorKind::QuotaExceeded
            || e.raw_os_error().is_some_and(is_full_errno)
        {
            Self::StorageFull(e.to_string())
        } else {
            Self::Internal(e.to_string())
        }
    }
}

impl From<StoreError> for WorkspaceError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(id) => Self::NotFound(id),
            // lp-store flattens io errors to their Display text; recover the disk-full case.
            StoreError::Backend(m) if is_full_message(&m) => Self::StorageFull(m),
            StoreError::Backend(m) => Self::Internal(m),
        }
    }
}

fn is_full_message(m: &str) -> bool {
    let m = m.to_ascii_lowercase();
    m.contains("no space left")
        || m.contains("os error 28)")
        || m.contains("disk quota exceeded")
        || m.contains("database or disk is full")
}

impl From<anyhow::Error> for WorkspaceError {
    fn from(e: anyhow::Error) -> Self {
        for cause in e.chain() {
            if let Some(io) = cause.downcast_ref::<std::io::Error>() {
                if let Some(code) = io.raw_os_error().filter(|c| is_full_errno(*c)) {
                    return std::io::Error::from_raw_os_error(code).into();
                }
            }
            if let Some(StoreError::Backend(m)) = cause.downcast_ref::<StoreError>() {
                if is_full_message(m) {
                    return Self::StorageFull(m.clone());
                }
            }
        }
        Self::Internal(format!("{e:#}"))
    }
}

impl From<serde_json::Error> for WorkspaceError {
    fn from(e: serde_json::Error) -> Self {
        if let Some(kind) = e.io_error_kind() {
            return std::io::Error::new(kind, e.to_string()).into();
        }
        Self::Internal(e.to_string())
    }
}

pub type Result<T, E = WorkspaceError> = std::result::Result<T, E>;
