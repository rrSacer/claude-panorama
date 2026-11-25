use crate::hardware::{LiveHardwareStats, SystemHardwareSpecs};
use crate::model::PanoramaAgentConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DAEMON_SOCKET_PATH: &str = "/run/claude-panorama/daemon.sock";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraycerModelInfo {
    pub id: String,
    #[serde(default)]
    pub reasoning_efforts: Vec<String>,
    #[serde(default)]
    pub fast_mode_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraycerHarnessInfo {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub models: Vec<TraycerModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraycerCatalog {
    pub harnesses: Vec<TraycerHarnessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionResult {
    pub agent: PanoramaAgentConfig,
    pub message: String,
    pub created_new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSnapshot {
    pub specs: SystemHardwareSpecs,
    pub stats: HashMap<String, LiveHardwareStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DaemonRequest {
    Ping,
    ListTraycerModels {
        #[serde(default)]
        refresh: bool,
    },
    StartPanoramaAgent {
        projects_dir: String,
        slug: String,
        title: String,
        path: String,
        harness_id: String,
        model: String,
        reasoning_effort: Option<String>,
    },
    ConfigurePanoramaAgent {
        projects_dir: String,
        slug: String,
        agent_id: String,
        epic_id: Option<String>,
        harness_id: String,
        model: String,
        reasoning_effort: Option<String>,
    },
    RequestPanoramaUpdate {
        projects_dir: String,
        slug: String,
        title: String,
        path: String,
        agent_id: Option<String>,
        epic_id: Option<String>,
        harness_id: Option<String>,
        model: Option<String>,
        reasoning_effort: Option<String>,
    },
    ArchiveTraycerAgent {
        agent_id: String,
        epic_id: Option<String>,
        archived: bool,
    },
    ApplyHardwareLimits {
        slug: String,
        path: String,
        cpu_cores: Option<u32>,
        ssd_percent: Option<u32>,
        max_ram_gb: Option<f64>,
    },
    RemoveHardwareLimits {
        slug: String,
    },
    GetHardwareSnapshot {
        projects: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DaemonResponse {
    Ok,
    Models { catalog: TraycerCatalog },
    AgentAction { result: AgentActionResult },
    HardwareStats { stats: LiveHardwareStats },
    HardwareSnapshot { snapshot: HardwareSnapshot },
    Error { message: String },
}
