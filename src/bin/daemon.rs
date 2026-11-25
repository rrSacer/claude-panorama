use claude_panorama::cgroup::HardwareMonitor;
use claude_panorama::ipc::{
    DaemonRequest, DaemonResponse, HardwareSnapshot, TraycerCatalog, DAEMON_SOCKET_PATH,
};
use claude_panorama::store::load_all_projects;
use claude_panorama::traycer_bridge;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

struct DaemonCtx {
    hw: Mutex<HardwareMonitor>,
    catalog: Mutex<Option<(TraycerCatalog, Instant)>>,
    projects_dir: Mutex<PathBuf>,
}

fn default_projects_dir() -> PathBuf {
    if let Ok(v) = std::env::var("CLAUDE_PANORAMA_DIR") {
        if !v.trim().is_empty() {
            return PathBuf::from(v);
        }
    }
    PathBuf::from("/home/sacer/.local/share/claude-panorama/projects")
}

fn main() {
    let sock_path = Path::new(DAEMON_SOCKET_PATH);
    if let Some(parent) = sock_path.parent() {
        let _ = fs::create_dir_all(parent);
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o755));
    }
    if sock_path.exists() {
        let _ = fs::remove_file(sock_path);
    }

    let listener = match UnixListener::bind(sock_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("erro fatal ao abrir socket {}: {e}", sock_path.display());
            std::process::exit(1);
        }
    };
    let _ = fs::set_permissions(sock_path, fs::Permissions::from_mode(0o666));

    let ctx = Arc::new(DaemonCtx {
        hw: Mutex::new(HardwareMonitor::new()),
        catalog: Mutex::new(None),
        projects_dir: Mutex::new(default_projects_dir()),
    });

    // Thread de fundo: aplica limites salvos e coleta telemetria a cada 1 segundo
    let bg_ctx = Arc::clone(&ctx);
    thread::spawn(move || loop {
        let pdir = bg_ctx
            .projects_dir
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| default_projects_dir());
        let loaded = load_all_projects(&pdir);
        let mut pairs: Vec<(String, String)> = Vec::new();
        if let Ok(mut hw) = bg_ctx.hw.lock() {
            let mut active_slugs = std::collections::HashSet::new();
            for proj in &loaded.projects {
                if proj.archived {
                    if hw.tracked.contains_key(&proj.slug) {
                        hw.remove_limits(&proj.slug);
                    }
                    continue;
                }
                active_slugs.insert(proj.slug.clone());
                pairs.push((proj.slug.clone(), proj.path.clone()));

                let desired = proj.hardware.as_ref();
                let cpu_cores = desired.and_then(|h| h.cpu_cores);
                let ssd_percent = desired.and_then(|h| h.ssd_percent);
                let max_ram_gb = desired.and_then(|h| h.max_ram_gb);
                let has_desired = cpu_cores.is_some() || ssd_percent.is_some() || max_ram_gb.is_some();

                let needs_sync = match hw.tracked.get(&proj.slug) {
                    Some(cur) => {
                        cur.path != proj.path
                            || cur.cpu_cores != cpu_cores
                            || cur.ssd_percent != ssd_percent
                            || cur.max_ram_gb != max_ram_gb
                    }
                    None => has_desired,
                };

                if needs_sync {
                    if has_desired {
                        let _ = hw.set_limits(
                            &proj.slug,
                            &proj.path,
                            cpu_cores,
                            ssd_percent,
                            max_ram_gb,
                        );
                    } else {
                        hw.remove_limits(&proj.slug);
                    }
                }
            }
            let stale: Vec<String> = hw
                .tracked
                .keys()
                .filter(|k| !active_slugs.contains(*k))
                .cloned()
                .collect();
            for s in stale {
                hw.remove_limits(&s);
            }

            hw.sample_projects(&pairs);
        }
        thread::sleep(Duration::from_secs(1));
    });

    eprintln!(
        "claude-panorama-daemon ativo em {}",
        sock_path.display()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(st) => {
                let c = Arc::clone(&ctx);
                thread::spawn(move || handle_client(st, c));
            }
            Err(e) => {
                eprintln!("erro accept: {e}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn handle_client(stream: UnixStream, ctx: Arc<DaemonCtx>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));

    let mut reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };
    let mut writer = stream;

    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }

    let resp = match serde_json::from_str::<DaemonRequest>(&line) {
        Ok(req) => dispatch_request(req, &ctx),
        Err(e) => DaemonResponse::Error {
            message: format!("Requisição JSON inválida: {e}"),
        },
    };

    if let Ok(mut out) = serde_json::to_string(&resp) {
        out.push('\n');
        let _ = writer.write_all(out.as_bytes());
        let _ = writer.flush();
    }
}

fn dispatch_request(req: DaemonRequest, ctx: &DaemonCtx) -> DaemonResponse {
    match req {
        DaemonRequest::Ping => DaemonResponse::Ok,
        DaemonRequest::ListTraycerModels { refresh } => {
            if !refresh {
                if let Ok(guard) = ctx.catalog.lock() {
                    if let Some((cat, fetched_at)) = guard.as_ref() {
                        if fetched_at.elapsed() < Duration::from_secs(120) && !cat.harnesses.is_empty()
                        {
                            return DaemonResponse::Models {
                                catalog: cat.clone(),
                            };
                        }
                    }
                }
            }
            match traycer_bridge::list_traycer_catalog() {
                Ok(catalog) => {
                    if let Ok(mut guard) = ctx.catalog.lock() {
                        *guard = Some((catalog.clone(), Instant::now()));
                    }
                    DaemonResponse::Models { catalog }
                }
                Err(message) => DaemonResponse::Error { message },
            }
        }
        DaemonRequest::StartPanoramaAgent {
            projects_dir,
            slug,
            title,
            path,
            harness_id,
            model,
            reasoning_effort,
        } => {
            if let Ok(mut p) = ctx.projects_dir.lock() {
                *p = PathBuf::from(&projects_dir);
            }
            match traycer_bridge::start_panorama_agent(
                Path::new(&projects_dir),
                &slug,
                &title,
                &path,
                &harness_id,
                &model,
                reasoning_effort.as_deref(),
            ) {
                Ok(result) => DaemonResponse::AgentAction { result },
                Err(message) => DaemonResponse::Error { message },
            }
        }
        DaemonRequest::ConfigurePanoramaAgent {
            projects_dir,
            slug,
            agent_id,
            epic_id,
            harness_id,
            model,
            reasoning_effort,
        } => {
            if let Ok(mut p) = ctx.projects_dir.lock() {
                *p = PathBuf::from(&projects_dir);
            }
            match traycer_bridge::configure_panorama_agent(
                Path::new(&projects_dir),
                &slug,
                &agent_id,
                epic_id.as_deref(),
                &harness_id,
                &model,
                reasoning_effort.as_deref(),
            ) {
                Ok(result) => DaemonResponse::AgentAction { result },
                Err(message) => DaemonResponse::Error { message },
            }
        }
        DaemonRequest::RequestPanoramaUpdate {
            projects_dir,
            slug,
            title,
            path,
            agent_id,
            epic_id,
            harness_id,
            model,
            reasoning_effort,
        } => {
            if let Ok(mut p) = ctx.projects_dir.lock() {
                *p = PathBuf::from(&projects_dir);
            }
            match traycer_bridge::request_panorama_update(
                Path::new(&projects_dir),
                &slug,
                &title,
                &path,
                agent_id.as_deref(),
                epic_id.as_deref(),
                harness_id.as_deref(),
                model.as_deref(),
                reasoning_effort.as_deref(),
            ) {
                Ok(result) => DaemonResponse::AgentAction { result },
                Err(message) => DaemonResponse::Error { message },
            }
        }
        DaemonRequest::ArchiveTraycerAgent {
            agent_id,
            epic_id,
            archived,
        } => match traycer_bridge::archive_traycer_agent(&agent_id, epic_id.as_deref(), archived) {
            Ok(()) => DaemonResponse::Ok,
            Err(message) => DaemonResponse::Error { message },
        },
        DaemonRequest::ApplyHardwareLimits {
            slug,
            path,
            cpu_cores,
            ssd_percent,
            max_ram_gb,
        } => match ctx.hw.lock() {
            Ok(mut hw) => match hw.set_limits(&slug, &path, cpu_cores, ssd_percent, max_ram_gb) {
                Ok(()) => DaemonResponse::Ok,
                Err(message) => DaemonResponse::Error { message },
            },
            Err(_) => DaemonResponse::Error {
                message: "Lock de hardware indisponível".to_string(),
            },
        },
        DaemonRequest::RemoveHardwareLimits { slug } => match ctx.hw.lock() {
            Ok(mut hw) => {
                hw.remove_limits(&slug);
                DaemonResponse::Ok
            }
            Err(_) => DaemonResponse::Error {
                message: "Lock de hardware indisponível".to_string(),
            },
        },
        DaemonRequest::GetHardwareSnapshot { projects } => match ctx.hw.lock() {
            Ok(mut hw) => {
                let mut stats = hw.latest_stats.clone();
                let missing = projects
                    .iter()
                    .any(|(slug, _)| !stats.contains_key(slug));
                if missing && !projects.is_empty() {
                    stats = hw.sample_projects(&projects);
                }
                DaemonResponse::HardwareSnapshot {
                    snapshot: HardwareSnapshot {
                        specs: hw.specs.clone(),
                        stats,
                    },
                }
            }
            Err(_) => DaemonResponse::Error {
                message: "Lock de hardware indisponível".to_string(),
            },
        },
    }
}
