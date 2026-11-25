use crate::cgroup::HardwareMonitor;
use crate::hardware::SystemHardwareSpecs;
use crate::ipc::{
    AgentActionResult, DaemonRequest, DaemonResponse, HardwareSnapshot, TraycerCatalog,
    DAEMON_SOCKET_PATH,
};
use crate::traycer_bridge;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

pub fn daemon_available() -> bool {
    UnixStream::connect(DAEMON_SOCKET_PATH).is_ok()
}

pub fn send_request(req: &DaemonRequest, timeout: Duration) -> Result<DaemonResponse, String> {
    let mut stream = UnixStream::connect(DAEMON_SOCKET_PATH).map_err(|e| {
        format!(
            "Daemon `claude-panorama-daemon` indisponível em `{DAEMON_SOCKET_PATH}`: {e}"
        )
    })?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let mut line = serde_json::to_string(req).map_err(|e| e.to_string())?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("Falha ao enviar comando ao daemon: {e}"))?;
    let _ = stream.flush();

    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    reader
        .read_line(&mut resp_line)
        .map_err(|e| format!("Falha ao ler resposta do daemon: {e}"))?;
    if resp_line.trim().is_empty() {
        return Err("Daemon encerrou conexão sem resposta.".to_string());
    }
    serde_json::from_str::<DaemonResponse>(&resp_line)
        .map_err(|e| format!("Resposta inválida do daemon: {e}"))
}

fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn list_traycer_models(refresh: bool) -> Result<TraycerCatalog, String> {
    match send_request(
        &DaemonRequest::ListTraycerModels { refresh },
        Duration::from_secs(25),
    ) {
        Ok(DaemonResponse::Models { catalog }) => Ok(catalog),
        Ok(DaemonResponse::Error { message }) => Err(message),
        Ok(_) => Err("Resposta inesperada ao listar modelos do Traycer.".to_string()),
        Err(_err) if is_root() => traycer_bridge::list_traycer_catalog(),
        Err(err) => Err(err),
    }
}

pub fn start_panorama_agent(
    projects_dir: &Path,
    slug: &str,
    title: &str,
    path: &str,
    harness_id: &str,
    model: &str,
    reasoning_effort: Option<&str>,
) -> Result<AgentActionResult, String> {
    let pdir = projects_dir.to_string_lossy().to_string();
    match send_request(
        &DaemonRequest::StartPanoramaAgent {
            projects_dir: pdir,
            slug: slug.to_string(),
            title: title.to_string(),
            path: path.to_string(),
            harness_id: harness_id.to_string(),
            model: model.to_string(),
            reasoning_effort: reasoning_effort.map(|s| s.to_string()),
        },
        Duration::from_secs(35),
    ) {
        Ok(DaemonResponse::AgentAction { result }) => Ok(result),
        Ok(DaemonResponse::Error { message }) => Err(message),
        Ok(_) => Err("Resposta inesperada ao iniciar agente dedicado.".to_string()),
        Err(_err) if is_root() => traycer_bridge::start_panorama_agent(
            projects_dir,
            slug,
            title,
            path,
            harness_id,
            model,
            reasoning_effort,
        ),
        Err(err) => Err(err),
    }
}

pub fn configure_panorama_agent(
    projects_dir: &Path,
    slug: &str,
    agent_id: &str,
    epic_id: Option<&str>,
    harness_id: &str,
    model: &str,
    reasoning_effort: Option<&str>,
) -> Result<AgentActionResult, String> {
    let pdir = projects_dir.to_string_lossy().to_string();
    match send_request(
        &DaemonRequest::ConfigurePanoramaAgent {
            projects_dir: pdir,
            slug: slug.to_string(),
            agent_id: agent_id.to_string(),
            epic_id: epic_id.map(|s| s.to_string()),
            harness_id: harness_id.to_string(),
            model: model.to_string(),
            reasoning_effort: reasoning_effort.map(|s| s.to_string()),
        },
        Duration::from_secs(20),
    ) {
        Ok(DaemonResponse::AgentAction { result }) => Ok(result),
        Ok(DaemonResponse::Error { message }) => Err(message),
        Ok(_) => Err("Resposta inesperada ao configurar modelo do agente.".to_string()),
        Err(_err) if is_root() => traycer_bridge::configure_panorama_agent(
            projects_dir,
            slug,
            agent_id,
            epic_id,
            harness_id,
            model,
            reasoning_effort,
        ),
        Err(err) => Err(err),
    }
}

pub fn request_panorama_update(
    projects_dir: &Path,
    slug: &str,
    title: &str,
    path: &str,
    agent_id: Option<&str>,
    epic_id: Option<&str>,
    harness_id: Option<&str>,
    model: Option<&str>,
    reasoning_effort: Option<&str>,
) -> Result<AgentActionResult, String> {
    let pdir = projects_dir.to_string_lossy().to_string();
    match send_request(
        &DaemonRequest::RequestPanoramaUpdate {
            projects_dir: pdir,
            slug: slug.to_string(),
            title: title.to_string(),
            path: path.to_string(),
            agent_id: agent_id.map(|s| s.to_string()),
            epic_id: epic_id.map(|s| s.to_string()),
            harness_id: harness_id.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            reasoning_effort: reasoning_effort.map(|s| s.to_string()),
        },
        Duration::from_secs(35),
    ) {
        Ok(DaemonResponse::AgentAction { result }) => Ok(result),
        Ok(DaemonResponse::Error { message }) => Err(message),
        Ok(_) => Err("Resposta inesperada ao solicitar atualização ao agente.".to_string()),
        Err(_err) if is_root() => traycer_bridge::request_panorama_update(
            projects_dir,
            slug,
            title,
            path,
            agent_id,
            epic_id,
            harness_id,
            model,
            reasoning_effort,
        ),
        Err(err) => Err(err),
    }
}

pub fn archive_traycer_agent(
    agent_id: &str,
    epic_id: Option<&str>,
    archived: bool,
) -> Result<(), String> {
    match send_request(
        &DaemonRequest::ArchiveTraycerAgent {
            agent_id: agent_id.to_string(),
            epic_id: epic_id.map(|s| s.to_string()),
            archived,
        },
        Duration::from_secs(10),
    ) {
        Ok(DaemonResponse::Ok) => Ok(()),
        Ok(DaemonResponse::Error { message }) => Err(message),
        Ok(_) => Ok(()),
        Err(_err) if is_root() => traycer_bridge::archive_traycer_agent(agent_id, epic_id, archived),
        Err(err) => Err(err),
    }
}

pub fn apply_hardware_limits(
    slug: &str,
    path: &str,
    cpu_cores: Option<u32>,
    cpu_threads: Option<Vec<u32>>,
    ssd_percent: Option<u32>,
    max_ram_gb: Option<f64>,
) -> Result<(), String> {
    match send_request(
        &DaemonRequest::ApplyHardwareLimits {
            slug: slug.to_string(),
            path: path.to_string(),
            cpu_cores,
            cpu_threads: cpu_threads.clone(),
            ssd_percent,
            max_ram_gb,
        },
        Duration::from_secs(5),
    ) {
        Ok(DaemonResponse::Ok) => Ok(()),
        Ok(DaemonResponse::Error { message }) => Err(message),
        Ok(_) => Err("Resposta inesperada ao aplicar limites de hardware.".to_string()),
        Err(_err) if is_root() => {
            let mut mon = HardwareMonitor::new();
            mon.set_limits(slug, path, cpu_cores, cpu_threads, ssd_percent, max_ram_gb)
        }
        Err(err) => Err(err),
    }
}

pub fn remove_hardware_limits(slug: &str) -> Result<(), String> {
    match send_request(
        &DaemonRequest::RemoveHardwareLimits {
            slug: slug.to_string(),
        },
        Duration::from_secs(5),
    ) {
        Ok(DaemonResponse::Ok) => Ok(()),
        Ok(DaemonResponse::Error { message }) => Err(message),
        Ok(_) => Ok(()),
        Err(_err) if is_root() => {
            let mut mon = HardwareMonitor::new();
            mon.remove_limits(slug);
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub fn get_hardware_snapshot(projects: Vec<(String, String)>) -> HardwareSnapshot {
    match send_request(
        &DaemonRequest::GetHardwareSnapshot { projects },
        Duration::from_secs(2),
    ) {
        Ok(DaemonResponse::HardwareSnapshot { snapshot }) => snapshot,
        _ => HardwareSnapshot {
            specs: SystemHardwareSpecs::detect(),
            stats: Default::default(),
        },
    }
}
