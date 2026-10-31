//! Source settings, discovery and workspace-owned ingest jobs.
use super::common::{blocking, core, open_writable, open_ws};
use crate::{
    app::AppState,
    dto::{mutations::*, workspace::SourcesDto},
    error::{ApiError, ApiJson, ErrorCode, ErrorEnvelope},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use lp_service::{
    events::{Event, Scope},
    ingest_svc as svc,
    settings::Settings,
    workspace::WorkspacePatch,
};
use std::sync::Arc;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/{ws}/ingest/sources",
            get(sources).put(save_sources),
        )
        .route("/workspaces/{ws}/ingest/run", post(run))
        .route("/workspaces/{ws}/ingest/runs", get(runs))
        .route("/workspaces/{ws}/ingest/runs/{id}/cancel", post(cancel))
}
#[utoipa::path(get, path="/workspaces/{ws}/ingest/sources", tag="ingest", params(("ws"=String, Path)), responses((status=200, body=IngestSources),(status=404, body=ErrorEnvelope)))]
pub async fn sources(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<IngestSources>, ApiError> {
    let h = open_ws(&st, &ws).await?;
    let c = core(&st);
    blocking(move || {
        let config = h.meta().sources;
        let settings = Settings::load(c.root())?;
        let roots =
            lp_service::ingest::roots(&c.home, &settings.claude_dirs, c.claude_config_dir.clone());
        let since = c
            .clock
            .now()
            .saturating_sub(i64::from(config.since_days) * 86_400_000);
        let candidates = svc::candidates(&roots, since)?
            .into_iter()
            .map(Into::into)
            .collect();
        let warnings = svc::overlaps(&c.registry, h.id(), &config)?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Json(IngestSources {
            enabled: !config.allow.is_empty(),
            sources: config.into(),
            candidates,
            warnings,
            running_id: c.ingest_runs.current(h.id()),
        }))
    })
    .await
}
#[utoipa::path(put, path="/workspaces/{ws}/ingest/sources", tag="ingest", params(("ws"=String, Path)), request_body=SourcesDto, responses((status=200, body=IngestSources),(status=400, body=ErrorEnvelope)))]
pub async fn save_sources(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(body): ApiJson<SourcesDto>,
) -> Result<Json<IngestSources>, ApiError> {
    if !(1..=3650).contains(&body.since_days) {
        return Err(ApiError::invalid("sinceDays", "must be between 1 and 3650"));
    }
    let h = open_writable(&st, &ws).await?;
    let c = core(&st);
    blocking(move || {
        let m = c.registry.update(
            h.id(),
            WorkspacePatch {
                sources: Some(body.into()),
                ..Default::default()
            },
        )?;
        let warnings = svc::overlaps(&c.registry, h.id(), &m.sources)?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(Json(IngestSources {
            enabled: !m.sources.allow.is_empty(),
            sources: m.sources.into(),
            candidates: vec![],
            warnings,
            running_id: c.ingest_runs.current(h.id()),
        }))
    })
    .await
}
#[utoipa::path(get, path="/workspaces/{ws}/ingest/runs", tag="ingest", params(("ws"=String, Path)), responses((status=200, body=IngestHistory),(status=404, body=ErrorEnvelope)))]
pub async fn runs(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<IngestHistory>, ApiError> {
    let h = open_ws(&st, &ws).await?;
    blocking(move || {
        Ok(Json(IngestHistory {
            items: svc::history(h.root())?
                .into_iter()
                .map(Into::into)
                .collect(),
        }))
    })
    .await
}
#[utoipa::path(post, path="/workspaces/{ws}/ingest/runs/{id}/cancel", tag="ingest", params(("ws"=String, Path),("id"=String, Path)), responses((status=204),(status=404, body=ErrorEnvelope)))]
pub async fn cancel(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    open_writable(&st, &ws).await?;
    if !core(&st).ingest_runs.cancel(&ws, &id) {
        return Err(ApiError::not_found("no active ingest run with this id"));
    }
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post, path="/workspaces/{ws}/ingest/run", tag="ingest", params(("ws"=String, Path)), request_body=StartIngest, responses((status=202, body=IngestStarted),(status=409, body=ErrorEnvelope),(status=400, body=ErrorEnvelope)))]
pub async fn run(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(body): ApiJson<StartIngest>,
) -> Result<(StatusCode, Json<IngestStarted>), ApiError> {
    let h = open_writable(&st, &ws).await?;
    let sources = h.meta().sources;
    if sources.allow.is_empty() {
        return Err(ApiError::invalid(
            "sources.allow",
            "ingest is disabled; allow a project directory first",
        ));
    }
    let days = body.since_days.unwrap_or(sources.since_days);
    if !(1..=3650).contains(&days) {
        return Err(ApiError::invalid("sinceDays", "must be between 1 and 3650"));
    }
    let c = core(&st);
    let reservation = c
        .ingest_runs
        .reserve(&h)
        .ok_or_else(|| ApiError::new(ErrorCode::Conflict, "an ingest run is already active"))?;
    let run_id = reservation.id.clone();
    let started_at = c.clock.now();
    let settings_core = c.clone();
    let settings = blocking(move || Ok(Settings::load(settings_core.root())?)).await?;
    let engine = Arc::new(st.engine_for(&settings, "distiller"));
    let roots = svc::scoped_roots(
        lp_service::ingest::roots(&c.home, &settings.claude_dirs, c.claude_config_dir.clone()),
        &sources,
        h.root(),
    );
    let busy = h.busy();
    let shutdown = st.shutdown.clone();
    let runtime = tokio::runtime::Handle::current();
    // Keep the reservation until the blocking scan exits, even after cancellation.
    std::mem::drop(tokio::task::spawn_blocking(move || {
        let _reservation = reservation;
        let _busy = busy;
        let cancel = _reservation.cancel.clone();
        let watcher_cancel = cancel.clone();
        let watcher = runtime.spawn(async move {
            shutdown.cancelled().await;
            watcher_cancel.cancel();
        });
        let scope = Scope::Workspace(h.id().to_owned());
        let publish = |event| {
            c.hub.publish(scope.clone(), event);
        };
        let mut report = svc::Report::default();
        publish(Event::IngestProgress {
            run_id: _reservation.id.clone(),
            done: 0,
            total: 0,
            phase: "scan".into(),
        });
        let since = started_at.saturating_sub(i64::from(days) * 86_400_000);
        let scan = if cancel.is_cancelled() {
            Err(anyhow::anyhow!("ingest cancelled"))
        } else {
            lp_service::ingest::scan(&*h.stores().tracker, &roots, &sources.allow, since)
        };
        match scan {
            Err(e) => report.errors.push(e.to_string()),
            Ok(scan) => {
                report.sessions = scan.sessions as u64;
                report.skipped = scan.files.saturating_sub(scan.sessions) as u64;
                report.errors = scan.errors.clone();
                let total = scan.stored.len() as u64;
                for (i, session) in scan.stored.into_iter().enumerate() {
                    if cancel.is_cancelled() {
                        break;
                    }
                    publish(Event::IngestProgress {
                        run_id: _reservation.id.clone(),
                        done: i as u64,
                        total,
                        phase: "distill".into(),
                    });
                    let one = lp_ingest::IngestReport {
                        stored: vec![session],
                        ..Default::default()
                    };
                    let graph_lock = c.graph_mutations.workspace_lock(h.root());
                    let _graph_lock = graph_lock.lock();
                    let distilled = runtime.block_on(lp_service::ingest::distill(
                        &*h.stores().tracker,
                        h.stores().graph.clone(),
                        engine.clone(),
                        if sources.git { &sources.allow } else { &[] },
                        &one,
                        started_at,
                        cancel.clone(),
                    ));
                    report.new_decisions += distilled.decisions as u64;
                    report.new_cards += distilled.cards as u64;
                    report.errors.extend(distilled.errors);
                    report.errors.extend(distilled.warnings);
                }
                if cancel.is_cancelled() {
                    report.errors.push("ingest cancelled".into());
                }
            }
        }
        let run = svc::Run {
            id: _reservation.id.clone(),
            started_at,
            finished_at: Some(c.clock.now()),
            report: report.clone(),
        };
        if let Err(e) = svc::finish(h.root(), &run) {
            report
                .errors
                .push(format!("could not persist run history: {e}"));
        }
        c.registry.invalidate_stats(h.id());
        c.graphs.forget(&h.root().join("graph"));
        if let Ok(snapshot) = lp_service::graph::snapshot(&*h.stores().graph) {
            c.start_layout(&h, Arc::new(lp_service::graph::Loaded::new(snapshot)));
        }
        let run_id = _reservation.id.clone();
        drop(_reservation);
        publish(Event::IngestProgress {
            run_id: run_id.clone(),
            done: report.sessions,
            total: report.sessions,
            phase: if cancel.is_cancelled() {
                "cancelled"
            } else {
                "done"
            }
            .into(),
        });
        publish(Event::IngestDone {
            run_id,
            report: (&report).into(),
        });
        watcher.abort();
    }));
    Ok((StatusCode::ACCEPTED, Json(IngestStarted { run_id })))
}
