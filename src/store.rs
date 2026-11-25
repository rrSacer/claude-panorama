use crate::model::{
    parse_and_validate_json, HardwareConfig, HistoryPoint, PanoramaAgentConfig, ProjectDocument, ProjectStatus,
    ValidationError,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::Local;
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

pub const DEFAULT_DATA_ROOT: &str = "/home/sacer/.local/share/claude-panorama";
pub const DEFAULT_PROJECTS_DIR: &str = "/home/sacer/.local/share/claude-panorama/projects";
const MAX_HISTORY_POINTS: usize = 96;

static TMP_SEQ: AtomicU64 = AtomicU64::new(1);

pub fn resolve_projects_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("CLAUDE_PANORAMA_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(DEFAULT_PROJECTS_DIR)
}

pub fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

pub fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.is_empty() || slug.len() > 96 {
        return Err(format!(
            "Slug inválido `{slug}`: deve conter de 1 a 96 caracteres `[a-zA-Z0-9_-]`."
        ));
    }
    if slug.starts_with('-') || slug.starts_with('_') {
        return Err(format!(
            "Slug inválido `{slug}`: deve iniciar com letra ou dígito."
        ));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "Slug inválido `{slug}`: use apenas letras ASCII, dígitos, `-` ou `_`."
        ));
    }
    Ok(())
}

pub fn ensure_projects_dir(dir: &Path) -> io::Result<()> {
    if let Some(parent) = dir.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o777));
        } else if parent == Path::new(DEFAULT_DATA_ROOT) {
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o777));
        }
    }
    fs::create_dir_all(dir)?;
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o777));
    Ok(())
}

pub fn project_file_path(dir: &Path, slug: &str) -> PathBuf {
    dir.join(format!("{slug}.json"))
}

pub fn atomic_write_project(
    dir: &Path,
    slug: &str,
    doc: &ProjectDocument,
) -> Result<PathBuf, String> {
    validate_slug(slug)?;
    ensure_projects_dir(dir)
        .map_err(|e| format!("Falha ao preparar diretório `{}`: {e}", dir.display()))?;

    let mut payload = serde_json::to_string_pretty(doc)
        .map_err(|e| format!("Falha ao serializar JSON de `{slug}`: {e}"))?;
    payload.push('\n');

    let pid = std::process::id();
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_path = dir.join(format!(".{slug}.json.tmp.{pid}.{seq}"));
    let final_path = project_file_path(dir, slug);

    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|e| format!("Falha ao criar temporário `{}`: {e}", tmp_path.display()))?;
        if let Err(e) = file.write_all(payload.as_bytes()) {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!(
                "Falha ao gravar temporário `{}`: {e}",
                tmp_path.display()
            ));
        }
        if let Err(e) = file.sync_all() {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!(
                "Falha no fsync de `{}`: {e}",
                tmp_path.display()
            ));
        }
    }

    let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o666));
    if let Err(e) = fs::rename(&tmp_path, &final_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "Falha ao renomear `{}` -> `{}`: {e}",
            tmp_path.display(),
            final_path.display()
        ));
    }
    let _ = fs::set_permissions(&final_path, fs::Permissions::from_mode(0o666));
    Ok(final_path)
}

pub fn read_project_file(path: &Path) -> Result<ProjectDocument, ValidationError> {
    let raw = fs::read_to_string(path).map_err(|e| ValidationError {
        message: format!("Não foi possível ler `{}`: {e}", path.display()),
        line: None,
        column: None,
    })?;
    let mut doc = parse_and_validate_json(&raw)?;
    if doc.slug.trim().is_empty() {
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            doc.slug = stem.to_string();
        }
    }
    Ok(doc)
}

pub const STATE_FILE_NAME: &str = ".panorama-state.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PanoramaState {
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub default_harness: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub default_reasoning_effort: Option<String>,
}

pub fn state_file_path(dir: &Path) -> PathBuf {
    dir.join(STATE_FILE_NAME)
}

pub fn load_state(dir: &Path) -> PanoramaState {
    let p = state_file_path(dir);
    let Ok(raw) = fs::read_to_string(p) else {
        return PanoramaState::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_state(dir: &Path, state: &PanoramaState) -> Result<(), String> {
    ensure_projects_dir(dir)
        .map_err(|e| format!("Falha ao preparar diretório `{}`: {e}", dir.display()))?;
    let mut payload = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Falha ao serializar estado: {e}"))?;
    payload.push('\n');

    let pid = std::process::id();
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_path = dir.join(format!(".panorama-state.json.tmp.{pid}.{seq}"));
    let final_path = state_file_path(dir);

    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|e| format!("Falha ao criar temporário `{}`: {e}", tmp_path.display()))?;
        if let Err(e) = file.write_all(payload.as_bytes()) {
            let _ = fs::remove_file(&tmp_path);
            return Err(format!("Falha ao gravar estado: {e}"));
        }
        let _ = file.sync_all();
    }
    let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o666));
    if let Err(e) = fs::rename(&tmp_path, &final_path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Falha ao salvar estado: {e}"));
    }
    let _ = fs::set_permissions(&final_path, fs::Permissions::from_mode(0o666));
    Ok(())
}

pub struct InitOptions {
    pub slug: String,
    pub title: String,
    pub path: String,
    pub progresso: f64,
    pub confianca: f64,
    pub phase: String,
    pub agent: Option<PanoramaAgentConfig>,
    pub hardware: Option<HardwareConfig>,
}

pub fn init_project(dir: &Path, opts: InitOptions) -> Result<PathBuf, String> {
    validate_slug(&opts.slug)?;
    let ts = now_iso();
    let phase = if opts.phase.trim().is_empty() {
        "Inicialização".to_string()
    } else {
        opts.phase
    };
    let doc = ProjectDocument {
        slug: opts.slug.clone(),
        title: opts.title,
        subtitle: String::new(),
        path: opts.path,
        icon: crate::model::default_icon(),
        status: ProjectStatus::Ativo,
        phase: phase.clone(),
        updated_at: ts.clone(),
        progresso: opts.progresso,
        progresso_delta: Some(0.0),
        progresso_calculo: "Registro inicial criado via `panorama init`.".to_string(),
        progresso_subtitulo: String::new(),
        confianca: opts.confianca,
        confianca_delta: Some(0.0),
        confianca_resumo: "Estimativa inicial aguardando primeiras provas técnicas.".to_string(),
        confianca_fatores: vec!["Projeto registrado no Claude Panorama.".to_string()],
        confianca_riscos: vec![
            "Aguardando detalhamento da arquitetura e validação automatizada.".to_string(),
        ],
        history: vec![HistoryPoint {
            timestamp: ts,
            progresso: opts.progresso,
            confianca: opts.confianca,
            note: phase,
        }],
        archived: false,
        agent: opts.agent,
        hardware: opts.hardware,
        ui: Vec::new(),
    };
    doc.validate().map_err(|e| e.to_string())?;
    atomic_write_project(dir, &opts.slug, &doc)
}

pub fn apply_project_json(
    dir: &Path,
    slug: &str,
    raw_json: &str,
) -> Result<(PathBuf, ProjectDocument), String> {
    validate_slug(slug)?;
    let raw_val: serde_json::Value =
        serde_json::from_str(raw_json).map_err(|e| format!("JSON inválido: {e}"))?;
    let mut doc = parse_and_validate_json(raw_json).map_err(|e| e.to_string())?;
    doc.slug = slug.to_string();

    let ts = now_iso();
    doc.updated_at = ts.clone();

    let existing_path = project_file_path(dir, slug);
    let existing_doc = if existing_path.exists() {
        read_project_file(&existing_path).ok()
    } else {
        None
    };

    if let Some(prev) = &existing_doc {
        if raw_val.get("archived").is_none() {
            doc.archived = prev.archived;
        }
        if raw_val.get("agent").is_none() {
            doc.agent = prev.agent.clone();
        } else if let (Some(new_ag), Some(prev_ag)) = (doc.agent.as_mut(), prev.agent.as_ref()) {
            if new_ag.agent_id.is_none() {
                new_ag.agent_id = prev_ag.agent_id.clone();
            }
            if new_ag.epic_id.is_none() {
                new_ag.epic_id = prev_ag.epic_id.clone();
            }
            if new_ag.harness_id.trim().is_empty() {
                new_ag.harness_id = prev_ag.harness_id.clone();
            }
            if new_ag.model.trim().is_empty() {
                new_ag.model = prev_ag.model.clone();
            }
            if new_ag.reasoning_effort.is_none() {
                new_ag.reasoning_effort = prev_ag.reasoning_effort.clone();
            }
            if new_ag.last_requested_at.is_none() {
                new_ag.last_requested_at = prev_ag.last_requested_at.clone();
            }
        }
        match (doc.hardware.as_mut(), prev.hardware.as_ref()) {
            (None, Some(prev_hw)) => doc.hardware = Some(prev_hw.clone()),
            (Some(new_hw), Some(prev_hw)) => {
                if new_hw.cpu_cores.is_none() {
                    new_hw.cpu_cores = prev_hw.cpu_cores;
                }
                if new_hw.ssd_percent.is_none() {
                    new_hw.ssd_percent = prev_hw.ssd_percent;
                }
                if new_hw.max_ram_gb.is_none() {
                    new_hw.max_ram_gb = prev_hw.max_ram_gb;
                }
                if new_hw.heavy_cpu.is_none() {
                    new_hw.heavy_cpu = prev_hw.heavy_cpu;
                }
                if new_hw.heavy_ram.is_none() {
                    new_hw.heavy_ram = prev_hw.heavy_ram;
                }
                if new_hw.heavy_ssd.is_none() {
                    new_hw.heavy_ssd = prev_hw.heavy_ssd;
                }
                if new_hw.reason.trim().is_empty() {
                    new_hw.reason = prev_hw.reason.clone();
                }
            }
            _ => {}
        }
    }

    if doc.history.is_empty() {
        if let Some(prev) = &existing_doc {
            doc.history = prev.history.clone();
        }
    }

    let baseline = doc
        .history
        .last()
        .map(|h| (h.progresso, h.confianca))
        .or_else(|| existing_doc.as_ref().map(|d| (d.progresso, d.confianca)));

    if doc.progresso_delta.is_none() {
        doc.progresso_delta = baseline.map(|(p, _)| round_tenths(doc.progresso - p));
    }
    if doc.confianca_delta.is_none() {
        doc.confianca_delta = baseline.map(|(_, c)| round_tenths(doc.confianca - c));
    }

    doc.history.push(HistoryPoint {
        timestamp: ts,
        progresso: doc.progresso,
        confianca: doc.confianca,
        note: doc.phase.clone(),
    });
    if doc.history.len() > MAX_HISTORY_POINTS {
        let excess = doc.history.len() - MAX_HISTORY_POINTS;
        doc.history.drain(0..excess);
    }

    doc.validate().map_err(|e| e.to_string())?;
    let written = atomic_write_project(dir, slug, &doc)?;
    Ok((written, doc))
}

fn round_tenths(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[derive(Debug, Default)]
pub struct LoadedProjects {
    pub projects: Vec<ProjectDocument>,
    pub errors: Vec<(String, String)>,
}

pub fn load_all_projects(dir: &Path) -> LoadedProjects {
    let mut out = LoadedProjects::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| {
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                return false;
            };
            !name.starts_with('.') && name.ends_with(".json")
        })
        .collect();
    paths.sort();

    for path in paths {
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        match read_project_file(&path) {
            Ok(doc) => out.projects.push(doc),
            Err(err) => out.errors.push((fname, err.to_string())),
        }
    }

    let state = load_state(dir);
    let order_map: HashMap<&str, usize> = state
        .order
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();

    // Ordenar: primeiro pela ordem personalizada salva em .panorama-state.json;
    // projetos novos ainda não ordenados seguem por status e slug.
    out.projects.sort_by(|a, b| {
        let oa = order_map.get(a.slug.as_str()).copied();
        let ob = order_map.get(b.slug.as_str()).copied();
        match (oa, ob) {
            (Some(ia), Some(ib)) => ia.cmp(&ib),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => {
                let rank = |s: ProjectStatus| match s {
                    ProjectStatus::Validando => 0,
                    ProjectStatus::Ativo => 1,
                    ProjectStatus::Bloqueado => 2,
                    ProjectStatus::Pausado => 3,
                    ProjectStatus::Concluido => 4,
                };
                rank(a.status)
                    .cmp(&rank(b.status))
                    .then_with(|| a.slug.cmp(&b.slug))
            }
        }
    });

    out
}

pub fn move_project_in_order(
    dir: &Path,
    slug: &str,
    move_up: bool,
    visible_slugs: Option<&[String]>,
) -> Result<(), String> {
    validate_slug(slug)?;
    let loaded = load_all_projects(dir);
    let mut state = load_state(dir);
    for p in &loaded.projects {
        if !state.order.iter().any(|s| s == &p.slug) {
            state.order.push(p.slug.clone());
        }
    }
    state
        .order
        .retain(|s| loaded.projects.iter().any(|p| &p.slug == s));

    let Some(cur_idx) = state.order.iter().position(|s| s == slug) else {
        return Err(format!("Projeto `{slug}` não encontrado na lista."));
    };

    let neighbor_idx = if let Some(vis) = visible_slugs {
        let Some(vis_pos) = vis.iter().position(|s| s == slug) else {
            return Ok(());
        };
        let target_vis = if move_up {
            vis_pos.checked_sub(1)
        } else if vis_pos + 1 < vis.len() {
            Some(vis_pos + 1)
        } else {
            None
        };
        let Some(tv) = target_vis else {
            return Ok(());
        };
        let neighbor_slug = &vis[tv];
        state.order.iter().position(|s| s == neighbor_slug)
    } else if move_up {
        cur_idx.checked_sub(1)
    } else if cur_idx + 1 < state.order.len() {
        Some(cur_idx + 1)
    } else {
        None
    };

    if let Some(n_idx) = neighbor_idx {
        state.order.swap(cur_idx, n_idx);
        save_state(dir, &state)?;
    }
    Ok(())
}

pub fn set_project_archived(
    dir: &Path,
    slug: &str,
    archived: bool,
) -> Result<ProjectDocument, String> {
    validate_slug(slug)?;
    let path = project_file_path(dir, slug);
    let mut doc = read_project_file(&path).map_err(|e| e.to_string())?;
    doc.slug = slug.to_string();
    doc.archived = archived;
    atomic_write_project(dir, slug, &doc)?;
    Ok(doc)
}

pub fn delete_project(dir: &Path, slug: &str) -> Result<(), String> {
    validate_slug(slug)?;
    let path = project_file_path(dir, slug);
    if !path.exists() {
        return Err(format!("Projeto `{slug}` não existe em `{}`.", dir.display()));
    }
    fs::remove_file(&path)
        .map_err(|e| format!("Falha ao deletar `{}`: {e}", path.display()))?;
    let mut state = load_state(dir);
    state.order.retain(|s| s != slug);
    let _ = save_state(dir, &state);
    Ok(())
}

pub fn update_project_agent(
    dir: &Path,
    slug: &str,
    agent: PanoramaAgentConfig,
) -> Result<ProjectDocument, String> {
    validate_slug(slug)?;
    let path = project_file_path(dir, slug);
    let mut doc = read_project_file(&path).map_err(|e| e.to_string())?;
    doc.slug = slug.to_string();
    doc.agent = Some(agent);
    atomic_write_project(dir, slug, &doc)?;
    Ok(doc)
}

pub fn update_project_hardware(
    dir: &Path,
    slug: &str,
    hardware: HardwareConfig,
) -> Result<ProjectDocument, String> {
    validate_slug(slug)?;
    let path = project_file_path(dir, slug);
    let mut doc = read_project_file(&path).map_err(|e| e.to_string())?;
    doc.slug = slug.to_string();
    doc.hardware = Some(hardware);
    doc.validate().map_err(|e| e.to_string())?;
    atomic_write_project(dir, slug, &doc)?;
    Ok(doc)
}

pub fn compute_dir_fingerprint(dir: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    let Ok(entries) = fs::read_dir(dir) else {
        0u8.hash(&mut hasher);
        return hasher.finish();
    };
    let mut items: Vec<(String, u128, u64)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if name != STATE_FILE_NAME && (name.starts_with('.') || !name.ends_with(".json")) {
                return None;
            }
            let meta = entry.metadata().ok()?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            Some((name, mtime, meta.len()))
        })
        .collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "claude-panorama-test-{}-{}-{}",
            tag,
            std::process::id(),
            TMP_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn init_and_apply_atomic_permissions_and_history() {
        let dir = temp_test_dir("atomic");
        let path = init_project(
            &dir,
            InitOptions {
                slug: "meu-proj".to_string(),
                title: "Meu Projeto".to_string(),
                path: "/home/sacer/Projects/meu-proj".to_string(),
                progresso: 10.0,
                confianca: 60.0,
                phase: "Onda 0".to_string(),
                agent: None,
                hardware: None,
            },
        )
        .expect("init must succeed");

        let meta = fs::metadata(&path).expect("metadata");
        assert_eq!(meta.permissions().mode() & 0o666, 0o666);
        let dir_meta = fs::metadata(&dir).expect("dir metadata");
        assert_eq!(dir_meta.permissions().mode() & 0o777, 0o777);

        let fp1 = compute_dir_fingerprint(&dir);

        let update_json = r#"{
            "title": "Meu Projeto",
            "progresso": 25.5,
            "confianca": 78.0,
            "phase": "Onda 1"
        }"#;
        let (_, updated) = apply_project_json(&dir, "meu-proj", update_json).expect("apply");
        assert_eq!(updated.history.len(), 2);
        assert_eq!(updated.progresso_delta, Some(15.5));
        assert_eq!(updated.confianca_delta, Some(18.0));

        let fp2 = compute_dir_fingerprint(&dir);
        assert_ne!(fp1, fp2);

        let loaded = load_all_projects(&dir);
        assert_eq!(loaded.projects.len(), 1);
        assert!(loaded.errors.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_invalid_slug() {
        assert!(validate_slug("../escape").is_err());
        assert!(validate_slug("").is_err());
        assert!(validate_slug("-bad").is_err());
        assert!(validate_slug("horizon-ntm_01").is_ok());
    }

    #[test]
    fn archive_order_delete_and_apply_preserves_metadata() {
        let dir = temp_test_dir("state-ops");
        for (slug, title) in [("alpha", "Alpha"), ("beta", "Beta"), ("gamma", "Gamma")] {
            init_project(
                &dir,
                InitOptions {
                    slug: slug.into(),
                    title: title.into(),
                    path: String::new(),
                    progresso: 10.0,
                    confianca: 50.0,
                    phase: "Init".into(),
                    agent: None,
                    hardware: None,
                },
            )
            .unwrap();
        }
        update_project_agent(
            &dir,
            "alpha",
            PanoramaAgentConfig {
                agent_id: Some("ag-123".into()),
                epic_id: Some("ep-123".into()),
                harness_id: "antigravity".into(),
                model: "gemini-4-hydrogen".into(),
                reasoning_effort: Some("high".into()),
                last_requested_at: None,
                last_status: None,
            },
        )
        .unwrap();
        update_project_hardware(
            &dir,
            "alpha",
            HardwareConfig {
                heavy_cpu: Some(true),
                cpu_cores: Some(4),
                ssd_percent: Some(60),
                max_ram_gb: Some(10.0),
                ..Default::default()
            },
        )
        .unwrap();
        set_project_archived(&dir, "alpha", true).unwrap();

        // apply sem os campos de metadados deve preservá-los!
        let (_, applied) =
            apply_project_json(&dir, "alpha", r#"{"title":"Alpha","progresso":50,"confianca":80}"#)
                .unwrap();
        assert!(applied.archived);
        assert_eq!(applied.agent.as_ref().unwrap().agent_id.as_deref(), Some("ag-123"));
        assert_eq!(applied.hardware.as_ref().unwrap().cpu_cores, Some(4));

        // Reordenação com setas
        move_project_in_order(&dir, "gamma", true, None).unwrap();
        let loaded = load_all_projects(&dir);
        let slugs: Vec<_> = loaded.projects.iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(slugs, vec!["alpha", "gamma", "beta"]);

        delete_project(&dir, "beta").unwrap();
        let loaded2 = load_all_projects(&dir);
        assert_eq!(loaded2.projects.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }
}
