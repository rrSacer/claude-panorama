use crate::gui::charts::{
    build_bar_chart_widget, build_donut_widget, build_flow_graph_widget, build_matrix_widget,
    build_ring_gauge, build_sparkline, build_step_marker, GaugeKind,
};
use crate::hardware::{
    detect_project_hardware, format_cpu_ranges, CpuCoreInfo, LiveHardwareStats,
    SystemHardwareSpecs,
};
use crate::ipc::TraycerCatalog;
use crate::model::{
    CardItem, CellState, GlossaryItem, HardwareConfig, KpiStatus, ProjectDocument, StepItem, Tone,
    UiNode,
};
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use gtk4::gio;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::f64::consts::TAU;
use std::rc::Rc;

#[derive(Clone)]
pub struct ProjectViewCallbacks {
    pub catalog: TraycerCatalog,
    pub hw_specs: SystemHardwareSpecs,
    pub live_hw: Option<LiveHardwareStats>,
    pub on_start_or_configure_agent: Rc<dyn Fn(String, String, Option<String>)>,
    pub on_request_update: Rc<dyn Fn()>,
    pub on_refresh_models: Rc<dyn Fn()>,
    pub on_hardware_changed: Rc<dyn Fn(HardwareConfig)>,
    pub on_edit_project: Rc<dyn Fn()>,
}

#[derive(Default, Clone)]
pub struct LiveHardwareWidgets {
    pub slug: String,
    pub show_cpu: bool,
    pub show_ram: bool,
    pub show_ssd: bool,
    pub status_pill: Option<gtk4::Label>,
    pub cpu_live_lbl: Option<gtk4::Label>,
    pub cpu_bar: Option<gtk4::ProgressBar>,
    pub ssd_live_lbl: Option<gtk4::Label>,
    pub ssd_bar: Option<gtk4::ProgressBar>,
    pub ram_live_lbl: Option<gtk4::Label>,
    pub ram_bar: Option<gtk4::ProgressBar>,
}

impl LiveHardwareWidgets {
    pub fn update(&self, stats: &LiveHardwareStats, specs: &SystemHardwareSpecs) {
        if let Some(pill) = &self.status_pill {
            pill.remove_css_class("pill-pass");
            pill.remove_css_class("pill-run");
            pill.remove_css_class("pill-pend");
            if stats.active_pids > 0 && stats.enforced {
                pill.set_text(&format!("Limitado · {} proc.", stats.active_pids));
                pill.add_css_class("pill-pass");
            } else if stats.active_pids > 0 {
                pill.set_text(&format!("Ativo · {} proc.", stats.active_pids));
                pill.add_css_class("pill-run");
            } else if stats.enforced {
                pill.set_text("Limite armado");
                pill.add_css_class("pill-pass");
            } else {
                pill.set_text("Ocioso");
                pill.add_css_class("pill-pend");
            }
        }
        if let (Some(lbl), Some(bar)) = (&self.cpu_live_lbl, &self.cpu_bar) {
            let total_t = specs.total_cpus.max(1);
            let phys_c = specs.physical_cores.len().max(1);
            let approx_threads = (stats.cpu_percent / 100.0) * (total_t as f64);
            lbl.set_text(&format!(
                "Uso atual: {:.1}% (~{:.1}/{} threads · {} núcleos)",
                stats.cpu_percent, approx_threads, total_t, phys_c
            ));
            bar.set_fraction((stats.cpu_percent / 100.0).clamp(0.0, 1.0));
        }
        if let (Some(lbl), Some(bar)) = (&self.ssd_live_lbl, &self.ssd_bar) {
            lbl.set_text(&format!(
                "Taxa atual: {:.1} MB/s ({:.1}%)",
                stats.ssd_mb_s, stats.ssd_percent
            ));
            bar.set_fraction((stats.ssd_percent / 100.0).clamp(0.0, 1.0));
        }
        if let (Some(lbl), Some(bar)) = (&self.ram_live_lbl, &self.ram_bar) {
            let used_gb = stats.ram_used_mb / 1024.0;
            let total = specs.total_ram_gb.max(1.0);
            lbl.set_text(&format!(
                "Uso atual: {:.2} GB ({:.0} MB) de {:.1} GB",
                used_gb, stats.ram_used_mb, total
            ));
            bar.set_fraction((used_gb / total).clamp(0.0, 1.0));
        }
    }
}

#[derive(Default, Clone)]
pub struct UiViewState {
    pub active_tabs: Rc<RefCell<HashMap<String, String>>>,
    pub expanded_rows: Rc<RefCell<HashMap<String, bool>>>,
    pub live_hw_widgets: Rc<RefCell<Option<LiveHardwareWidgets>>>,
}

pub fn render_project_view(
    doc: &ProjectDocument,
    state: &UiViewState,
    callbacks: &ProjectViewCallbacks,
) -> gtk4::Widget {
    *state.live_hw_widgets.borrow_mut() = None;

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    root.set_margin_top(16);
    root.set_margin_bottom(28);
    root.set_margin_start(18);
    root.set_margin_end(18);

    // 1. Faixa de identificação do projeto
    root.append(&build_project_banner(doc, callbacks.on_edit_project.clone()));

    // 2. Painel do Agente Dedicado + Limites Seletivos de Hardware em Tempo Real
    root.append(&build_dedicated_agent_card(doc, callbacks));

    if let Some((hw_card, live_widgets)) = build_selective_hardware_card(doc, callbacks) {
        *state.live_hw_widgets.borrow_mut() = Some(live_widgets);
        root.append(&hw_card);
    }

    // 3. Cabeçalho Fixo Obrigatório (Progresso + Confiança)
    root.append(&build_fixed_header_pair(doc, state));

    // 4. Área Dinâmica Livre (`ui`)
    if doc.ui.is_empty() {
        let slug_cmd = if doc.slug.is_empty() {
            "<slug>"
        } else {
            doc.slug.as_str()
        };
        let status_page = adw::StatusPage::builder()
            .icon_name("utilities-terminal-symbolic")
            .title("Aguardando panorama dinâmico do agente")
            .description(format!(
                "Os indicadores fixos já estão ativos acima.\nPara construir abas, matrizes, grafos e tabelas dinâmicas neste projeto, execute:\n\npanorama apply {slug_cmd} panorama.json"
            ))
            .vexpand(true)
            .build();
        status_page.add_css_class("compact");
        let wrap_card = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        wrap_card.add_css_class("card");
        wrap_card.append(&status_page);
        root.append(&wrap_card);
    } else {
        let dynamic_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        for (idx, node) in doc.ui.iter().enumerate() {
            let path_key = format!("{}::root[{idx}]", doc.slug);
            dynamic_box.append(&render_ui_node(node, &doc.slug, &path_key, state));
        }
        root.append(&dynamic_box);
    }

    let clamp = adw::Clamp::builder()
        .maximum_size(1360)
        .tightening_threshold(920)
        .child(&root)
        .build();

    clamp.upcast()
}

fn build_dedicated_agent_card(
    doc: &ProjectDocument,
    callbacks: &ProjectViewCallbacks,
) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");

    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let icon = gtk4::Image::from_icon_name("avatar-default-symbolic");
    icon.set_pixel_size(20);
    icon.set_valign(gtk4::Align::Center);
    icon.add_css_class("accent");
    top.append(&icon);

    let title_col = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    title_col.set_hexpand(true);

    let title_line = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let title_lbl = gtk4::Label::new(Some("Agente Dedicado (Panorama)"));
    title_lbl.add_css_class("heading");
    title_lbl.set_xalign(0.0);
    title_line.append(&title_lbl);

    let has_agent = doc
        .agent
        .as_ref()
        .and_then(|a| a.agent_id.as_deref())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let pill = if has_agent {
        let aid = doc
            .agent
            .as_ref()
            .and_then(|a| a.agent_id.as_deref())
            .unwrap_or("");
        let short_id = if aid.len() > 8 { &aid[..8] } else { aid };
        let l = gtk4::Label::new(Some(&format!("Ativo · {short_id}")));
        l.add_css_class("mini-pill");
        l.add_css_class("pill-pass");
        l
    } else {
        let l = gtk4::Label::new(Some("Aguardando maestro"));
        l.add_css_class("mini-pill");
        l.add_css_class("pill-pend");
        l
    };
    title_line.append(&pill);
    title_col.append(&title_line);

    let mut sub_parts: Vec<String> = Vec::new();
    if let Some(ag) = &doc.agent {
        if let Some(st) = &ag.last_status {
            if !st.trim().is_empty() {
                sub_parts.push(st.clone());
            }
        }
        if let Some(ts) = &ag.last_requested_at {
            if !ts.trim().is_empty() {
                sub_parts.push(format!("Último pedido: {ts}"));
            }
        }
    }
    if sub_parts.is_empty() {
        if has_agent {
            sub_parts.push("Subagente dedicado vinculado à conversa do projeto no Traycer. Atualiza este panorama quando você clicar em Atualizar.".to_string());
        } else {
            sub_parts.push("O maestro da conversa deste projeto no Traycer cria este subagente na aba Agents e o vincula a este panorama.".to_string());
        }
    }
    let sub_lbl = gtk4::Label::new(Some(&sub_parts.join(" · ")));
    sub_lbl.add_css_class("caption");
    sub_lbl.add_css_class("dim-label");
    sub_lbl.set_xalign(0.0);
    sub_lbl.set_wrap(true);
    sub_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    title_col.append(&sub_lbl);
    top.append(&title_col);

    let refresh_models_btn = gtk4::Button::from_icon_name("system-run-symbolic");
    refresh_models_btn.add_css_class("flat");
    refresh_models_btn.set_valign(gtk4::Align::Center);
    refresh_models_btn.set_tooltip_text(Some("Recarregar harnesses, modelos e efforts disponíveis no Traycer"));
    let on_refresh_models = callbacks.on_refresh_models.clone();
    refresh_models_btn.connect_clicked(move |_| {
        on_refresh_models();
    });
    top.append(&refresh_models_btn);

    let update_now_content = adw::ButtonContent::builder()
        .icon_name("view-refresh-symbolic")
        .label("Pedir Atualização")
        .build();
    let update_now_btn = gtk4::Button::builder()
        .child(&update_now_content)
        .valign(gtk4::Align::Center)
        .build();
    update_now_btn.set_tooltip_text(Some("Enviar pedido ao subagente dedicado para atualizar este panorama"));
    let on_req = callbacks.on_request_update.clone();
    update_now_btn.connect_clicked(move |_| {
        on_req();
    });
    update_now_btn.set_visible(has_agent);
    refresh_models_btn.set_visible(has_agent);
    top.append(&update_now_btn);
    card.append(&top);

    // Controles em 2 linhas: Harness + Modelo (linha 1) e Effort + Aplicar Modelo (linha 2)
    let controls = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    controls.set_hexpand(true);

    let saved_harness = doc
        .agent
        .as_ref()
        .map(|a| a.harness_id.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            callbacks
                .catalog
                .harnesses
                .first()
                .map(|h| h.id.clone())
                .unwrap_or_else(|| "antigravity".to_string())
        });
    let saved_model = doc
        .agent
        .as_ref()
        .map(|a| a.model.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let saved_effort = doc
        .agent
        .as_ref()
        .and_then(|a| a.reasoning_effort.clone());

    let harnesses = merge_catalog_with_saved(
        &callbacks.catalog,
        &saved_harness,
        &saved_model,
        saved_effort.as_deref(),
    );

    let harness_labels: Vec<String> = harnesses
        .iter()
        .map(|h| format!("{} ({})", h.label, h.id))
        .collect();
    let harness_str_refs: Vec<&str> = harness_labels.iter().map(|s| s.as_str()).collect();
    let harness_dd = gtk4::DropDown::from_strings(&harness_str_refs);
    harness_dd.set_valign(gtk4::Align::Center);
    harness_dd.set_tooltip_text(Some("Harness do agente no Traycer"));
    let initial_h_idx = harnesses
        .iter()
        .position(|h| h.id == saved_harness)
        .unwrap_or(0);
    harness_dd.set_selected(initial_h_idx as u32);

    let model_dd = gtk4::DropDown::from_strings(&[]);
    model_dd.set_hexpand(true);
    model_dd.set_valign(gtk4::Align::Center);
    model_dd.set_tooltip_text(Some("Modelo do Traycer usado pelo agente"));

    // Evita que IDs muito longos (ex.: Hermes) alarguem o card; lista completa no popup
    let model_btn_factory = gtk4::SignalListItemFactory::new();
    model_btn_factory.connect_setup(|_, obj| {
        if let Some(item) = obj.downcast_ref::<gtk4::ListItem>() {
            let lbl = gtk4::Label::new(None);
            lbl.set_xalign(0.0);
            lbl.set_hexpand(true);
            lbl.set_ellipsize(pango::EllipsizeMode::End);
            item.set_child(Some(&lbl));
        }
    });
    model_btn_factory.connect_bind(|_, obj| {
        if let Some(item) = obj.downcast_ref::<gtk4::ListItem>() {
            if let (Some(lbl), Some(so)) = (
                item.child().and_downcast::<gtk4::Label>(),
                item.item().and_downcast::<gtk4::StringObject>(),
            ) {
                lbl.set_text(&so.string());
            }
        }
    });
    let model_list_factory = gtk4::SignalListItemFactory::new();
    model_list_factory.connect_setup(|_, obj| {
        if let Some(item) = obj.downcast_ref::<gtk4::ListItem>() {
            let lbl = gtk4::Label::new(None);
            lbl.set_xalign(0.0);
            item.set_child(Some(&lbl));
        }
    });
    model_list_factory.connect_bind(|_, obj| {
        if let Some(item) = obj.downcast_ref::<gtk4::ListItem>() {
            if let (Some(lbl), Some(so)) = (
                item.child().and_downcast::<gtk4::Label>(),
                item.item().and_downcast::<gtk4::StringObject>(),
            ) {
                lbl.set_text(&so.string());
            }
        }
    });
    model_dd.set_factory(Some(&model_btn_factory));
    model_dd.set_list_factory(Some(&model_list_factory));

    let effort_dd = gtk4::DropDown::from_strings(&["Padrão (N/A)"]);
    effort_dd.set_valign(gtk4::Align::Center);
    effort_dd.set_tooltip_text(Some("Nível de reasoning effort do modelo"));

    let apply_btn = gtk4::Button::builder()
        .label("Aplicar Modelo")
        .valign(gtk4::Align::Center)
        .build();

    let harnesses_rc = Rc::new(harnesses);
    let current_models: Rc<RefCell<Vec<crate::ipc::TraycerModelInfo>>> =
        Rc::new(RefCell::new(Vec::new()));
    let current_efforts: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let syncing = Rc::new(Cell::new(false));

    let populate_efforts = {
        let current_efforts = current_efforts.clone();
        let effort_dd = effort_dd.clone();
        move |efforts: &[String], preferred: Option<&str>| {
            *current_efforts.borrow_mut() = efforts.to_vec();
            effort_dd.set_visible(true);
            if efforts.is_empty() {
                let na_model = gtk4::StringList::new(&["Padrão (N/A)"]);
                effort_dd.set_model(Some(&na_model));
                effort_dd.set_selected(0);
                effort_dd.set_sensitive(false);
                effort_dd.set_tooltip_text(Some(
                    "Este harness/modelo não usa seleção de reasoning effort",
                ));
            } else {
                let refs: Vec<&str> = efforts.iter().map(|s| s.as_str()).collect();
                let list = gtk4::StringList::new(&refs);
                effort_dd.set_model(Some(&list));
                let idx = preferred
                    .and_then(|p| efforts.iter().position(|e| e == p))
                    .or_else(|| efforts.iter().position(|e| e == "high"))
                    .unwrap_or(0);
                effort_dd.set_selected(idx as u32);
                effort_dd.set_sensitive(true);
                effort_dd.set_tooltip_text(Some("Nível de reasoning effort suportado pelo modelo"));
            }
        }
    };

    let populate_models = {
        let harnesses_rc = harnesses_rc.clone();
        let current_models = current_models.clone();
        let model_dd = model_dd.clone();
        let populate_efforts = populate_efforts.clone();
        move |h_idx: usize, preferred_model: Option<&str>, preferred_effort: Option<&str>| {
            let models = harnesses_rc
                .get(h_idx)
                .map(|h| h.models.clone())
                .unwrap_or_default();
            *current_models.borrow_mut() = models.clone();
            let refs: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
            let list = gtk4::StringList::new(&refs);
            model_dd.set_model(Some(&list));
            if models.is_empty() {
                populate_efforts(&[], None);
            } else {
                let m_idx = preferred_model
                    .and_then(|pm| models.iter().position(|m| m.id == pm))
                    .unwrap_or(0);
                model_dd.set_selected(m_idx as u32);
                populate_efforts(&models[m_idx].reasoning_efforts, preferred_effort);
            }
        }
    };

    syncing.set(true);
    populate_models(
        initial_h_idx,
        if saved_model.is_empty() { None } else { Some(saved_model.as_str()) },
        saved_effort.as_deref(),
    );
    syncing.set(false);

    {
        let syncing = syncing.clone();
        let populate_models = populate_models.clone();
        let apply_btn = apply_btn.clone();
        harness_dd.connect_selected_notify(move |dd| {
            if syncing.get() {
                return;
            }
            syncing.set(true);
            populate_models(dd.selected() as usize, None, None);
            syncing.set(false);
            apply_btn.add_css_class("suggested-action");
        });
    }

    {
        let syncing = syncing.clone();
        let current_models = current_models.clone();
        let populate_efforts = populate_efforts.clone();
        let apply_btn = apply_btn.clone();
        model_dd.connect_selected_notify(move |dd| {
            if syncing.get() {
                return;
            }
            syncing.set(true);
            let idx = dd.selected() as usize;
            let models = current_models.borrow();
            if let Some(m) = models.get(idx) {
                populate_efforts(&m.reasoning_efforts, None);
            }
            syncing.set(false);
            apply_btn.add_css_class("suggested-action");
        });
    }

    {
        let syncing = syncing.clone();
        let apply_btn = apply_btn.clone();
        effort_dd.connect_selected_notify(move |_| {
            if !syncing.get() {
                apply_btn.add_css_class("suggested-action");
            }
        });
    }

    {
        let harnesses_rc = harnesses_rc.clone();
        let current_models = current_models.clone();
        let current_efforts = current_efforts.clone();
        let harness_dd = harness_dd.clone();
        let model_dd = model_dd.clone();
        let effort_dd = effort_dd.clone();
        let on_cfg = callbacks.on_start_or_configure_agent.clone();
        apply_btn.connect_clicked(move |_| {
            let h_idx = harness_dd.selected() as usize;
            let Some(h) = harnesses_rc.get(h_idx) else {
                return;
            };
            let m_idx = model_dd.selected() as usize;
            let models = current_models.borrow();
            let Some(m) = models.get(m_idx) else {
                return;
            };
            let eff = if effort_dd.is_sensitive() {
                current_efforts
                    .borrow()
                    .get(effort_dd.selected() as usize)
                    .cloned()
            } else {
                None
            };
            on_cfg(h.id.clone(), m.id.clone(), eff);
        });
    }

    let row1 = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row1.set_hexpand(true);
    let harness_lbl = gtk4::Label::new(Some("Harness"));
    harness_lbl.add_css_class("caption");
    harness_lbl.add_css_class("dim-label");
    harness_lbl.set_valign(gtk4::Align::Center);
    let model_lbl = gtk4::Label::new(Some("Modelo"));
    model_lbl.add_css_class("caption");
    model_lbl.add_css_class("dim-label");
    model_lbl.set_valign(gtk4::Align::Center);
    row1.append(&harness_lbl);
    row1.append(&harness_dd);
    row1.append(&model_lbl);
    row1.append(&model_dd);

    let row2 = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    row2.set_hexpand(true);
    let effort_lbl = gtk4::Label::new(Some("Effort"));
    effort_lbl.add_css_class("caption");
    effort_lbl.add_css_class("dim-label");
    effort_lbl.set_valign(gtk4::Align::Center);
    let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    row2.append(&effort_lbl);
    row2.append(&effort_dd);
    row2.append(&spacer);
    row2.append(&apply_btn);

    controls.append(&row1);
    controls.append(&row2);
    controls.set_visible(has_agent);
    card.append(&controls);

    card.upcast()
}

fn build_selective_hardware_card(
    doc: &ProjectDocument,
    callbacks: &ProjectViewCallbacks,
) -> Option<(gtk4::Widget, LiveHardwareWidgets)> {
    let needs = detect_project_hardware(doc, callbacks.live_hw.as_ref());
    if !needs.any() {
        return None;
    }

    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");

    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let icon = gtk4::Image::from_icon_name("computer-symbolic");
    icon.set_pixel_size(20);
    icon.set_valign(gtk4::Align::Center);
    icon.add_css_class("accent");
    top.append(&icon);

    let title_col = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    title_col.set_hexpand(true);
    let title = gtk4::Label::new(Some("Limites de Hardware em Tempo Real"));
    title.add_css_class("heading");
    title.set_xalign(0.0);
    title_col.append(&title);

    if !needs.reason.is_empty() {
        let r = gtk4::Label::new(Some(&needs.reason));
        r.add_css_class("caption");
        r.add_css_class("dim-label");
        r.set_xalign(0.0);
        r.set_wrap(true);
        r.set_wrap_mode(pango::WrapMode::WordChar);
        title_col.append(&r);
    }
    top.append(&title_col);

    let status_pill = gtk4::Label::new(Some("Monitorando"));
    status_pill.add_css_class("mini-pill");
    status_pill.add_css_class("pill-pend");
    status_pill.set_valign(gtk4::Align::Center);
    top.append(&status_pill);
    card.append(&top);

    let hw_state = Rc::new(RefCell::new(HardwareConfig {
        heavy_cpu: doc.hardware.as_ref().and_then(|h| h.heavy_cpu).or(Some(needs.show_cpu)),
        heavy_ram: doc.hardware.as_ref().and_then(|h| h.heavy_ram).or(Some(needs.show_ram)),
        heavy_ssd: doc.hardware.as_ref().and_then(|h| h.heavy_ssd).or(Some(needs.show_ssd)),
        reason: if !needs.reason.is_empty() {
            needs.reason.clone()
        } else {
            doc.hardware.as_ref().map(|h| h.reason.clone()).unwrap_or_default()
        },
        cpu_cores: doc.hardware.as_ref().and_then(|h| h.cpu_cores),
        cpu_threads: doc.hardware.as_ref().and_then(|h| h.cpu_threads.clone()),
        ssd_percent: doc.hardware.as_ref().and_then(|h| h.ssd_percent),
        max_ram_gb: doc.hardware.as_ref().and_then(|h| h.max_ram_gb),
    }));

    let mut live_widgets = LiveHardwareWidgets {
        slug: doc.slug.clone(),
        show_cpu: needs.show_cpu,
        show_ram: needs.show_ram,
        show_ssd: needs.show_ssd,
        status_pill: Some(status_pill),
        ..Default::default()
    };

    let grid = gtk4::Box::new(gtk4::Orientation::Vertical, 10);

    // 1. CPU — seleção específica de núcleos físicos e threads (apenas se usa muita CPU)
    if needs.show_cpu {
        let cores_topo: Vec<CpuCoreInfo> = if callbacks.hw_specs.physical_cores.is_empty() {
            (0..callbacks.hw_specs.total_cpus.max(1))
                .map(|i| CpuCoreInfo {
                    core_id: i,
                    threads: vec![i],
                })
                .collect()
        } else {
            callbacks.hw_specs.physical_cores.clone()
        };

        let init_enabled = hw_state
            .borrow()
            .cpu_threads
            .as_ref()
            .is_some_and(|v| !v.is_empty())
            || hw_state.borrow().cpu_cores.is_some();

        let init_threads: Vec<u32> = if let Some(ts) = hw_state
            .borrow()
            .cpu_threads
            .as_ref()
            .filter(|v| !v.is_empty())
        {
            ts.clone()
        } else if let Some(c) = hw_state.borrow().cpu_cores {
            (0..c.clamp(1, callbacks.hw_specs.total_cpus.max(1))).collect()
        } else {
            let take_cores = ((cores_topo.len() * 2) / 3).max(1).min(cores_topo.len());
            cores_topo
                .iter()
                .take(take_cores)
                .flat_map(|c| c.threads.iter().copied())
                .collect()
        };

        let (row_widget, live_lbl, bar) = build_cpu_topology_row(
            &cores_topo,
            callbacks.hw_specs.total_cpus.max(1),
            init_enabled,
            &init_threads,
            {
                let hw_state = hw_state.clone();
                let on_change = callbacks.on_hardware_changed.clone();
                move |enabled, threads| {
                    let mut st = hw_state.borrow_mut();
                    if enabled && !threads.is_empty() {
                        st.cpu_threads = Some(threads);
                        st.cpu_cores = None;
                    } else {
                        st.cpu_threads = None;
                        st.cpu_cores = None;
                    }
                    on_change(st.clone());
                }
            },
        );
        live_widgets.cpu_live_lbl = Some(live_lbl);
        live_widgets.cpu_bar = Some(bar);
        grid.append(&row_widget);
    }

    // 2. SSD (apenas se o projeto usa muito SSD)
    if needs.show_ssd {
        let init_enabled = hw_state.borrow().ssd_percent.is_some();
        let init_val = hw_state.borrow().ssd_percent.unwrap_or(60) as f64;

        let (row_widget, live_lbl, bar) = build_hw_slider_row(
            "Uso máximo do SSD (%)",
            init_enabled,
            5.0,
            100.0,
            5.0,
            init_val,
            0,
            "%",
            {
                let hw_state = hw_state.clone();
                let on_change = callbacks.on_hardware_changed.clone();
                move |enabled, val| {
                    let mut st = hw_state.borrow_mut();
                    st.ssd_percent = if enabled { Some(val.round() as u32) } else { None };
                    on_change(st.clone());
                }
            },
        );
        live_widgets.ssd_live_lbl = Some(live_lbl);
        live_widgets.ssd_bar = Some(bar);
        grid.append(&row_widget);
    }

    // 3. RAM (apenas se o projeto usa muita RAM)
    if needs.show_ram {
        let max_gb = callbacks.hw_specs.total_ram_gb.max(2.0);
        let default_ram = ((max_gb * 0.5 * 2.0).round() / 2.0).max(1.0);
        let init_enabled = hw_state.borrow().max_ram_gb.is_some();
        let init_val = hw_state.borrow().max_ram_gb.unwrap_or(default_ram);

        let (row_widget, live_lbl, bar) = build_hw_slider_row(
            "Máximo de RAM (GB)",
            init_enabled,
            0.5,
            max_gb,
            0.5,
            init_val,
            1,
            "GB",
            {
                let hw_state = hw_state.clone();
                let on_change = callbacks.on_hardware_changed.clone();
                move |enabled, val| {
                    let mut st = hw_state.borrow_mut();
                    st.max_ram_gb = if enabled {
                        Some((val * 10.0).round() / 10.0)
                    } else {
                        None
                    };
                    on_change(st.clone());
                }
            },
        );
        live_widgets.ram_live_lbl = Some(live_lbl);
        live_widgets.ram_bar = Some(bar);
        grid.append(&row_widget);
    }

    card.append(&grid);
    if let Some(st) = &callbacks.live_hw {
        live_widgets.update(st, &callbacks.hw_specs);
    }

    Some((card.upcast(), live_widgets))
}

#[allow(clippy::too_many_arguments)]
fn build_hw_slider_row<F>(
    title: &str,
    initial_enabled: bool,
    min: f64,
    max: f64,
    step: f64,
    initial_val: f64,
    digits: i32,
    unit: &str,
    on_commit: F,
) -> (gtk4::Widget, gtk4::Label, gtk4::ProgressBar)
where
    F: Fn(bool, f64) + 'static,
{
    let box_row = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    box_row.add_css_class("hw-limit-row");

    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let lbl = gtk4::Label::new(Some(title));
    lbl.add_css_class("caption-heading");
    lbl.set_xalign(0.0);
    lbl.set_hexpand(true);
    top.append(&lbl);

    let live_lbl = gtk4::Label::new(Some("Aguardando leitura..."));
    live_lbl.add_css_class("caption");
    live_lbl.add_css_class("dim-label");
    live_lbl.set_valign(gtk4::Align::Center);
    top.append(&live_lbl);

    let val_lbl = gtk4::Label::new(None);
    val_lbl.add_css_class("mini-pill");
    val_lbl.add_css_class(if initial_enabled { "pill-run" } else { "pill-pend" });
    val_lbl.set_valign(gtk4::Align::Center);
    top.append(&val_lbl);

    let sw = gtk4::Switch::builder()
        .active(initial_enabled)
        .valign(gtk4::Align::Center)
        .tooltip_text("Ativar/desativar limite em tempo real")
        .build();
    top.append(&sw);
    box_row.append(&top);

    let bar = gtk4::ProgressBar::new();
    bar.add_css_class("panorama-linear-bar");
    bar.set_hexpand(true);
    box_row.append(&bar);

    let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, min, max.max(min + step), step);
    scale.set_value(initial_val.clamp(min, max.max(min + step)));
    scale.set_digits(digits);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.set_sensitive(initial_enabled);
    box_row.append(&scale);

    let fmt_val = {
        let unit = unit.to_string();
        move |enabled: bool, v: f64| -> String {
            if !enabled {
                "Sem limite".to_string()
            } else if digits == 0 {
                format!("Limite: {} {}", v.round() as u32, unit)
            } else {
                format!("Limite: {:.1} {}", v, unit)
            }
        }
    };
    val_lbl.set_text(&fmt_val(initial_enabled, scale.value()));

    let on_commit = Rc::new(on_commit);
    let debounce_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    {
        let scale = scale.clone();
        let val_lbl = val_lbl.clone();
        let fmt_val = fmt_val.clone();
        let on_commit = on_commit.clone();
        let debounce_id = debounce_id.clone();
        sw.connect_state_set(move |_, active| {
            scale.set_sensitive(active);
            val_lbl.remove_css_class("pill-run");
            val_lbl.remove_css_class("pill-pend");
            val_lbl.add_css_class(if active { "pill-run" } else { "pill-pend" });
            val_lbl.set_text(&fmt_val(active, scale.value()));
            if let Some(id) = debounce_id.borrow_mut().take() {
                id.remove();
            }
            on_commit(active, scale.value());
            glib::Propagation::Proceed
        });
    }

    {
        let sw = sw.clone();
        let val_lbl = val_lbl.clone();
        let fmt_val = fmt_val.clone();
        let on_commit = on_commit.clone();
        let debounce_id = debounce_id.clone();
        scale.connect_value_changed(move |sc| {
            let active = sw.is_active();
            let v = sc.value();
            val_lbl.set_text(&fmt_val(active, v));
            if !active {
                return;
            }
            if let Some(id) = debounce_id.borrow_mut().take() {
                id.remove();
            }
            let on_commit = on_commit.clone();
            let debounce_id_inner = debounce_id.clone();
            let src_id = glib::timeout_add_local_once(std::time::Duration::from_millis(180), move || {
                *debounce_id_inner.borrow_mut() = None;
                on_commit(true, v);
            });
            *debounce_id.borrow_mut() = Some(src_id);
        });
    }

    (box_row.upcast(), live_lbl, bar)
}

fn build_cpu_topology_row<F>(
    cores: &[CpuCoreInfo],
    total_threads: u32,
    initial_enabled: bool,
    initial_threads: &[u32],
    on_commit: F,
) -> (gtk4::Widget, gtk4::Label, gtk4::ProgressBar)
where
    F: Fn(bool, Vec<u32>) + 'static,
{
    let box_row = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    box_row.add_css_class("hw-limit-row");

    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let title_lbl = gtk4::Label::new(Some(&format!(
        "Núcleos e Threads da CPU ({} núcleos · {} threads)",
        cores.len(),
        total_threads
    )));
    title_lbl.add_css_class("caption-heading");
    title_lbl.set_xalign(0.0);
    title_lbl.set_hexpand(true);
    top.append(&title_lbl);

    let live_lbl = gtk4::Label::new(Some("Aguardando leitura..."));
    live_lbl.add_css_class("caption");
    live_lbl.add_css_class("dim-label");
    live_lbl.set_valign(gtk4::Align::Center);
    top.append(&live_lbl);

    let val_lbl = gtk4::Label::new(None);
    val_lbl.add_css_class("mini-pill");
    val_lbl.add_css_class(if initial_enabled { "pill-run" } else { "pill-pend" });
    val_lbl.set_valign(gtk4::Align::Center);
    top.append(&val_lbl);

    let sw = gtk4::Switch::builder()
        .active(initial_enabled)
        .valign(gtk4::Align::Center)
        .tooltip_text("Ativar/desativar afinidade de núcleos/threads em tempo real")
        .build();
    top.append(&sw);
    box_row.append(&top);

    let bar = gtk4::ProgressBar::new();
    bar.add_css_class("panorama-linear-bar");
    bar.set_hexpand(true);
    box_row.append(&bar);

    let selected: Rc<RefCell<BTreeSet<u32>>> = Rc::new(RefCell::new(
        initial_threads
            .iter()
            .copied()
            .filter(|&t| t < total_threads)
            .collect(),
    ));
    if selected.borrow().is_empty() && !cores.is_empty() {
        let mut s = selected.borrow_mut();
        for &t in &cores[0].threads {
            s.insert(t);
        }
    }

    let cores_vec: Rc<Vec<CpuCoreInfo>> = Rc::new(cores.to_vec());
    let format_summary = {
        let cores_vec = cores_vec.clone();
        move |enabled: bool, sel: &BTreeSet<u32>| -> String {
            if !enabled {
                return "Sem limite".to_string();
            }
            let active_cores = cores_vec
                .iter()
                .filter(|c| c.threads.iter().any(|t| sel.contains(t)))
                .count();
            let list: Vec<u32> = sel.iter().copied().collect();
            let ranges = format_cpu_ranges(&list);
            format!(
                "{}/{} núcleos ({}T) · CPUs {}",
                active_cores,
                cores_vec.len(),
                list.len(),
                ranges
            )
        }
    };
    val_lbl.set_text(&format_summary(initial_enabled, &selected.borrow()));

    let flow = gtk4::FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(6)
        .homogeneous(true)
        .column_spacing(6)
        .row_spacing(6)
        .hexpand(true)
        .build();
    flow.set_sensitive(initial_enabled);

    struct CoreUiGroup {
        core_btn: gtk4::ToggleButton,
        threads: Vec<(u32, gtk4::ToggleButton)>,
    }

    let ui_groups: Rc<RefCell<Vec<CoreUiGroup>>> = Rc::new(RefCell::new(Vec::new()));
    let syncing = Rc::new(Cell::new(false));
    let on_commit = Rc::new(on_commit);
    let debounce_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let refresh_ui_buttons = {
        let ui_groups = ui_groups.clone();
        let cores_vec = cores_vec.clone();
        let selected = selected.clone();
        let syncing = syncing.clone();
        let val_lbl = val_lbl.clone();
        let sw = sw.clone();
        let format_summary = format_summary.clone();
        move || {
            syncing.set(true);
            let sel = selected.borrow();
            let groups = ui_groups.borrow();
            for (core, grp) in cores_vec.iter().zip(groups.iter()) {
                let any_active = core.threads.iter().any(|t| sel.contains(t));
                let all_active = !core.threads.is_empty() && core.threads.iter().all(|t| sel.contains(t));
                grp.core_btn.set_active(any_active);
                if any_active && !all_active {
                    grp.core_btn.add_css_class("cpu-core-partial");
                } else {
                    grp.core_btn.remove_css_class("cpu-core-partial");
                }
                for (tid, tbtn) in &grp.threads {
                    tbtn.set_active(sel.contains(tid));
                }
            }
            val_lbl.set_text(&format_summary(sw.is_active(), &sel));
            syncing.set(false);
        }
    };

    let schedule_commit = {
        let on_commit = on_commit.clone();
        let debounce_id = debounce_id.clone();
        let selected = selected.clone();
        let sw = sw.clone();
        move |immediate: bool| {
            if let Some(id) = debounce_id.borrow_mut().take() {
                id.remove();
            }
            let active = sw.is_active();
            let list: Vec<u32> = selected.borrow().iter().copied().collect();
            if immediate {
                on_commit(active, list);
            } else {
                let on_commit = on_commit.clone();
                let debounce_id_inner = debounce_id.clone();
                let src = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(140),
                    move || {
                        *debounce_id_inner.borrow_mut() = None;
                        on_commit(active, list);
                    },
                );
                *debounce_id.borrow_mut() = Some(src);
            }
        }
    };

    for core in cores {
        let cell = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        cell.add_css_class("cpu-core-card");

        let core_btn = gtk4::ToggleButton::with_label(&format!("Núcleo {}", core.core_id));
        core_btn.add_css_class("cpu-core-btn");
        core_btn.set_hexpand(true);
        let thread_list_txt = core
            .threads
            .iter()
            .map(|t| format!("T{t}"))
            .collect::<Vec<_>>()
            .join(", ");
        core_btn.set_tooltip_text(Some(&format!(
            "Alternar Núcleo {} ({thread_list_txt})",
            core.core_id
        )));
        cell.append(&core_btn);

        let mut thread_btns = Vec::new();
        if core.threads.len() > 1 {
            let trow = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
            trow.set_homogeneous(true);
            for &tid in &core.threads {
                let tb = gtk4::ToggleButton::with_label(&format!("T{tid}"));
                tb.add_css_class("cpu-thread-btn");
                tb.set_tooltip_text(Some(&format!(
                    "Thread lógica {tid} (Núcleo {})",
                    core.core_id
                )));
                {
                    let syncing = syncing.clone();
                    let selected = selected.clone();
                    let refresh_ui_buttons = refresh_ui_buttons.clone();
                    let schedule_commit = schedule_commit.clone();
                    tb.connect_toggled(move |btn| {
                        if syncing.get() {
                            return;
                        }
                        let want = btn.is_active();
                        {
                            let mut sel = selected.borrow_mut();
                            if want {
                                sel.insert(tid);
                            } else if sel.len() > 1 {
                                sel.remove(&tid);
                            }
                        }
                        refresh_ui_buttons();
                        schedule_commit(false);
                    });
                }
                trow.append(&tb);
                thread_btns.push((tid, tb));
            }
            cell.append(&trow);
        }

        {
            let syncing = syncing.clone();
            let selected = selected.clone();
            let core_threads = core.threads.clone();
            let refresh_ui_buttons = refresh_ui_buttons.clone();
            let schedule_commit = schedule_commit.clone();
            core_btn.connect_toggled(move |btn| {
                if syncing.get() {
                    return;
                }
                let want = btn.is_active();
                {
                    let mut sel = selected.borrow_mut();
                    if want {
                        for &t in &core_threads {
                            sel.insert(t);
                        }
                    } else {
                        let remaining = sel.iter().filter(|t| !core_threads.contains(t)).count();
                        if remaining > 0 {
                            for t in &core_threads {
                                sel.remove(t);
                            }
                        }
                    }
                }
                refresh_ui_buttons();
                schedule_commit(false);
            });
        }

        ui_groups.borrow_mut().push(CoreUiGroup {
            core_btn,
            threads: thread_btns,
        });

        let fc = gtk4::FlowBoxChild::builder()
            .focusable(false)
            .child(&cell)
            .build();
        flow.append(&fc);
    }

    refresh_ui_buttons();
    box_row.append(&flow);

    {
        let flow = flow.clone();
        let val_lbl = val_lbl.clone();
        let format_summary = format_summary.clone();
        let selected = selected.clone();
        let schedule_commit = schedule_commit.clone();
        sw.connect_state_set(move |_, active| {
            flow.set_sensitive(active);
            val_lbl.remove_css_class("pill-run");
            val_lbl.remove_css_class("pill-pend");
            val_lbl.add_css_class(if active { "pill-run" } else { "pill-pend" });
            val_lbl.set_text(&format_summary(active, &selected.borrow()));
            schedule_commit(true);
            glib::Propagation::Proceed
        });
    }

    (box_row.upcast(), live_lbl, bar)
}

pub fn resolve_icon_image_path(icon_spec: &str) -> Option<PathBuf> {
    let trimmed = icon_spec.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path_str = if let Some(rest) = trimmed.strip_prefix("file://") {
        rest.to_string()
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            format!("{home}/{rest}")
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };
    let looks_like_path = path_str.contains('/')
        || path_str.ends_with(".png")
        || path_str.ends_with(".jpg")
        || path_str.ends_with(".jpeg")
        || path_str.ends_with(".webp")
        || path_str.ends_with(".svg")
        || path_str.ends_with(".ico");
    if !looks_like_path {
        return None;
    }
    let p = Path::new(&path_str);
    if p.is_file() {
        Some(p.to_path_buf())
    } else {
        None
    }
}

pub fn build_project_icon_widget(icon_spec: &str, size: i32, accent_fallback: bool) -> gtk4::Widget {
    if let Some(img_path) = resolve_icon_image_path(icon_spec) {
        let frame = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        frame.set_size_request(size, size);
        frame.set_halign(gtk4::Align::Center);
        frame.set_valign(gtk4::Align::Center);
        frame.set_overflow(gtk4::Overflow::Hidden);
        frame.add_css_class("project-icon-frame");

        let gfile = gio::File::for_path(&img_path);
        let pic = gtk4::Picture::for_file(&gfile);
        pic.set_content_fit(gtk4::ContentFit::Cover);
        pic.set_can_shrink(true);
        pic.set_size_request(size, size);
        pic.set_hexpand(true);
        pic.set_vexpand(true);
        frame.append(&pic);
        return frame.upcast();
    }

    let name = if icon_spec.trim().is_empty() || icon_spec.contains('/') {
        "applications-engineering-symbolic"
    } else {
        icon_spec.trim()
    };
    let icon = gtk4::Image::from_icon_name(name);
    icon.set_pixel_size(size);
    icon.set_valign(gtk4::Align::Center);
    if accent_fallback {
        icon.add_css_class("accent");
    }
    icon.upcast()
}

fn build_project_banner(doc: &ProjectDocument, on_edit: Rc<dyn Fn()>) -> gtk4::Widget {
    let banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
    banner.add_css_class("card");
    banner.add_css_class("project-banner-card");

    let icon_widget = build_project_icon_widget(&doc.icon, 44, true);
    let icon_btn = gtk4::Button::builder()
        .child(&icon_widget)
        .valign(gtk4::Align::Center)
        .build();
    icon_btn.add_css_class("flat");
    icon_btn.set_tooltip_text(Some("Clique para alterar o nome e a imagem do projeto"));
    {
        let on_edit = on_edit.clone();
        icon_btn.connect_clicked(move |_| {
            on_edit();
        });
    }
    banner.append(&icon_btn);

    let text_col = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    text_col.set_hexpand(true);

    let title_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let title_lbl = gtk4::Label::new(Some(&doc.title));
    title_lbl.add_css_class("title-2");
    title_lbl.set_xalign(0.0);
    title_lbl.set_wrap(true);
    title_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    title_row.append(&title_lbl);

    let badge = gtk4::Label::new(Some(doc.status.display_label()));
    badge.add_css_class("status-pill");
    badge.add_css_class(doc.status.css_class());
    badge.set_valign(gtk4::Align::Center);
    title_row.append(&badge);
    text_col.append(&title_row);

    if !doc.subtitle.trim().is_empty() {
        let sub = gtk4::Label::new(Some(&doc.subtitle));
        sub.set_xalign(0.0);
        sub.set_wrap(true);
        sub.set_wrap_mode(pango::WrapMode::WordChar);
        sub.add_css_class("dim-label");
        text_col.append(&sub);
    }

    let meta_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
    if !doc.path.trim().is_empty() {
        let path_lbl = gtk4::Label::new(Some(&format!("Repositório: {}", doc.path)));
        path_lbl.add_css_class("caption");
        path_lbl.add_css_class("monospace");
        path_lbl.add_css_class("dim-label");
        path_lbl.set_xalign(0.0);
        path_lbl.set_wrap(true);
        path_lbl.set_wrap_mode(pango::WrapMode::WordChar);
        path_lbl.set_selectable(true);
        meta_row.append(&path_lbl);
    }
    if !doc.updated_at.trim().is_empty() {
        let upd = gtk4::Label::new(Some(&format!("Atualizado: {}", doc.updated_at)));
        upd.add_css_class("caption");
        upd.add_css_class("dim-label");
        upd.set_xalign(0.0);
        meta_row.append(&upd);
    }
    text_col.append(&meta_row);

    banner.append(&text_col);

    let edit_content = adw::ButtonContent::builder()
        .icon_name("document-edit-symbolic")
        .label("Editar")
        .build();
    let edit_btn = gtk4::Button::builder()
        .child(&edit_content)
        .valign(gtk4::Align::Center)
        .build();
    edit_btn.add_css_class("flat");
    edit_btn.set_tooltip_text(Some("Alterar nome e imagem de ícone do projeto"));
    edit_btn.connect_clicked(move |_| {
        on_edit();
    });
    banner.append(&edit_btn);

    banner.upcast()
}

fn build_fixed_header_pair(doc: &ProjectDocument, state: &UiViewState) -> gtk4::Widget {
    let flow = gtk4::FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .max_children_per_line(2)
        .min_children_per_line(1)
        .column_spacing(14)
        .row_spacing(14)
        .homogeneous(false)
        .hexpand(true)
        .build();

    let prog_card = build_fixed_progresso_card(doc, state);
    let conf_card = build_fixed_confianca_card(doc, state);

    let child1 = gtk4::FlowBoxChild::builder()
        .focusable(false)
        .hexpand(true)
        .valign(gtk4::Align::Fill)
        .child(&prog_card)
        .build();
    let child2 = gtk4::FlowBoxChild::builder()
        .focusable(false)
        .hexpand(true)
        .valign(gtk4::Align::Fill)
        .child(&conf_card)
        .build();

    flow.append(&child1);
    flow.append(&child2);
    flow.upcast()
}

/// Seção recolhível do cabeçalho fixo; lembra se estava aberta entre recargas.
fn build_header_disclosure(
    state: &UiViewState,
    key: String,
    label: &str,
    content: &gtk4::Widget,
) -> gtk4::Widget {
    let initial = state.expanded_rows.borrow().get(&key).copied().unwrap_or(false);
    let lbl = gtk4::Label::new(Some(label));
    lbl.add_css_class("caption-heading");
    let exp = gtk4::Expander::builder()
        .label_widget(&lbl)
        .expanded(initial)
        .child(content)
        .build();
    content.set_margin_top(6);
    let map = state.expanded_rows.clone();
    exp.connect_expanded_notify(move |e| {
        map.borrow_mut().insert(key.clone(), e.is_expanded());
    });
    exp.upcast()
}

fn build_fixed_progresso_card(doc: &ProjectDocument, state: &UiViewState) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    card.add_css_class("card");
    card.add_css_class("fixed-metric-card");
    card.set_hexpand(true);
    card.set_vexpand(true);

    // Linha título + delta recente
    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let title = gtk4::Label::new(Some("Progresso"));
    title.add_css_class("title-4");
    title.set_xalign(0.0);
    title.set_hexpand(true);
    top.append(&title);

    if let Some(delta) = doc.progresso_delta {
        top.append(&build_delta_badge(delta));
    }
    card.append(&top);

    // Anel circular Cairo + resumo lateral
    let mid = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    let ring = build_ring_gauge(doc.progresso, GaugeKind::Progresso);
    mid.append(&ring);

    let info_col = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    info_col.set_hexpand(true);
    info_col.set_valign(gtk4::Align::Center);

    let phase_txt = if doc.phase.trim().is_empty() {
        "Fase atual: não especificada".to_string()
    } else {
        format!("Fase atual: {}", doc.phase)
    };
    let phase_lbl = gtk4::Label::new(Some(&phase_txt));
    phase_lbl.add_css_class("heading");
    phase_lbl.set_xalign(0.0);
    phase_lbl.set_wrap(true);
    phase_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    info_col.append(&phase_lbl);

    if !doc.progresso_subtitulo.trim().is_empty() {
        let sub = gtk4::Label::new(Some(&doc.progresso_subtitulo));
        sub.add_css_class("caption");
        sub.add_css_class("dim-label");
        sub.set_xalign(0.0);
        sub.set_wrap(true);
        sub.set_wrap_mode(pango::WrapMode::WordChar);
        info_col.append(&sub);
    }

    let pbar = gtk4::ProgressBar::new();
    pbar.set_fraction((doc.progresso / 100.0).clamp(0.0, 1.0));
    pbar.set_hexpand(true);
    pbar.add_css_class("panorama-linear-bar");
    info_col.append(&pbar);

    mid.append(&info_col);
    card.append(&mid);

    if !doc.progresso_calculo.trim().is_empty() {
        card.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        let calc_lbl = gtk4::Label::new(None);
        calc_lbl.set_markup(&inline_rich_markup(&doc.progresso_calculo));
        calc_lbl.set_xalign(0.0);
        calc_lbl.set_wrap(true);
        calc_lbl.set_wrap_mode(pango::WrapMode::WordChar);
        calc_lbl.set_selectable(true);
        card.append(&build_header_disclosure(
            state,
            format!("{}::hdr::calculo", doc.slug),
            "Como esse número é calculado",
            calc_lbl.upcast_ref(),
        ));
    }

    card.upcast()
}

fn build_fixed_confianca_card(doc: &ProjectDocument, state: &UiViewState) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    card.add_css_class("card");
    card.add_css_class("fixed-metric-card");
    card.set_hexpand(true);
    card.set_vexpand(true);

    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let title = gtk4::Label::new(Some("Confiança"));
    title.add_css_class("title-4");
    title.set_xalign(0.0);
    title.set_hexpand(true);
    top.append(&title);

    let band_lbl = if doc.confianca >= 75.0 {
        let l = gtk4::Label::new(Some("Alta"));
        l.add_css_class("mini-pill");
        l.add_css_class("pill-pass");
        l
    } else if doc.confianca >= 45.0 {
        let l = gtk4::Label::new(Some("Moderada"));
        l.add_css_class("mini-pill");
        l.add_css_class("pill-bloc");
        l
    } else {
        let l = gtk4::Label::new(Some("Baixa"));
        l.add_css_class("mini-pill");
        l.add_css_class("pill-fail");
        l
    };
    top.append(&band_lbl);

    if let Some(delta) = doc.confianca_delta {
        top.append(&build_delta_badge(delta));
    }
    card.append(&top);

    let mid = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    let ring = build_ring_gauge(doc.confianca, GaugeKind::Confianca);
    mid.append(&ring);

    let summary_col = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    summary_col.set_hexpand(true);
    summary_col.set_valign(gtk4::Align::Center);

    if !doc.confianca_resumo.trim().is_empty() {
        let res_lbl = gtk4::Label::new(Some(&doc.confianca_resumo));
        res_lbl.set_xalign(0.0);
        res_lbl.set_wrap(true);
        res_lbl.set_wrap_mode(pango::WrapMode::WordChar);
        res_lbl.set_selectable(true);
        summary_col.append(&res_lbl);
    }

    mid.append(&summary_col);
    card.append(&mid);

    // Fatores e riscos ficam recolhidos para o topo não pesar.
    for (items, icon, css, label, key) in [
        (
            &doc.confianca_fatores,
            "object-select-symbolic",
            "text-ok",
            "O que dá segurança",
            "fatores",
        ),
        (
            &doc.confianca_riscos,
            "dialog-warning-symbolic",
            "text-warn",
            "O que ainda preocupa",
            "riscos",
        ),
    ] {
        if items.is_empty() {
            continue;
        }
        let list = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        for item in items {
            list.append(&build_icon_bullet_row(icon, css, item));
        }
        card.append(&build_header_disclosure(
            state,
            format!("{}::hdr::{key}", doc.slug),
            &format!("{label} ({})", items.len()),
            list.upcast_ref(),
        ));
    }

    if !doc.history.is_empty() {
        card.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        card.append(&build_sparkline(&doc.history));
    }

    card.upcast()
}

fn build_delta_badge(delta: f64) -> gtk4::Label {
    let txt = if delta > 0.0 {
        format!("+{delta:.1}%")
    } else {
        format!("{delta:.1}%")
    };
    let lbl = gtk4::Label::new(Some(&txt));
    lbl.add_css_class("mini-pill");
    if delta > 0.04 {
        lbl.add_css_class("pill-pass");
    } else if delta < -0.04 {
        lbl.add_css_class("pill-fail");
    } else {
        lbl.add_css_class("pill-pend");
    }
    lbl
}

fn build_icon_bullet_row(icon_name: &str, icon_css: &str, text: &str) -> gtk4::Widget {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let ic = gtk4::Image::from_icon_name(icon_name);
    ic.set_pixel_size(14);
    ic.set_valign(gtk4::Align::Start);
    ic.set_margin_top(3);
    ic.add_css_class(icon_css);

    let lbl = gtk4::Label::new(None);
    lbl.set_markup(&inline_rich_markup(text));
    lbl.set_xalign(0.0);
    lbl.set_hexpand(true);
    lbl.set_wrap(true);
    lbl.set_wrap_mode(pango::WrapMode::WordChar);
    lbl.add_css_class("caption");
    lbl.set_selectable(true);

    row.append(&ic);
    row.append(&lbl);
    row.upcast()
}

fn render_ui_node(
    node: &UiNode,
    slug: &str,
    path_key: &str,
    state: &UiViewState,
) -> gtk4::Widget {
    match node {
        UiNode::Tabs { id, tabs } => {
            let stack_key = format!(
                "{slug}::tabs::{}",
                id.as_deref().unwrap_or(path_key)
            );
            let wrapper = gtk4::Box::new(gtk4::Orientation::Vertical, 12);

            let switcher_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            switcher_row.add_css_class("panorama-tab-bar");

            let view_stack = adw::ViewStack::new();
            view_stack.set_vhomogeneous(false);
            view_stack.set_hhomogeneous(false);

            let saved_tab = state.active_tabs.borrow().get(&stack_key).cloned();
            let mut first_id: Option<String> = None;
            let mut buttons: Vec<(String, gtk4::ToggleButton)> = Vec::new();

            for (t_idx, tab) in tabs.iter().enumerate() {
                let tab_id = if tab.id.is_empty() {
                    format!("tab_{t_idx}")
                } else {
                    tab.id.clone()
                };
                if first_id.is_none() {
                    first_id = Some(tab_id.clone());
                }

                let page_box = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
                page_box.set_margin_top(4);
                for (c_idx, child) in tab.children.iter().enumerate() {
                    let child_key = format!("{path_key}::tab[{tab_id}]::c[{c_idx}]");
                    page_box.append(&render_ui_node(child, slug, &child_key, state));
                }

                view_stack.add_titled(&page_box, Some(&tab_id), &tab.title);

                let btn_content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                if let Some(ic_name) = tab.icon.as_deref().filter(|s| !s.is_empty()) {
                    let img = gtk4::Image::from_icon_name(ic_name);
                    img.set_pixel_size(15);
                    btn_content.append(&img);
                }
                let btn_lbl = gtk4::Label::new(Some(&tab.title));
                btn_lbl.add_css_class("heading");
                btn_content.append(&btn_lbl);

                let btn = gtk4::ToggleButton::builder().child(&btn_content).build();
                btn.add_css_class("panorama-tab-btn");
                if let Some((_, first_btn)) = buttons.first() {
                    btn.set_group(Some(first_btn));
                }
                buttons.push((tab_id, btn));
            }

            let target_id = saved_tab
                .filter(|saved| buttons.iter().any(|(id, _)| id == saved))
                .or(first_id);

            for (tab_id, btn) in &buttons {
                if target_id.as_deref() == Some(tab_id.as_str()) {
                    btn.set_active(true);
                    view_stack.set_visible_child_name(tab_id);
                }
                let vs = view_stack.clone();
                let tid = tab_id.clone();
                let skey = stack_key.clone();
                let tabs_map = state.active_tabs.clone();
                btn.connect_toggled(move |b| {
                    if b.is_active() {
                        vs.set_visible_child_name(&tid);
                        tabs_map.borrow_mut().insert(skey.clone(), tid.clone());
                    }
                });
                switcher_row.append(btn);
            }

            let switcher_scroll = gtk4::ScrolledWindow::builder()
                .hscrollbar_policy(gtk4::PolicyType::Automatic)
                .vscrollbar_policy(gtk4::PolicyType::Never)
                .hexpand(true)
                .child(&switcher_row)
                .build();

            wrapper.append(&switcher_scroll);
            wrapper.append(&view_stack);
            wrapper.upcast()
        }

        UiNode::Section {
            title,
            description,
            children,
        } => {
            let group = adw::PreferencesGroup::new();
            group.set_title(&glib::markup_escape_text(title));
            if let Some(desc) = description.as_deref().filter(|s| !s.trim().is_empty()) {
                group.set_description(Some(&glib::markup_escape_text(desc)));
            }
            let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
            for (i, ch) in children.iter().enumerate() {
                let ckey = format!("{path_key}::sec[{i}]");
                inner.append(&render_ui_node(ch, slug, &ckey, state));
            }
            group.add(&inner);
            group.upcast()
        }

        UiNode::Columns { columns, children } => {
            let max_cols = columns.unwrap_or_else(|| children.len().clamp(1, 3) as u32).max(1);
            let flow = gtk4::FlowBox::builder()
                .selection_mode(gtk4::SelectionMode::None)
                .max_children_per_line(max_cols)
                .min_children_per_line(1)
                .column_spacing(12)
                .row_spacing(12)
                .homogeneous(false)
                .hexpand(true)
                .build();
            for (i, ch) in children.iter().enumerate() {
                let ckey = format!("{path_key}::col[{i}]");
                let w = render_ui_node(ch, slug, &ckey, state);
                w.set_hexpand(true);
                let fchild = gtk4::FlowBoxChild::builder()
                    .focusable(false)
                    .hexpand(true)
                    .valign(gtk4::Align::Fill)
                    .child(&w)
                    .build();
                flow.append(&fchild);
            }
            flow.upcast()
        }

        UiNode::Prose {
            title,
            body,
            bullets,
            tone,
        } => {
            let card = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
            card.add_css_class("card");
            card.add_css_class("panorama-block-card");
            if let Some(t) = tone {
                match t {
                    Tone::Info => card.add_css_class("prose-info"),
                    Tone::Success => card.add_css_class("prose-success"),
                    Tone::Warning => card.add_css_class("prose-warning"),
                    Tone::Danger => card.add_css_class("prose-danger"),
                    Tone::Neutral => {}
                }
            }

            if let Some(t) = title.as_deref().filter(|s| !s.trim().is_empty()) {
                let hdr = gtk4::Label::new(Some(t));
                hdr.add_css_class("heading");
                hdr.set_xalign(0.0);
                hdr.set_wrap(true);
                hdr.set_wrap_mode(pango::WrapMode::WordChar);
                card.append(&hdr);
            }

            append_rich_prose_body(&card, body);

            for b in bullets {
                card.append(&build_plain_bullet(b));
            }
            card.upcast()
        }

        UiNode::Alert {
            level,
            title,
            message,
        } => {
            let banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
            banner.add_css_class("card");
            banner.add_css_class("panorama-block-card");
            banner.add_css_class(level.css_class());

            let icon = gtk4::Image::from_icon_name(level.icon_name());
            icon.set_pixel_size(22);
            icon.set_valign(gtk4::Align::Start);
            icon.set_margin_top(2);
            banner.append(&icon);

            let col = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            col.set_hexpand(true);
            if !title.trim().is_empty() {
                let t_lbl = gtk4::Label::new(Some(title));
                t_lbl.add_css_class("heading");
                t_lbl.set_xalign(0.0);
                t_lbl.set_wrap(true);
                t_lbl.set_wrap_mode(pango::WrapMode::WordChar);
                col.append(&t_lbl);
            }
            if !message.trim().is_empty() {
                let m_lbl = gtk4::Label::new(None);
                m_lbl.set_markup(&inline_rich_markup(message));
                m_lbl.set_xalign(0.0);
                m_lbl.set_wrap(true);
                m_lbl.set_wrap_mode(pango::WrapMode::WordChar);
                m_lbl.set_selectable(true);
                col.append(&m_lbl);
            }
            banner.append(&col);
            banner.upcast()
        }

        UiNode::KpiGrid { items } => {
            let max_c = items.len().clamp(1, 4) as u32;
            let flow = gtk4::FlowBox::builder()
                .selection_mode(gtk4::SelectionMode::None)
                .max_children_per_line(max_c)
                .min_children_per_line(1)
                .column_spacing(10)
                .row_spacing(10)
                .homogeneous(false)
                .hexpand(true)
                .build();

            for item in items {
                let tile = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
                tile.add_css_class("card");
                tile.add_css_class("kpi-card");
                tile.add_css_class(item.status.css_class());
                tile.set_hexpand(true);

                let l_lbl = gtk4::Label::new(Some(&item.label));
                l_lbl.add_css_class("caption-heading");
                l_lbl.add_css_class("dim-label");
                l_lbl.set_xalign(0.0);
                l_lbl.set_wrap(true);
                l_lbl.set_wrap_mode(pango::WrapMode::WordChar);
                tile.append(&l_lbl);

                let v_lbl = gtk4::Label::new(Some(&item.value));
                v_lbl.add_css_class("title-3");
                v_lbl.set_xalign(0.0);
                v_lbl.set_wrap(true);
                v_lbl.set_wrap_mode(pango::WrapMode::WordChar);
                v_lbl.set_selectable(true);
                tile.append(&v_lbl);

                if !item.sublabel.trim().is_empty() {
                    let s_lbl = gtk4::Label::new(Some(&item.sublabel));
                    s_lbl.add_css_class("caption");
                    s_lbl.add_css_class("dim-label");
                    s_lbl.set_xalign(0.0);
                    s_lbl.set_wrap(true);
                    s_lbl.set_wrap_mode(pango::WrapMode::WordChar);
                    tile.append(&s_lbl);
                }

                let fchild = gtk4::FlowBoxChild::builder()
                    .focusable(false)
                    .hexpand(true)
                    .valign(gtk4::Align::Fill)
                    .child(&tile)
                    .build();
                flow.append(&fchild);
            }
            flow.upcast()
        }

        UiNode::ProgressList { title, items } => {
            let card = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
            card.add_css_class("card");
            card.add_css_class("panorama-block-card");

            if let Some(t) = title.as_deref().filter(|s| !s.trim().is_empty()) {
                let hdr = gtk4::Label::new(Some(t));
                hdr.add_css_class("heading");
                hdr.set_xalign(0.0);
                hdr.set_wrap(true);
                hdr.set_wrap_mode(pango::WrapMode::WordChar);
                card.append(&hdr);
            }

            for (idx, it) in items.iter().enumerate() {
                if idx > 0 {
                    card.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
                }
                let item_box = gtk4::Box::new(gtk4::Orientation::Vertical, 5);
                let row_top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                let name_lbl = gtk4::Label::new(Some(&it.label));
                name_lbl.add_css_class("heading");
                name_lbl.set_xalign(0.0);
                name_lbl.set_hexpand(true);
                name_lbl.set_wrap(true);
                name_lbl.set_wrap_mode(pango::WrapMode::WordChar);
                row_top.append(&name_lbl);

                let pct_lbl = gtk4::Label::new(Some(&format!("{:.1}%", it.percent)));
                pct_lbl.add_css_class("mini-pill");
                pct_lbl.add_css_class(match it.status {
                    KpiStatus::Ok => "pill-pass",
                    KpiStatus::Warn => "pill-bloc",
                    KpiStatus::Err => "pill-fail",
                    KpiStatus::Neutral => "pill-run",
                });
                row_top.append(&pct_lbl);
                item_box.append(&row_top);

                let bar = gtk4::ProgressBar::new();
                bar.set_fraction((it.percent / 100.0).clamp(0.0, 1.0));
                bar.add_css_class(match it.status {
                    KpiStatus::Ok => "bar-ok",
                    KpiStatus::Warn => "bar-warn",
                    KpiStatus::Err => "bar-err",
                    KpiStatus::Neutral => "panorama-linear-bar",
                });
                item_box.append(&bar);

                if !it.detail.trim().is_empty() {
                    let det = gtk4::Label::new(None);
                    det.set_markup(&inline_rich_markup(&it.detail));
                    det.add_css_class("caption");
                    det.add_css_class("dim-label");
                    det.set_xalign(0.0);
                    det.set_wrap(true);
                    det.set_wrap_mode(pango::WrapMode::WordChar);
                    det.set_selectable(true);
                    item_box.append(&det);
                }
                card.append(&item_box);
            }
            card.upcast()
        }

        UiNode::Expander {
            title,
            subtitle,
            icon,
            badge,
            status,
            expanded,
            children,
        } => {
            let list = gtk4::ListBox::new();
            list.set_selection_mode(gtk4::SelectionMode::None);
            list.add_css_class("boxed-list");

            let exp_key = format!("{path_key}::exp::{title}");
            let init_expanded = state
                .expanded_rows
                .borrow()
                .get(&exp_key)
                .copied()
                .unwrap_or(*expanded);

            let exp_row = adw::ExpanderRow::builder()
                .title(glib::markup_escape_text(title))
                .subtitle(glib::markup_escape_text(subtitle))
                .expanded(init_expanded)
                .enable_expansion(true)
                .build();

            if let Some(ic) = icon.as_deref().filter(|s| !s.trim().is_empty()) {
                let img = gtk4::Image::from_icon_name(ic);
                img.set_pixel_size(20);
                img.add_css_class("expander-icon");
                img.add_css_class(match status {
                    Some(KpiStatus::Ok) => "icon-ok",
                    Some(KpiStatus::Warn) => "icon-warn",
                    Some(KpiStatus::Err) => "icon-err",
                    _ => "icon-neutral",
                });
                exp_row.add_prefix(&img);
            }

            if let Some(b_txt) = badge.as_deref().filter(|s| !s.trim().is_empty()) {
                let b_lbl = gtk4::Label::new(Some(b_txt));
                b_lbl.set_valign(gtk4::Align::Center);
                b_lbl.add_css_class("mini-pill");
                b_lbl.add_css_class(match status.unwrap_or(KpiStatus::Neutral) {
                    KpiStatus::Ok => "pill-pass",
                    KpiStatus::Warn => "pill-bloc",
                    KpiStatus::Err => "pill-fail",
                    KpiStatus::Neutral => "pill-run",
                });
                exp_row.add_suffix(&b_lbl);
            }

            let exp_map = state.expanded_rows.clone();
            let exp_key_notify = exp_key.clone();
            exp_row.connect_expanded_notify(move |r| {
                exp_map
                    .borrow_mut()
                    .insert(exp_key_notify.clone(), r.is_expanded());
            });

            if !children.is_empty() {
                let nested_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
                nested_box.set_margin_top(12);
                nested_box.set_margin_bottom(12);
                nested_box.set_margin_start(12);
                nested_box.set_margin_end(12);

                for (i, ch) in children.iter().enumerate() {
                    let ckey = format!("{path_key}::expchild[{i}]");
                    nested_box.append(&render_ui_node(ch, slug, &ckey, state));
                }

                let holder_row = gtk4::ListBoxRow::builder()
                    .activatable(false)
                    .selectable(false)
                    .focusable(false)
                    .child(&nested_box)
                    .build();
                exp_row.add_row(&holder_row);
            }

            list.append(&exp_row);
            list.upcast()
        }

        UiNode::Matrix {
            title,
            subtitle,
            columns,
            cells,
        } => build_matrix_widget(title.as_deref(), subtitle.as_deref(), *columns, cells),

        UiNode::FlowGraph {
            title,
            subtitle,
            nodes,
            edges,
        } => build_flow_graph_widget(title.as_deref(), subtitle.as_deref(), nodes, edges),

        UiNode::Table {
            title,
            columns,
            rows,
        } => build_table_widget(title.as_deref(), columns, rows),

        UiNode::Timeline { title, items } => build_timeline_widget(title.as_deref(), items),

        UiNode::Donut {
            title,
            subtitle,
            center_value,
            center_label,
            slices,
        } => build_donut_widget(
            title.as_deref(),
            subtitle.as_deref(),
            center_value.as_deref(),
            center_label.as_deref(),
            slices,
        ),

        UiNode::BarChart {
            title,
            subtitle,
            unit,
            max,
            bars,
        } => build_bar_chart_widget(
            title.as_deref(),
            subtitle.as_deref(),
            unit.as_deref(),
            *max,
            bars,
        ),

        UiNode::Steps {
            title,
            subtitle,
            first_number,
            steps,
        } => build_steps_widget(
            title.as_deref(),
            subtitle.as_deref(),
            first_number.unwrap_or(1),
            steps,
        ),

        UiNode::Cards { columns, items } => build_cards_widget(*columns, items),

        UiNode::Glossary { title, items } => build_glossary_widget(title.as_deref(), items),
    }
}

fn wrapped_label(text: &str, css: &[&str]) -> gtk4::Label {
    let l = gtk4::Label::new(None);
    l.set_markup(&inline_rich_markup(text));
    l.set_xalign(0.0);
    l.set_wrap(true);
    l.set_wrap_mode(pango::WrapMode::WordChar);
    for c in css {
        l.add_css_class(c);
    }
    l
}

fn tone_css(tone: Option<Tone>) -> &'static str {
    match tone.unwrap_or(Tone::Info) {
        Tone::Info => "tone-info",
        Tone::Success => "tone-success",
        Tone::Warning => "tone-warning",
        Tone::Danger => "tone-danger",
        Tone::Neutral => "tone-neutral",
    }
}

/// Jornada em etapas: círculos numerados ligados por uma linha; clique abre a explicação.
fn build_steps_widget(
    title: Option<&str>,
    subtitle: Option<&str>,
    first_number: usize,
    steps: &[StepItem],
) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");
    if let Some(t) = title.filter(|s| !s.trim().is_empty()) {
        card.append(&wrapped_label(t, &["heading"]));
    }
    if let Some(st) = subtitle.filter(|s| !s.trim().is_empty()) {
        card.append(&wrapped_label(st, &["caption", "dim-label"]));
    }

    let grid = gtk4::Grid::builder()
        .column_homogeneous(true)
        .row_spacing(4)
        .hexpand(true)
        .build();
    let n = steps.len();
    for (i, step) in steps.iter().enumerate() {
        let col = i as i32;
        let prev_done = i > 0 && steps[i - 1].state == CellState::Pass;
        let number = first_number + i;
        let marker = build_step_marker(number, step.state, i > 0, i + 1 < n, prev_done);
        grid.attach(&marker, col, 0, 1, 1);

        let name = gtk4::Label::new(Some(&step.title));
        name.set_wrap(true);
        name.set_wrap_mode(pango::WrapMode::WordChar);
        name.set_justify(gtk4::Justification::Center);
        name.set_max_width_chars(14);
        name.set_margin_start(4);
        name.set_margin_end(4);
        name.add_css_class("caption-heading");
        grid.attach(&name, col, 1, 1, 1);

        let st = gtk4::Label::new(Some(step.state.label_pt()));
        st.add_css_class("caption");
        st.add_css_class("dim-label");
        grid.attach(&st, col, 2, 1, 1);

        if !step.description.trim().is_empty() {
            let pop_lbl = wrapped_label(&step.description, &[]);
            pop_lbl.set_max_width_chars(44);
            pop_lbl.set_selectable(true);
            let pop_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
            pop_box.set_margin_top(10);
            pop_box.set_margin_bottom(10);
            pop_box.set_margin_start(12);
            pop_box.set_margin_end(12);
            pop_box.append(&wrapped_label(&format!("{number}. {}", step.title), &["heading"]));
            pop_box.append(&wrapped_label(step.state.label_pt(), &["caption-heading", "dim-label"]));
            pop_box.append(&pop_lbl);
            let popover = gtk4::Popover::builder().child(&pop_box).build();
            popover.set_parent(&marker);
            marker.connect_destroy({
                let popover = popover.clone();
                move |_| popover.unparent()
            });
            marker.set_cursor_from_name(Some("pointer"));
            marker.set_tooltip_text(Some("Clique para ver o que esta etapa inclui"));
            let click = gtk4::GestureClick::new();
            click.connect_released(move |_, _, _, _| popover.popup());
            marker.add_controller(click);
        }
    }

    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&grid)
        .build();
    grid.set_size_request((n as i32) * 96, -1);
    card.append(&scroller);
    card.upcast()
}

/// Cartões com ícone grande, título e texto curto.
fn build_cards_widget(columns: Option<u32>, items: &[CardItem]) -> gtk4::Widget {
    let max_c = columns.unwrap_or_else(|| items.len().clamp(1, 3) as u32).max(1);
    let flow = gtk4::FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .max_children_per_line(max_c)
        .min_children_per_line(1)
        .column_spacing(12)
        .row_spacing(12)
        .homogeneous(true)
        .hexpand(true)
        .build();
    for item in items {
        let tile = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        tile.add_css_class("card");
        tile.add_css_class("icon-card");

        let badge = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        badge.add_css_class("icon-badge");
        badge.add_css_class(tone_css(item.tone));
        badge.set_halign(gtk4::Align::Start);
        let img = gtk4::Image::from_icon_name(
            item.icon.as_deref().filter(|s| !s.is_empty()).unwrap_or("dialog-information-symbolic"),
        );
        img.set_pixel_size(24);
        badge.append(&img);
        tile.append(&badge);

        // Largura natural limitada para os cartões caberem lado a lado.
        let title_lbl = wrapped_label(&item.title, &["heading"]);
        title_lbl.set_max_width_chars(30);
        tile.append(&title_lbl);
        if !item.text.trim().is_empty() {
            let text_lbl = wrapped_label(&item.text, &["dim-label"]);
            text_lbl.set_max_width_chars(30);
            tile.append(&text_lbl);
        }
        tile.set_hexpand(true);
        let fchild = gtk4::FlowBoxChild::builder()
            .focusable(false)
            .hexpand(true)
            .valign(gtk4::Align::Fill)
            .child(&tile)
            .build();
        flow.append(&fchild);
    }
    flow.upcast()
}

/// Glossário: termos que aparecem no panorama explicados em linguagem simples.
fn build_glossary_widget(title: Option<&str>, items: &[GlossaryItem]) -> gtk4::Widget {
    let group = adw::PreferencesGroup::new();
    group.set_title(&glib::markup_escape_text(title.unwrap_or("Glossário")));
    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");
    for it in items {
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&it.term))
            .subtitle(glib::markup_escape_text(&it.meaning))
            .subtitle_lines(0)
            .activatable(false)
            .build();
        let img = gtk4::Image::from_icon_name("accessories-dictionary-symbolic");
        img.add_css_class("dim-label");
        row.add_prefix(&img);
        list.append(&row);
    }
    group.add(&list);
    group.upcast()
}

fn build_table_widget(
    title: Option<&str>,
    columns: &[String],
    rows: &[Vec<String>],
) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");

    if let Some(t) = title.filter(|s| !s.trim().is_empty()) {
        let hdr = gtk4::Label::new(Some(t));
        hdr.add_css_class("heading");
        hdr.set_xalign(0.0);
        hdr.set_wrap(true);
        hdr.set_wrap_mode(pango::WrapMode::WordChar);
        card.append(&hdr);
    }

    let grid = gtk4::Grid::builder()
        .column_spacing(14)
        .row_spacing(8)
        .hexpand(true)
        .build();

    let ncols = columns.len().max(1) as i32;
    for (col_idx, col_title) in columns.iter().enumerate() {
        let lbl = gtk4::Label::new(Some(col_title));
        lbl.add_css_class("caption-heading");
        lbl.add_css_class("table-header-cell");
        lbl.set_xalign(0.0);
        lbl.set_hexpand(true);
        lbl.set_wrap(true);
        lbl.set_wrap_mode(pango::WrapMode::WordChar);
        grid.attach(&lbl, col_idx as i32, 0, 1, 1);
    }

    let sep_head = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    grid.attach(&sep_head, 0, 1, ncols, 1);

    for (r_idx, row) in rows.iter().enumerate() {
        let grid_row = 2 + (r_idx as i32) * 2;
        for (c_idx, cell_text) in row.iter().enumerate() {
            let cell_lbl = gtk4::Label::new(None);
            cell_lbl.set_markup(&inline_rich_markup(cell_text));
            cell_lbl.set_xalign(0.0);
            cell_lbl.set_valign(gtk4::Align::Start);
            cell_lbl.set_hexpand(c_idx + 1 == columns.len() || columns.len() <= 3);
            cell_lbl.set_wrap(true);
            cell_lbl.set_wrap_mode(pango::WrapMode::WordChar);
            cell_lbl.set_selectable(true);
            if c_idx == 0 {
                cell_lbl.add_css_class("table-first-col");
            }
            grid.attach(&cell_lbl, c_idx as i32, grid_row, 1, 1);
        }
        if r_idx + 1 < rows.len() {
            let row_sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
            row_sep.add_css_class("spacer");
            grid.attach(&row_sep, 0, grid_row + 1, ncols, 1);
        }
    }

    card.append(&grid);
    card.upcast()
}

fn build_timeline_widget(
    title: Option<&str>,
    items: &[crate::model::TimelineItem],
) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");

    if let Some(t) = title.filter(|s| !s.trim().is_empty()) {
        let hdr = gtk4::Label::new(Some(t));
        hdr.add_css_class("heading");
        hdr.set_xalign(0.0);
        hdr.set_wrap(true);
        hdr.set_wrap_mode(pango::WrapMode::WordChar);
        card.append(&hdr);
    }

    let list_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let total = items.len();

    for (idx, item) in items.iter().enumerate() {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);

        // Coluna vetorial Cairo com ponto + linha conectora vertical
        let state = item.state;
        let is_last = idx + 1 == total;
        let rail = gtk4::DrawingArea::builder()
            .content_width(22)
            .content_height(46)
            .vexpand(true)
            .build();
        let style_mgr = adw::StyleManager::default();
        style_mgr.connect_dark_notify(glib::clone!(
            #[weak]
            rail,
            move |_| {
                rail.queue_draw();
            }
        ));
        rail.set_draw_func(move |_, cr, w, h| {
            let dark = adw::StyleManager::default().is_dark();
            let cx = (w as f64) * 0.5;
            let (sr, sg, sb) = state.rgb(dark);

            if !is_last {
                if dark {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.16);
                } else {
                    cr.set_source_rgba(0.0, 0.0, 0.0, 0.14);
                }
                cr.set_line_width(2.0);
                cr.move_to(cx, 14.0);
                cr.line_to(cx, h as f64);
                let _ = cr.stroke();
            }

            cr.set_source_rgba(sr, sg, sb, 0.24);
            cr.arc(cx, 12.0, 8.0, 0.0, TAU);
            let _ = cr.fill();

            cr.set_source_rgb(sr, sg, sb);
            cr.arc(cx, 12.0, 4.5, 0.0, TAU);
            let _ = cr.fill();
        });
        row.append(&rail);

        let body_col = gtk4::Box::new(gtk4::Orientation::Vertical, 3);
        body_col.set_hexpand(true);
        body_col.set_margin_bottom(if is_last { 2 } else { 12 });

        let head_line = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let ts_lbl = gtk4::Label::new(Some(&item.timestamp));
        ts_lbl.add_css_class("caption");
        ts_lbl.add_css_class("monospace");
        ts_lbl.add_css_class("mini-pill");
        ts_lbl.add_css_class("pill-pend");
        head_line.append(&ts_lbl);

        let st_lbl = gtk4::Label::new(Some(item.state.short_tag()));
        st_lbl.add_css_class("mini-pill");
        st_lbl.add_css_class(match item.state {
            crate::model::CellState::Pass => "pill-pass",
            crate::model::CellState::Running => "pill-run",
            crate::model::CellState::Fail => "pill-fail",
            crate::model::CellState::Blocked => "pill-bloc",
            crate::model::CellState::Pending => "pill-pend",
            crate::model::CellState::Reserved => "pill-resv",
        });
        head_line.append(&st_lbl);

        let t_lbl = gtk4::Label::new(Some(&item.title));
        t_lbl.add_css_class("heading");
        t_lbl.set_xalign(0.0);
        t_lbl.set_hexpand(true);
        t_lbl.set_wrap(true);
        t_lbl.set_wrap_mode(pango::WrapMode::WordChar);
        head_line.append(&t_lbl);
        body_col.append(&head_line);

        if !item.detail.trim().is_empty() {
            let d_lbl = gtk4::Label::new(None);
            d_lbl.set_markup(&inline_rich_markup(&item.detail));
            d_lbl.set_xalign(0.0);
            d_lbl.set_wrap(true);
            d_lbl.set_wrap_mode(pango::WrapMode::WordChar);
            d_lbl.add_css_class("dim-label");
            d_lbl.set_selectable(true);
            body_col.append(&d_lbl);
        }

        row.append(&body_col);
        list_box.append(&row);
    }

    card.append(&list_box);
    card.upcast()
}

fn append_rich_prose_body(container: &gtk4::Box, body: &str) {
    if body.trim().is_empty() {
        return;
    }
    let mut para_buf = Vec::<&str>::new();
    let flush_para = |buf: &mut Vec<&str>, target: &gtk4::Box| {
        if buf.is_empty() {
            return;
        }
        let joined = buf.join(" ");
        let lbl = gtk4::Label::new(None);
        lbl.set_markup(&inline_rich_markup(&joined));
        lbl.set_xalign(0.0);
        lbl.set_wrap(true);
        lbl.set_wrap_mode(pango::WrapMode::WordChar);
        lbl.set_selectable(true);
        target.append(&lbl);
        buf.clear();
    };

    for raw_line in body.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            flush_para(&mut para_buf, container);
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("• "))
            .or_else(|| trimmed.strip_prefix("* "))
        {
            flush_para(&mut para_buf, container);
            container.append(&build_plain_bullet(rest));
        } else {
            para_buf.push(trimmed);
        }
    }
    flush_para(&mut para_buf, container);
}

fn build_plain_bullet(text: &str) -> gtk4::Widget {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let dot = gtk4::Label::new(Some("•"));
    dot.add_css_class("heading");
    dot.add_css_class("accent");
    dot.set_valign(gtk4::Align::Start);

    let lbl = gtk4::Label::new(None);
    lbl.set_markup(&inline_rich_markup(text));
    lbl.set_xalign(0.0);
    lbl.set_hexpand(true);
    lbl.set_wrap(true);
    lbl.set_wrap_mode(pango::WrapMode::WordChar);
    lbl.set_selectable(true);

    row.append(&dot);
    row.append(&lbl);
    row.upcast()
}

/// Converte destaques leves (`` `codigo` `` e `**negrito**`) em Pango Markup escapado com segurança.
pub fn inline_rich_markup(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            if let Some(end_rel) = chars[i + 1..].iter().position(|&c| c == '`') {
                let end = i + 1 + end_rel;
                if end > i + 1 {
                    let code_str: String = chars[i + 1..end].iter().collect();
                    out.push_str("<tt>");
                    out.push_str(&glib::markup_escape_text(&code_str));
                    out.push_str("</tt>");
                    i = end + 1;
                    continue;
                }
            }
        }
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            let rest = &chars[i + 2..];
            if let Some(end_rel) = rest
                .windows(2)
                .position(|w| w[0] == '*' && w[1] == '*')
            {
                if end_rel > 0 {
                    let bold_str: String = rest[..end_rel].iter().collect();
                    out.push_str("<b>");
                    out.push_str(&glib::markup_escape_text(&bold_str));
                    out.push_str("</b>");
                    i += 2 + end_rel + 2;
                    continue;
                }
            }
        }
        match chars[i] {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            ch => out.push(ch),
        }
        i += 1;
    }
    out
}


fn harness_display_label(id: &str) -> &str {
    match id {
        "antigravity" => "Antigravity",
        "codex" => "Codex",
        "claude" => "Claude Code",
        "hermes" => "Hermes Agent",
        other => other,
    }
}

pub fn merge_catalog_with_saved(
    catalog: &TraycerCatalog,
    saved_harness: &str,
    saved_model: &str,
    saved_effort: Option<&str>,
) -> Vec<crate::ipc::TraycerHarnessInfo> {
    let mut harnesses = catalog.harnesses.clone();
    let saved_h = saved_harness.trim();
    let saved_m = saved_model.trim();
    let fallback_efforts: Vec<String> = saved_effort
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| vec![s.to_string()])
        .unwrap_or_default();

    if !saved_h.is_empty() {
        if let Some(existing) = harnesses.iter_mut().find(|h| h.id == saved_h) {
            if !saved_m.is_empty() && !existing.models.iter().any(|m| m.id == saved_m) {
                existing.models.push(crate::ipc::TraycerModelInfo {
                    id: saved_m.to_string(),
                    reasoning_efforts: fallback_efforts,
                    fast_mode_available: false,
                });
            }
        } else {
            harnesses.push(crate::ipc::TraycerHarnessInfo {
                id: saved_h.to_string(),
                label: harness_display_label(saved_h).to_string(),
                models: if saved_m.is_empty() {
                    vec![]
                } else {
                    vec![crate::ipc::TraycerModelInfo {
                        id: saved_m.to_string(),
                        reasoning_efforts: fallback_efforts,
                        fast_mode_available: false,
                    }]
                },
            });
        }
    }

    if harnesses.is_empty() {
        harnesses.push(crate::ipc::TraycerHarnessInfo {
            id: "antigravity".to_string(),
            label: "Antigravity".to_string(),
            models: vec![],
        });
    }

    crate::traycer_bridge::sort_harnesses(&mut harnesses);
    harnesses
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_markup_escapes_xml_and_formats_code_and_bold() {
        let m = inline_rich_markup("Teste **A & B** em `crates/ntm-core` <ok>");
        assert_eq!(
            m,
            "Teste <b>A &amp; B</b> em <tt>crates/ntm-core</tt> &lt;ok&gt;"
        );
    }

    #[test]
    fn merge_catalog_preserves_saved_antigravity_when_catalog_only_has_hermes() {
        let partial = TraycerCatalog {
            harnesses: vec![crate::ipc::TraycerHarnessInfo {
                id: "hermes".into(),
                label: "Hermes Agent".into(),
                models: vec![crate::ipc::TraycerModelInfo {
                    id: "antigravity-subscription-directsdk:gemini-3-flash".into(),
                    reasoning_efforts: vec![],
                    fast_mode_available: false,
                }],
            }],
        };
        let merged = merge_catalog_with_saved(
            &partial,
            "antigravity",
            "gemini-4-hydrogen",
            Some("high"),
        );
        let ids: Vec<_> = merged.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["antigravity", "hermes"]);
        assert_eq!(merged[0].models[0].id, "gemini-4-hydrogen");
        assert_eq!(merged[0].models[0].reasoning_efforts, vec!["high"]);
    }

    #[test]
    fn merge_catalog_sorts_all_harnesses_and_keeps_missing_saved_model() {
        let cat = TraycerCatalog {
            harnesses: vec![
                crate::ipc::TraycerHarnessInfo {
                    id: "hermes".into(),
                    label: "Hermes Agent".into(),
                    models: vec![],
                },
                crate::ipc::TraycerHarnessInfo {
                    id: "codex".into(),
                    label: "Codex".into(),
                    models: vec![crate::ipc::TraycerModelInfo {
                        id: "gpt-6.2-sol".into(),
                        reasoning_efforts: vec!["low".into(), "medium".into(), "high".into()],
                        fast_mode_available: true,
                    }],
                },
                crate::ipc::TraycerHarnessInfo {
                    id: "antigravity".into(),
                    label: "Antigravity".into(),
                    models: vec![],
                },
            ],
        };
        let merged = merge_catalog_with_saved(&cat, "codex", "custom-codex", Some("xhigh"));
        let ids: Vec<_> = merged.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, vec!["antigravity", "codex", "hermes"]);
        let codex = &merged[1];
        assert!(codex.models.iter().any(|m| m.id == "custom-codex" && m.reasoning_efforts == vec!["xhigh"]));
    }
}
