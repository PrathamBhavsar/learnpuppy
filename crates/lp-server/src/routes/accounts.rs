//! CLI accounts and global settings (docs/web/03 §Bootstrap / global). No route here returns a
//! token, a keychain value or a credential path beyond the re-login hint.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use lp_service::accounts as svc;
use lp_service::events::{Event, Scope};
use lp_service::settings::Settings;

use super::common::{blocking, conditional_body, core};
use crate::app::AppState;
use crate::dto::settings::{
    AccountDto, AccountsDto, AddClaudeProfile, CatalogDto, PatchAccount, RescanDto, SettingsDto,
};
use crate::error::{ApiError, ApiJson, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/accounts", get(list))
        .route("/accounts/rescan", post(rescan))
        .route("/accounts/claude", post(add_claude))
        .route("/accounts/{id}", patch(patch_account))
        .route("/settings", get(get_settings).put(put_settings))
        .route("/settings/catalog", get(catalog))
}

fn dtos(st: &AppState) -> Vec<AccountDto> {
    let now = st.workspaces.core().clock.now();
    svc::list(&st.pool)
        .into_iter()
        .map(|a| AccountDto::new(a, now))
        .collect()
}

fn changed(st: &AppState) {
    st.hub.publish(Scope::Global, Event::AccountsChanged);
}

/// The account pool: status, cooldowns and usage. Never secrets.
#[utoipa::path(
    get,
    path = "/accounts",
    tag = "accounts",
    responses(
        (status = 200, body = AccountsDto, headers(("ETag" = String, description = "Changes with the pool"))),
        (status = 304, description = "Unchanged since `If-None-Match`"),
    )
)]
pub async fn list(State(st): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    conditional_body(&headers, &AccountsDto { items: dtos(&st) })
}

/// Re-runs account discovery into the live pool.
#[utoipa::path(
    post,
    path = "/accounts/rescan",
    tag = "accounts",
    responses((status = 200, body = RescanDto), (status = 500, body = ErrorEnvelope))
)]
pub async fn rescan(State(st): State<AppState>) -> Result<Json<RescanDto>, ApiError> {
    let core = core(&st);
    let pool = st.pool.clone();
    let added = blocking(move || {
        let _g = core.settings_lock.lock();
        let settings = Settings::load(core.root())?;
        let roots = svc::roots(&core.home, core.claude_config_dir.clone(), &settings);
        Ok(svc::rescan(&pool, &roots, &settings))
    })
    .await?;
    if !added.is_empty() {
        changed(&st);
    }
    Ok(Json(RescanDto {
        items: dtos(&st),
        added,
    }))
}

/// Adds another Claude profile directory (`CLAUDE_CONFIG_DIR`) after checking its login.
#[utoipa::path(
    post,
    path = "/accounts/claude",
    tag = "accounts",
    request_body = AddClaudeProfile,
    responses(
        (status = 200, body = AccountDto),
        (status = 422, description = "No Claude login in that directory", body = ErrorEnvelope),
    )
)]
pub async fn add_claude(
    State(st): State<AppState>,
    ApiJson(body): ApiJson<AddClaudeProfile>,
) -> Result<Json<AccountDto>, ApiError> {
    let core = core(&st);
    let pool = st.pool.clone();
    let id = blocking(move || {
        let _g = core.settings_lock.lock();
        let mut settings = Settings::load(core.root())?;
        let found = svc::add_claude(
            core.root(),
            &core.home,
            &mut settings,
            std::path::Path::new(body.path.trim()),
        )
        .map_err(|e| ApiError::invalid("path", format!("{e:#}")))?;
        let roots = svc::roots(&core.home, core.claude_config_dir.clone(), &settings);
        svc::rescan(&pool, &roots, &settings);
        Ok(found.id.0)
    })
    .await?;
    changed(&st);
    dtos(&st)
        .into_iter()
        .find(|a| a.id == id)
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("no account {id}")))
}

/// Excludes an account from the pool, or puts it back.
#[utoipa::path(
    patch,
    path = "/accounts/{id}",
    tag = "accounts",
    params(("id" = String, Path, description = "Account id, e.g. `claude:me@example.com`")),
    request_body = PatchAccount,
    responses((status = 200, body = AccountDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn patch_account(
    State(st): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<PatchAccount>,
) -> Result<Json<AccountDto>, ApiError> {
    let core = core(&st);
    let pool = st.pool.clone();
    let id2 = id.clone();
    let known = blocking(move || {
        let _g = core.settings_lock.lock();
        let mut settings = Settings::load(core.root())?;
        Ok(svc::set_disabled(
            &pool,
            core.root(),
            &mut settings,
            &id2,
            body.disabled,
        )?)
    })
    .await?;
    if !known {
        return Err(ApiError::not_found(format!("no account {id}")));
    }
    changed(&st);
    dtos(&st)
        .into_iter()
        .find(|a| a.id == id)
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("no account {id}")))
}

/// Global settings: role profiles, pins, concurrency and UI preferences.
#[utoipa::path(
    get,
    path = "/settings",
    tag = "settings",
    responses((status = 200, body = SettingsDto))
)]
pub async fn get_settings(State(st): State<AppState>) -> Result<Json<SettingsDto>, ApiError> {
    let core = core(&st);
    blocking(move || {
        let _g = core.settings_lock.lock();
        Ok(Json(SettingsDto::from(&Settings::load(core.root())?)))
    })
    .await
}

/// Replaces the editable settings after validating them against the catalog and the pool.
#[utoipa::path(
    put,
    path = "/settings",
    tag = "settings",
    request_body = SettingsDto,
    responses(
        (status = 200, body = SettingsDto),
        (status = 422, description = "Fields outside the catalog", body = ErrorEnvelope),
    )
)]
pub async fn put_settings(
    State(st): State<AppState>,
    ApiJson(body): ApiJson<SettingsDto>,
) -> Result<Json<SettingsDto>, ApiError> {
    let core = core(&st);
    let ids: Vec<String> = svc::list(&st.pool).into_iter().map(|a| a.id).collect();
    blocking(move || {
        let _g = core.settings_lock.lock();
        let current = Settings::load(core.root())?;
        let next = body
            .apply_to(&current, &ids)
            .map_err(ApiError::validation)?;
        next.save(core.root())?;
        Ok(Json(SettingsDto::from(&next)))
    })
    .await
}

/// Providers, model ids, effort levels and the other choices settings forms offer.
#[utoipa::path(
    get,
    path = "/settings/catalog",
    tag = "settings",
    responses((status = 200, body = CatalogDto))
)]
pub async fn catalog(State(st): State<AppState>) -> Result<Json<CatalogDto>, ApiError> {
    let core = core(&st);
    blocking(move || {
        let _g = core.settings_lock.lock();
        Ok(Json(CatalogDto::from_settings(&Settings::load(
            core.root(),
        )?)))
    })
    .await
}
