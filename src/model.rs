use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

pub fn default_icon() -> String {
    "applications-engineering-symbolic".to_string()
}

pub fn default_confianca() -> f64 {
    50.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    #[default]
    #[serde(alias = "active")]
    Ativo,
    #[serde(alias = "validating")]
    Validando,
    #[serde(alias = "paused")]
    Pausado,
    #[serde(alias = "done", alias = "completed")]
    Concluido,
    #[serde(alias = "blocked")]
    Bloqueado,
}

impl ProjectStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ativo => "ativo",
            Self::Validando => "validando",
            Self::Pausado => "pausado",
            Self::Concluido => "concluido",
            Self::Bloqueado => "bloqueado",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            Self::Ativo => "Ativo",
            Self::Validando => "Validando",
            Self::Pausado => "Pausado",
            Self::Concluido => "Concluído",
            Self::Bloqueado => "Bloqueado",
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            Self::Ativo => "badge-ativo",
            Self::Validando => "badge-validando",
            Self::Pausado => "badge-pausado",
            Self::Concluido => "badge-concluido",
            Self::Bloqueado => "badge-bloqueado",
        }
    }
}

impl fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Neutral,
    Info,
    #[serde(alias = "ok")]
    Success,
    #[serde(alias = "warn")]
    Warning,
    #[serde(alias = "error", alias = "err")]
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AlertLevel {
    #[default]
    Info,
    #[serde(alias = "ok")]
    Success,
    #[serde(alias = "warn")]
    Warning,
    #[serde(alias = "error", alias = "err")]
    Danger,
}

impl AlertLevel {
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Info => "dialog-information-symbolic",
            Self::Success => "emblem-ok-symbolic",
            Self::Warning => "dialog-warning-symbolic",
            Self::Danger => "dialog-error-symbolic",
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            Self::Info => "alert-info",
            Self::Success => "alert-success",
            Self::Warning => "alert-warning",
            Self::Danger => "alert-danger",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KpiStatus {
    #[serde(alias = "pass", alias = "success")]
    Ok,
    #[serde(alias = "warning")]
    Warn,
    #[serde(alias = "fail", alias = "danger", alias = "error")]
    Err,
    #[default]
    #[serde(alias = "info")]
    Neutral,
}

impl KpiStatus {
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Ok => "status-ok",
            Self::Warn => "status-warn",
            Self::Err => "status-err",
            Self::Neutral => "status-neutral",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CellState {
    #[serde(alias = "ok", alias = "done")]
    Pass,
    #[serde(alias = "active", alias = "in_progress")]
    Running,
    #[serde(alias = "err", alias = "error", alias = "failed")]
    Fail,
    #[default]
    #[serde(alias = "todo")]
    Pending,
    Blocked,
    Reserved,
}

impl CellState {
    pub fn label_pt(self) -> &'static str {
        match self {
            Self::Pass => "Aprovado / Pronto",
            Self::Running => "Em execução",
            Self::Fail => "Falha / Conflito",
            Self::Pending => "Pendente",
            Self::Blocked => "Bloqueado",
            Self::Reserved => "Reservado",
        }
    }

    pub fn short_tag(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Running => "RUN",
            Self::Fail => "FAIL",
            Self::Pending => "PEND",
            Self::Blocked => "BLOC",
            Self::Reserved => "RESV",
        }
    }

    pub fn rgb(self, dark: bool) -> (f64, f64, f64) {
        match (self, dark) {
            (Self::Pass, true) => (0.180, 0.761, 0.494),
            (Self::Pass, false) => (0.149, 0.635, 0.412),
            (Self::Running, true) => (0.384, 0.627, 0.918),
            (Self::Running, false) => (0.110, 0.443, 0.847),
            (Self::Fail, true) => (0xf6 as f64 / 255.0, 0x61 as f64 / 255.0, 0x51 as f64 / 255.0),
            (Self::Fail, false) => (0.878, 0.106, 0.141),
            (Self::Pending, true) => (0.545, 0.545, 0.580),
            (Self::Pending, false) => (0.467, 0.463, 0.482),
            (Self::Blocked, true) => (0.898, 0.647, 0.039),
            (Self::Blocked, false) => (0.776, 0.471, 0.000),
            (Self::Reserved, true) => (0.753, 0.380, 0.796),
            (Self::Reserved, false) => (0.569, 0.255, 0.675),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HistoryPoint {
    pub timestamp: String,
    pub progresso: f64,
    pub confianca: f64,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectDocument {
    #[serde(default)]
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub path: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default)]
    pub status: ProjectStatus,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub updated_at: String,

    /// Elemento fixo 1: porcentagem real implementada e verificada (0.0 a 100.0)
    #[serde(default)]
    pub progresso: f64,
    #[serde(default)]
    pub progresso_delta: Option<f64>,
    #[serde(default)]
    pub progresso_calculo: String,
    #[serde(default)]
    pub progresso_subtitulo: String,

    /// Elemento fixo 2: estimativa dinâmica de sucesso do projeto (0.0 a 100.0)
    #[serde(default = "default_confianca")]
    pub confianca: f64,
    #[serde(default)]
    pub confianca_delta: Option<f64>,
    #[serde(default)]
    pub confianca_resumo: String,
    #[serde(default)]
    pub confianca_fatores: Vec<String>,
    #[serde(default)]
    pub confianca_riscos: Vec<String>,

    /// Série histórica de progresso e confiança
    #[serde(default)]
    pub history: Vec<HistoryPoint>,

    /// Área dinâmica livre construída pelo agente
    #[serde(default)]
    pub ui: Vec<UiNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiNode {
    Tabs {
        #[serde(default)]
        id: Option<String>,
        #[serde(default, alias = "items")]
        tabs: Vec<TabItem>,
    },
    Section {
        title: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        children: Vec<UiNode>,
    },
    Columns {
        #[serde(default)]
        columns: Option<u32>,
        #[serde(default)]
        children: Vec<UiNode>,
    },
    Prose {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        body: String,
        #[serde(default)]
        bullets: Vec<String>,
        #[serde(default)]
        tone: Option<Tone>,
    },
    Alert {
        #[serde(default, alias = "tone", alias = "kind")]
        level: AlertLevel,
        title: String,
        message: String,
    },
    KpiGrid {
        #[serde(default)]
        items: Vec<KpiItem>,
    },
    ProgressList {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        items: Vec<ProgressItem>,
    },
    Expander {
        title: String,
        #[serde(default)]
        subtitle: String,
        #[serde(default)]
        badge: Option<String>,
        #[serde(default)]
        status: Option<KpiStatus>,
        #[serde(default)]
        expanded: bool,
        #[serde(default)]
        children: Vec<UiNode>,
    },
    Matrix {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        subtitle: Option<String>,
        #[serde(default)]
        columns: Option<u32>,
        #[serde(default)]
        cells: Vec<MatrixCell>,
    },
    FlowGraph {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        subtitle: Option<String>,
        #[serde(default)]
        nodes: Vec<FlowNode>,
        #[serde(default)]
        edges: Vec<FlowEdge>,
    },
    Table {
        #[serde(default)]
        title: Option<String>,
        columns: Vec<String>,
        #[serde(default)]
        rows: Vec<Vec<String>>,
    },
    Timeline {
        #[serde(default)]
        title: Option<String>,
        #[serde(default, alias = "events")]
        items: Vec<TimelineItem>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TabItem {
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub children: Vec<UiNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KpiItem {
    pub label: String,
    pub value: String,
    #[serde(default)]
    pub sublabel: String,
    #[serde(default)]
    pub status: KpiStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProgressItem {
    pub label: String,
    pub percent: f64,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub status: KpiStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MatrixCell {
    pub label: String,
    #[serde(default)]
    pub sublabel: String,
    #[serde(default)]
    pub state: CellState,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub sublabel: String,
    #[serde(default)]
    pub state: CellState,
    #[serde(default, alias = "stage")]
    pub col: usize,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TimelineItem {
    pub timestamp: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub state: CellState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "Erro JSON na linha {l}, coluna {c}: {}", self.message),
            _ => write!(f, "Erro de validação: {}", self.message),
        }
    }
}

impl std::error::Error for ValidationError {}

pub fn parse_and_validate_json(raw: &str) -> Result<ProjectDocument, ValidationError> {
    let raw_val: serde_json::Value = serde_json::from_str(raw).map_err(|err| ValidationError {
        message: err.to_string(),
        line: Some(err.line()),
        column: Some(err.column()),
    })?;
    if let Some(ui_arr) = raw_val.get("ui").and_then(|v| v.as_array()) {
        for (idx, item) in ui_arr.iter().enumerate() {
            validate_ui_node_unknown_keys(item, &format!("ui[{idx}]"))?;
        }
    }

    let mut doc: ProjectDocument = serde_json::from_str(raw).map_err(|err| ValidationError {
        message: err.to_string(),
        line: Some(err.line()),
        column: Some(err.column()),
    })?;
    doc.normalize_ids();
    doc.validate()?;
    Ok(doc)
}

fn validate_ui_node_unknown_keys(
    val: &serde_json::Value,
    path: &str,
) -> Result<(), ValidationError> {
    let Some(obj) = val.as_object() else {
        return Ok(());
    };
    let Some(kind) = obj.get("type").and_then(|v| v.as_str()) else {
        return Ok(());
    };
    let allowed: &[&str] = match kind {
        "tabs" => &["type", "id", "tabs", "items"],
        "section" => &["type", "title", "description", "children"],
        "columns" => &["type", "columns", "children"],
        "prose" => &["type", "title", "body", "bullets", "tone"],
        "alert" => &["type", "level", "tone", "kind", "title", "message"],
        "kpi_grid" => &["type", "items"],
        "progress_list" => &["type", "title", "items"],
        "expander" => &[
            "type", "title", "subtitle", "badge", "status", "expanded", "children",
        ],
        "matrix" => &["type", "title", "subtitle", "columns", "cells"],
        "flow_graph" => &["type", "title", "subtitle", "nodes", "edges"],
        "table" => &["type", "title", "columns", "rows"],
        "timeline" => &["type", "title", "items", "events"],
        _ => return Ok(()),
    };
    for key in obj.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ValidationError {
                message: format!(
                    "{path} (`{kind}`): campo desconhecido `{key}` (esperados: {})",
                    allowed.join(", ")
                ),
                line: None,
                column: None,
            });
        }
    }
    if let Some(children) = obj.get("children").and_then(|v| v.as_array()) {
        for (i, ch) in children.iter().enumerate() {
            validate_ui_node_unknown_keys(ch, &format!("{path}.children[{i}]"))?;
        }
    }
    let tabs_val = obj.get("tabs").or_else(|| obj.get("items"));
    if kind == "tabs" {
        if let Some(tabs_arr) = tabs_val.and_then(|v| v.as_array()) {
            for (t_i, tab) in tabs_arr.iter().enumerate() {
                if let Some(children) = tab.get("children").and_then(|v| v.as_array()) {
                    for (c_i, ch) in children.iter().enumerate() {
                        validate_ui_node_unknown_keys(
                            ch,
                            &format!("{path}.tabs[{t_i}].children[{c_i}]"),
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

impl ProjectDocument {
    pub fn normalize_ids(&mut self) {
        for (idx, node) in self.ui.iter_mut().enumerate() {
            normalize_node_ids(node, idx);
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.title.trim().is_empty() {
            return Err(ValidationError {
                message: "o campo obrigatório `title` não pode estar vazio".to_string(),
                line: None,
                column: None,
            });
        }
        validate_percent("progresso", self.progresso)?;
        validate_percent("confianca", self.confianca)?;
        if let Some(d) = self.progresso_delta {
            if !d.is_finite() || !(-100.0..=100.0).contains(&d) {
                return Err(ValidationError {
                    message: format!("`progresso_delta` fora do intervalo [-100, 100]: {d}"),
                    line: None,
                    column: None,
                });
            }
        }
        if let Some(d) = self.confianca_delta {
            if !d.is_finite() || !(-100.0..=100.0).contains(&d) {
                return Err(ValidationError {
                    message: format!("`confianca_delta` fora do intervalo [-100, 100]: {d}"),
                    line: None,
                    column: None,
                });
            }
        }
        for (idx, hp) in self.history.iter().enumerate() {
            validate_percent(&format!("history[{idx}].progresso"), hp.progresso)?;
            validate_percent(&format!("history[{idx}].confianca"), hp.confianca)?;
        }
        for (idx, node) in self.ui.iter().enumerate() {
            validate_ui_node(node, &format!("ui[{idx}]"))?;
        }
        Ok(())
    }
}

fn normalize_node_ids(node: &mut UiNode, idx: usize) {
    match node {
        UiNode::Tabs { id, tabs } => {
            if id.as_deref().unwrap_or("").trim().is_empty() {
                *id = Some(format!("tabs_{idx}"));
            }
            for (t_idx, tab) in tabs.iter_mut().enumerate() {
                if tab.id.trim().is_empty() {
                    let slug = slugify_simple(&tab.title);
                    tab.id = if slug.is_empty() {
                        format!("tab_{t_idx}")
                    } else {
                        slug
                    };
                }
                for (c_idx, child) in tab.children.iter_mut().enumerate() {
                    normalize_node_ids(child, c_idx);
                }
            }
        }
        UiNode::Section { children, .. }
        | UiNode::Columns { children, .. }
        | UiNode::Expander { children, .. } => {
            for (c_idx, child) in children.iter_mut().enumerate() {
                normalize_node_ids(child, c_idx);
            }
        }
        _ => {}
    }
}

fn slugify_simple(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn validate_percent(name: &str, val: f64) -> Result<(), ValidationError> {
    if !val.is_finite() || !(0.0..=100.0).contains(&val) {
        return Err(ValidationError {
            message: format!("`{name}` deve estar entre 0.0 e 100.0 (recebido: {val})"),
            line: None,
            column: None,
        });
    }
    Ok(())
}

fn validate_ui_node(node: &UiNode, path: &str) -> Result<(), ValidationError> {
    match node {
        UiNode::Tabs { tabs, .. } => {
            if tabs.is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`tabs`) deve conter ao menos uma aba em `tabs`"),
                    line: None,
                    column: None,
                });
            }
            let mut seen = HashSet::new();
            for (i, tab) in tabs.iter().enumerate() {
                if tab.title.trim().is_empty() {
                    return Err(ValidationError {
                        message: format!("{path}.tabs[{i}].title não pode estar vazio"),
                        line: None,
                        column: None,
                    });
                }
                if !seen.insert(tab.id.clone()) {
                    return Err(ValidationError {
                        message: format!("{path}.tabs[{i}] possui `id` duplicado: `{}`", tab.id),
                        line: None,
                        column: None,
                    });
                }
                for (j, child) in tab.children.iter().enumerate() {
                    validate_ui_node(child, &format!("{path}.tabs[{i}].children[{j}]"))?;
                }
            }
        }
        UiNode::Section {
            title, children, ..
        } => {
            if title.trim().is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`section`) requer `title` não vazio"),
                    line: None,
                    column: None,
                });
            }
            for (j, child) in children.iter().enumerate() {
                validate_ui_node(child, &format!("{path}.children[{j}]"))?;
            }
        }
        UiNode::Columns { children, .. } => {
            for (j, child) in children.iter().enumerate() {
                validate_ui_node(child, &format!("{path}.children[{j}]"))?;
            }
        }
        UiNode::Prose {
            title,
            body,
            bullets,
            ..
        } => {
            if title.as_deref().unwrap_or("").trim().is_empty()
                && body.trim().is_empty()
                && bullets.is_empty()
            {
                return Err(ValidationError {
                    message: format!("{path} (`prose`) deve ter `body`, `bullets` ou `title`"),
                    line: None,
                    column: None,
                });
            }
        }
        UiNode::Alert { title, message, .. } => {
            if title.trim().is_empty() && message.trim().is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`alert`) requer `title` ou `message`"),
                    line: None,
                    column: None,
                });
            }
        }
        UiNode::KpiGrid { items } => {
            if items.is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`kpi_grid`) requer ao menos 1 item em `items`"),
                    line: None,
                    column: None,
                });
            }
        }
        UiNode::ProgressList { items, .. } => {
            for (i, item) in items.iter().enumerate() {
                validate_percent(&format!("{path}.items[{i}].percent"), item.percent)?;
            }
        }
        UiNode::Expander {
            title, children, ..
        } => {
            if title.trim().is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`expander`) requer `title` não vazio"),
                    line: None,
                    column: None,
                });
            }
            for (j, child) in children.iter().enumerate() {
                validate_ui_node(child, &format!("{path}.children[{j}]"))?;
            }
        }
        UiNode::Matrix { cells, .. } => {
            if cells.is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`matrix`) requer ao menos 1 célula em `cells`"),
                    line: None,
                    column: None,
                });
            }
        }
        UiNode::FlowGraph { nodes, edges, .. } => {
            if nodes.is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`flow_graph`) requer ao menos 1 nó em `nodes`"),
                    line: None,
                    column: None,
                });
            }
            let mut ids = HashSet::new();
            for (i, n) in nodes.iter().enumerate() {
                if n.id.trim().is_empty() {
                    return Err(ValidationError {
                        message: format!("{path}.nodes[{i}].id não pode estar vazio"),
                        line: None,
                        column: None,
                    });
                }
                if !ids.insert(n.id.as_str()) {
                    return Err(ValidationError {
                        message: format!("{path}.nodes[{i}] possui `id` duplicado: `{}`", n.id),
                        line: None,
                        column: None,
                    });
                }
            }
            for (i, e) in edges.iter().enumerate() {
                if !ids.contains(e.from.as_str()) {
                    return Err(ValidationError {
                        message: format!(
                            "{path}.edges[{i}].from referencia nó inexistente `{}`",
                            e.from
                        ),
                        line: None,
                        column: None,
                    });
                }
                if !ids.contains(e.to.as_str()) {
                    return Err(ValidationError {
                        message: format!(
                            "{path}.edges[{i}].to referencia nó inexistente `{}`",
                            e.to
                        ),
                        line: None,
                        column: None,
                    });
                }
            }
        }
        UiNode::Table { columns, rows, .. } => {
            if columns.is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`table`) requer `columns` não vazio"),
                    line: None,
                    column: None,
                });
            }
            for (r_idx, row) in rows.iter().enumerate() {
                if row.len() != columns.len() {
                    return Err(ValidationError {
                        message: format!(
                            "{path}.rows[{r_idx}] tem {} colunas, mas `columns` define {}",
                            row.len(),
                            columns.len()
                        ),
                        line: None,
                        column: None,
                    });
                }
            }
        }
        UiNode::Timeline { items, .. } => {
            if items.is_empty() {
                return Err(ValidationError {
                    message: format!("{path} (`timeline`) requer ao menos 1 item em `items`"),
                    line: None,
                    column: None,
                });
            }
        }
    }
    Ok(())
}

/// Retorna a cor semântica Adwaita (R, G, B em 0..=1) para o anel de Confiança.
/// - `>= 75.0%` -> `#2ec27e` (verde)
/// - `45.0..75.0%` -> `#e5a50a` (âmbar)
/// - `< 45.0%` -> `#e01b24` (vermelho)
pub fn confidence_semantic_rgb(confianca: f64) -> (f64, f64, f64) {
    if confianca >= 75.0 {
        (0x2e as f64 / 255.0, 0xc2 as f64 / 255.0, 0x7e as f64 / 255.0)
    } else if confianca >= 45.0 {
        (0xe5 as f64 / 255.0, 0xa5 as f64 / 255.0, 0x0a as f64 / 255.0)
    } else {
        (0xe0 as f64 / 255.0, 0x1b as f64 / 255.0, 0x24 as f64 / 255.0)
    }
}

pub fn confidence_hex(confianca: f64) -> &'static str {
    if confianca >= 75.0 {
        "#2ec27e"
    } else if confianca >= 45.0 {
        "#e5a50a"
    } else {
        "#e01b24"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_optional_fields_default_cleanly() {
        let raw = r#"{
            "title": "Projeto Mínimo",
            "progresso": 12.5,
            "confianca": 80.0
        }"#;
        let doc = parse_and_validate_json(raw).expect("minimal JSON should parse");
        assert_eq!(doc.title, "Projeto Mínimo");
        assert_eq!(doc.status, ProjectStatus::Ativo);
        assert_eq!(doc.icon, "applications-engineering-symbolic");
        assert!(doc.ui.is_empty());
        assert!(doc.history.is_empty());
    }

    #[test]
    fn unknown_field_reports_line_and_column() {
        let raw = "{\n  \"title\": \"Teste\",\n  \"progreso_errado\": 10.0\n}";
        let err = parse_and_validate_json(raw).expect_err("typo field must fail");
        assert_eq!(err.line, Some(3));
        assert!(err.column.unwrap_or(0) > 0);
        assert!(err.message.contains("progreso_errado"));
    }

    #[test]
    fn unknown_ui_node_field_rejected() {
        let raw = r#"{
            "title": "Teste",
            "progresso": 10.0,
            "confianca": 50.0,
            "ui": [{ "type": "section", "title": "S1", "childrens": [] }]
        }"#;
        let err = parse_and_validate_json(raw).expect_err("typo in ui node must fail");
        assert!(err.message.contains("childrens"));
    }

    #[test]
    fn full_ui_tree_roundtrips_and_validates() {
        let raw = r#"{
            "slug": "demo",
            "title": "Demo Completo",
            "status": "validando",
            "progresso": 42.0,
            "confianca": 76.5,
            "ui": [
                {
                    "type": "tabs",
                    "tabs": [
                        {
                            "id": "visao",
                            "title": "Visão Geral",
                            "children": [
                                { "type": "alert", "level": "info", "title": "Aviso", "message": "Ativo" },
                                { "type": "prose", "title": "Resumo", "body": "Linha 1\n- Item A" },
                                {
                                    "type": "columns",
                                    "children": [
                                        {
                                            "type": "kpi_grid",
                                            "items": [
                                                { "label": "Testes", "value": "398", "status": "ok" }
                                            ]
                                        },
                                        {
                                            "type": "progress_list",
                                            "items": [
                                                { "label": "Onda 0", "percent": 80.0, "status": "ok" }
                                            ]
                                        }
                                    ]
                                },
                                {
                                    "type": "section",
                                    "title": "Detalhes",
                                    "children": [
                                        {
                                            "type": "expander",
                                            "title": "Matriz e Fluxo",
                                            "badge": "2/2 OK",
                                            "children": [
                                                {
                                                    "type": "matrix",
                                                    "cells": [
                                                        { "label": "C0", "state": "pass", "detail": "OK" },
                                                        { "label": "C5", "state": "reserved", "detail": "Sistema" }
                                                    ]
                                                },
                                                {
                                                    "type": "flow_graph",
                                                    "nodes": [
                                                        { "id": "a", "label": "A", "col": 0, "state": "pass" },
                                                        { "id": "b", "label": "B", "col": 1, "state": "running" }
                                                    ],
                                                    "edges": [
                                                        { "from": "a", "to": "b", "label": "IPC" }
                                                    ]
                                                },
                                                {
                                                    "type": "table",
                                                    "columns": ["Col 1", "Col 2"],
                                                    "rows": [["V1", "V2"]]
                                                },
                                                {
                                                    "type": "timeline",
                                                    "items": [
                                                        { "timestamp": "21:45", "title": "Build", "state": "pass" }
                                                    ]
                                                }
                                            ]
                                        }
                                    ]
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;
        let doc = parse_and_validate_json(raw).expect("full tree should parse and validate");
        let serialized = serde_json::to_string_pretty(&doc).expect("serialize");
        let reparsed = parse_and_validate_json(&serialized).expect("reparse");
        assert_eq!(doc, reparsed);
    }

    #[test]
    fn out_of_bounds_percentage_rejected() {
        let raw = r#"{"title": "X", "progresso": 105.0, "confianca": 50.0}"#;
        let err = parse_and_validate_json(raw).expect_err("must reject > 100");
        assert!(err.message.contains("progresso"));
    }

    #[test]
    fn broken_flow_edge_rejected() {
        let raw = r#"{
            "title": "X",
            "progresso": 10.0,
            "confianca": 50.0,
            "ui": [{
                "type": "flow_graph",
                "nodes": [{"id": "n1", "label": "N1"}],
                "edges": [{"from": "n1", "to": "inexistente"}]
            }]
        }"#;
        let err = parse_and_validate_json(raw).expect_err("must reject missing node ref");
        assert!(err.message.contains("inexistente"));
    }
}
