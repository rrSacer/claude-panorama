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
            Self::Success => "object-select-symbolic",
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
            Self::Pass => "Pronto",
            Self::Running => "Em andamento",
            Self::Fail => "Com falha",
            Self::Pending => "A fazer",
            Self::Blocked => "Travado",
            Self::Reserved => "Reservado",
        }
    }

    pub fn short_tag(self) -> &'static str {
        match self {
            Self::Pass => "PRONTO",
            Self::Running => "EM CURSO",
            Self::Fail => "FALHOU",
            Self::Pending => "A FAZER",
            Self::Blocked => "TRAVADO",
            Self::Reserved => "RESERV.",
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
        icon: Option<String>,
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
    Donut {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        subtitle: Option<String>,
        #[serde(default)]
        center_value: Option<String>,
        #[serde(default)]
        center_label: Option<String>,
        #[serde(default)]
        slices: Vec<ChartSlice>,
    },
    BarChart {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        subtitle: Option<String>,
        #[serde(default)]
        unit: Option<String>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        bars: Vec<ChartBar>,
    },
    Steps {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        subtitle: Option<String>,
        #[serde(default)]
        first_number: Option<usize>,
        #[serde(default)]
        steps: Vec<StepItem>,
    },
    Cards {
        #[serde(default)]
        columns: Option<u32>,
        #[serde(default)]
        items: Vec<CardItem>,
    },
    Glossary {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        items: Vec<GlossaryItem>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChartSlice {
    pub label: String,
    pub value: f64,
    #[serde(default)]
    pub state: CellState,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChartBar {
    pub label: String,
    pub value: f64,
    #[serde(default)]
    pub value_label: Option<String>,
    #[serde(default)]
    pub state: CellState,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StepItem {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub state: CellState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CardItem {
    #[serde(default)]
    pub icon: Option<String>,
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub tone: Option<Tone>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GlossaryItem {
    pub term: String,
    pub meaning: String,
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
            "type", "title", "subtitle", "icon", "badge", "status", "expanded", "children",
        ],
        "matrix" => &["type", "title", "subtitle", "columns", "cells"],
        "flow_graph" => &["type", "title", "subtitle", "nodes", "edges"],
        "table" => &["type", "title", "columns", "rows"],
        "timeline" => &["type", "title", "items", "events"],
        "donut" => &["type", "title", "subtitle", "center_value", "center_label", "slices"],
        "bar_chart" => &["type", "title", "subtitle", "unit", "max", "bars"],
        "steps" => &["type", "title", "subtitle", "first_number", "steps"],
        "cards" => &["type", "columns", "items"],
        "glossary" => &["type", "title", "items"],
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
        UiNode::Donut { slices, .. } => {
            require_non_empty(path, "donut", "slices", slices.len())?;
            for (i, s) in slices.iter().enumerate() {
                validate_non_negative(&format!("{path}.slices[{i}].value"), s.value)?;
                validate_color(&format!("{path}.slices[{i}].color"), s.color.as_deref())?;
            }
            if slices.iter().map(|s| s.value).sum::<f64>() <= 0.0 {
                return Err(ValidationError {
                    message: format!("{path} (`donut`) precisa de ao menos uma fatia com `value` > 0"),
                    line: None,
                    column: None,
                });
            }
        }
        UiNode::BarChart { bars, max, .. } => {
            require_non_empty(path, "bar_chart", "bars", bars.len())?;
            if let Some(m) = max {
                if !m.is_finite() || *m <= 0.0 {
                    return Err(ValidationError {
                        message: format!("{path}.max deve ser maior que 0 (recebido: {m})"),
                        line: None,
                        column: None,
                    });
                }
            }
            for (i, b) in bars.iter().enumerate() {
                validate_non_negative(&format!("{path}.bars[{i}].value"), b.value)?;
                validate_color(&format!("{path}.bars[{i}].color"), b.color.as_deref())?;
            }
        }
        UiNode::Steps { steps, .. } => {
            require_non_empty(path, "steps", "steps", steps.len())?;
        }
        UiNode::Cards { items, .. } => {
            require_non_empty(path, "cards", "items", items.len())?;
        }
        UiNode::Glossary { items, .. } => {
            require_non_empty(path, "glossary", "items", items.len())?;
        }
    }
    Ok(())
}

fn require_non_empty(path: &str, kind: &str, field: &str, len: usize) -> Result<(), ValidationError> {
    if len == 0 {
        return Err(ValidationError {
            message: format!("{path} (`{kind}`) requer ao menos 1 item em `{field}`"),
            line: None,
            column: None,
        });
    }
    Ok(())
}

fn validate_non_negative(name: &str, val: f64) -> Result<(), ValidationError> {
    if !val.is_finite() || val < 0.0 {
        return Err(ValidationError {
            message: format!("`{name}` deve ser um número >= 0 (recebido: {val})"),
            line: None,
            column: None,
        });
    }
    Ok(())
}

fn validate_color(name: &str, color: Option<&str>) -> Result<(), ValidationError> {
    match color {
        Some(c) if palette_rgb(c).is_none() => Err(ValidationError {
            message: format!(
                "`{name}` tem cor desconhecida `{c}` (aceitas: {})",
                PALETTE_NAMES.join(", ")
            ),
            line: None,
            column: None,
        }),
        _ => Ok(()),
    }
}

pub const PALETTE_NAMES: &[&str] = &[
    "blue", "teal", "green", "yellow", "orange", "red", "pink", "purple", "brown", "gray",
];

/// Paleta de destaque do GNOME (Libadwaita) para fatias e barras com `color` explícito.
pub fn palette_rgb(name: &str) -> Option<(f64, f64, f64)> {
    let hex = match name {
        "blue" => 0x3584e4,
        "teal" => 0x2190a4,
        "green" => 0x2ec27e,
        "yellow" => 0xe5a50a,
        "orange" => 0xff7800,
        "red" => 0xe01b24,
        "pink" => 0xd56199,
        "purple" => 0x9141ac,
        "brown" => 0x986a44,
        "gray" => 0x77767b,
        _ => return None,
    };
    Some((
        ((hex >> 16) & 0xff) as f64 / 255.0,
        ((hex >> 8) & 0xff) as f64 / 255.0,
        (hex & 0xff) as f64 / 255.0,
    ))
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
    fn visual_blocks_parse_and_roundtrip() {
        let raw = r#"{
            "title": "Visual",
            "progresso": 10.0,
            "confianca": 50.0,
            "ui": [
                { "type": "cards", "items": [{ "icon": "help-about-symbolic", "title": "O que é", "text": "Explicação", "tone": "info" }] },
                { "type": "donut", "center_label": "itens", "slices": [
                    { "label": "Pronto", "value": 30, "state": "pass" },
                    { "label": "Falta", "value": 70, "color": "gray" }
                ] },
                { "type": "bar_chart", "unit": "testes", "bars": [{ "label": "Reator", "value": 133, "detail": "ok" }] },
                { "type": "steps", "steps": [{ "title": "Base", "state": "pass" }, { "title": "Reatores", "description": "próxima" }] },
                { "type": "glossary", "items": [{ "term": "Save", "meaning": "Gravar o mundo no disco" }] },
                { "type": "expander", "title": "Detalhes", "icon": "dialog-information-symbolic", "children": [] }
            ]
        }"#;
        let doc = parse_and_validate_json(raw).expect("visual blocks should parse");
        let again = parse_and_validate_json(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(doc, again);
    }

    #[test]
    fn visual_blocks_reject_bad_values() {
        let bad_color = r#"{"title": "X", "progresso": 1.0, "confianca": 1.0,
            "ui": [{ "type": "donut", "slices": [{ "label": "A", "value": 1, "color": "magenta" }] }]}"#;
        assert!(parse_and_validate_json(bad_color).unwrap_err().message.contains("magenta"));
        let zero_donut = r#"{"title": "X", "progresso": 1.0, "confianca": 1.0,
            "ui": [{ "type": "donut", "slices": [{ "label": "A", "value": 0 }] }]}"#;
        assert!(parse_and_validate_json(zero_donut).is_err());
        let negative_bar = r#"{"title": "X", "progresso": 1.0, "confianca": 1.0,
            "ui": [{ "type": "bar_chart", "bars": [{ "label": "A", "value": -2 }] }]}"#;
        assert!(parse_and_validate_json(negative_bar).is_err());
        let typo = r#"{"title": "X", "progresso": 1.0, "confianca": 1.0,
            "ui": [{ "type": "steps", "passos": [] }]}"#;
        assert!(parse_and_validate_json(typo).unwrap_err().message.contains("passos"));
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
