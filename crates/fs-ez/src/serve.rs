//! Bootstrap `fs-ez serve` : config, store, utilisateur initial, routeur et arrêt gracieux.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use fs_ez_app::App;
use fs_ez_core::datetime::now_local;
use fs_ez_web::backend::AppBackend;
use fs_ez_web::config::WebConfig;
use fs_ez_web::locales::Locales;
use fs_ez_web::router::build_router;
use fs_ez_web::state::AppState;

const DEFAULT_BACKUP_HOUR: i64 = 3;
/// Recalcule l'heure choisie au plus chaque minute : un changement de réglage s'applique sans redémarrage.
const BACKUP_CHECK: Duration = Duration::from_secs(60);

/// `RUST_LOG` minimal sans filtre de cible : info par défaut, stderr pour Docker.
fn init_tracing() {
    let level = match std::env::var("RUST_LOG")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };
    let _ = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(true)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Consomme l'`App` et la base déjà construits par `context::build`, aucune seconde ouverture SQLite.
pub fn serve(
    host: Option<String>,
    port: Option<u16>,
    app: App,
    db_path: &Path,
) -> Result<(), String> {
    init_tracing();
    let config = WebConfig::from_env_with(app.root.path(), db_path)
        .map_err(|error| error.to_string())?
        .with_listen(host, port);

    // `init` crée le layout puis applique les réglages absents, même priorité env > toml > settings.
    app.init(&config.db_path)
        .map_err(|error| format!("init: {error}"))?;
    // Refus de symlink ou d'identité : bloquant, le serveur ne démarre pas sur un état douteux.
    let report = app
        .apply_owner_layout()
        .map_err(|error| format!("owner layout: {error}"))?;
    tracing::info!(
        applied = report.applied,
        skipped = report.skipped,
        failed = report.failed,
        "owner layout"
    );
    match app.purge_upload_temps() {
        Ok(0) => {}
        Ok(purged) => tracing::info!(purged, "stale upload temps removed"),
        Err(error) => tracing::warn!(%error, "upload tmp purge failed"),
    }
    // Après le chown : le serveur tourne en PUID:PGID et ne peut plus changer de propriétaire.
    match app.drop_privileges() {
        Ok(Some(owner)) => {
            tracing::info!(uid = owner.uid, gid = owner.gid, "privileges dropped");
        }
        Ok(None) => {}
        Err(error) => return Err(format!("PUID/PGID: {error}")),
    }

    let username = std::env::var("USERNAME")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let password = std::env::var("PASSWORD")
        .ok()
        .filter(|value| !value.is_empty());
    if create_env_user(&app, username.as_deref(), password.as_deref())
        .map_err(|error| format!("user: {error}"))?
        && let Some(username) = username.as_deref()
    {
        tracing::info!(%username, "initial user created from environment");
    }
    run_backup(&app);

    let locales = Locales::load().map_err(|error| error.to_string())?;
    let backend = Arc::new(AppBackend::new(Arc::new(app.clone())));
    let state = AppState::new(backend, Arc::new(config.clone()), Arc::new(locales));
    let watcher_state = state.clone();
    let router = build_router(state);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("runtime: {error}"))?;
    runtime.block_on(async move {
        let watcher = fs_ez_web::events::spawn_data_watcher(watcher_state);
        let backup_task = spawn_backup(app);
        let address = format!("{}:{}", config.host, config.port);
        let listener = tokio::net::TcpListener::bind(&address)
            .await
            .map_err(|error| format!("bind {address}: {error}"))?;
        let local = listener
            .local_addr()
            .map_err(|error| format!("local addr: {error}"))?;
        tracing::info!(address = %local, "listening on {local}");
        let result = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| format!("serve: {error}"));
        backup_task.abort();
        watcher.abort();
        result
    })
}

/// Boucle quotidienne à l'heure choisie, relue avant chaque attente, les erreurs ne tuent pas le serveur.
fn spawn_backup(app: App) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        loop {
            let reader = app.clone();
            let hour = match tokio::task::spawn_blocking(move || reader.settings()).await {
                Ok(Ok(settings)) => settings.backup_hour,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "backup settings unavailable");
                    DEFAULT_BACKUP_HOUR
                }
                Err(error) => {
                    tracing::warn!(%error, "backup settings task failed");
                    DEFAULT_BACKUP_HOUR
                }
            };
            let wait = duration_until_hour(hour);
            if wait > BACKUP_CHECK {
                tokio::time::sleep(BACKUP_CHECK).await;
                continue;
            }
            tokio::time::sleep(wait).await;
            let worker = app.clone();
            let _ = tokio::task::spawn_blocking(move || run_backup(&worker)).await;
        }
    })
}

/// Secondes restantes avant la prochaine occurrence de l'heure locale, au moins une.
fn duration_until_hour(hour: i64) -> Duration {
    let now = now_local();
    let offset = now.offset().whole_seconds().into();
    let seconds = now.unix_timestamp().max(0).unsigned_abs();
    Duration::from_secs(seconds_until_hour(seconds, hour, offset))
}

fn seconds_until_hour(now: u64, hour: i64, offset_seconds: i64) -> u64 {
    let local = i64::try_from(now)
        .unwrap_or(i64::MAX)
        .saturating_add(offset_seconds)
        .rem_euclid(86_400);
    let target = hour.clamp(0, 23) * 3_600;
    let wait = if target > local {
        target - local
    } else {
        86_400 + target - local
    };
    u64::try_from(wait).unwrap_or(1).max(1)
}

fn run_backup(app: &App) {
    match app.backup() {
        Ok(report) => tracing::info!(
            path = %report.created.display(),
            pruned = report.pruned,
            "database backup created"
        ),
        Err(error) => tracing::warn!(%error, "database backup failed"),
    }
}

/// Premier boot : crée l'utilisateur et retourne `true`, `false` sinon.
/// `PASSWORD` présent = bootstrap non interactif, sinon le web ouvre l'onboarding.
/// Aucun nom par défaut : `USERNAME` est obligatoire avec `PASSWORD`.
fn create_env_user(
    app: &App,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<bool, String> {
    if app.has_user().map_err(|error| error.to_string())? {
        return Ok(false);
    }
    let Some(password) = password else {
        return Ok(false);
    };
    let username = username
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "USERNAME is required when PASSWORD is set".to_string())?;
    app.init_user(username, password)
        .map_err(|error| error.to_string())?;
    Ok(true)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "SIGTERM handler unavailable");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

#[cfg(test)]
mod tests;
