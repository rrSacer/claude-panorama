pub mod charts;
pub mod renderer;

use crate::client;
use crate::gui::renderer::{render_project_view, ProjectViewCallbacks, UiViewState};
use crate::hardware::{detect_project_hardware, LiveHardwareStats, SystemHardwareSpecs};
use crate::ipc::{AgentActionResult, TraycerCatalog};
use crate::model::{HardwareConfig, PanoramaAgentConfig, ProjectDocument};
use crate::store::{
    compute_dir_fingerprint, delete_project, ensure_projects_dir, init_project, load_all_projects,
    load_state, move_project_in_order, resolve_projects_dir, save_state, set_project_archived,
    update_project_hardware, InitOptions,
};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

const APP_ID: &str = "com.sacer.ClaudePanorama";

const CUSTOM_CSS: &str = r#"
.project-banner-card {
    padding: 14px 16px;
}

.fixed-metric-card {
    padding: 16px;
}

.panorama-block-card {
    padding: 14px 16px;
}

.kpi-card {
    padding: 12px 14px;
    border-left: 4px solid alpha(currentColor, 0.22);
}

.kpi-card.status-ok {
    border-left-color: #2ec27e;
}

.kpi-card.status-warn {
    border-left-color: #e5a50a;
}

.kpi-card.status-err {
    border-left-color: #e01b24;
}

.kpi-card.status-neutral {
    border-left-color: #3584e4;
}

.status-pill {
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 0.78rem;
    font-weight: 700;
}

.mini-pill {
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 0.73rem;
    font-weight: 700;
}

.badge-ativo, .pill-run {
    background-color: alpha(#3584e4, 0.18);
    color: #3584e4;
}

.badge-validando {
    background-color: alpha(#9141ac, 0.20);
    color: #c061cb;
}

.badge-pausado, .pill-pend {
    background-color: alpha(currentColor, 0.10);
}

.badge-concluido, .pill-pass {
    background-color: alpha(#2ec27e, 0.18);
    color: #26a269;
}

.badge-bloqueado, .pill-fail {
    background-color: alpha(#e01b24, 0.18);
    color: #e01b24;
}

.pill-bloc {
    background-color: alpha(#e5a50a, 0.20);
    color: #c88800;
}

.pill-resv {
    background-color: alpha(#9141ac, 0.20);
    color: #a347ba;
}

.text-ok {
    color: #2ec27e;
}

.text-warn {
    color: #e5a50a;
}

.alert-info, .prose-info {
    border-left: 4px solid #3584e4;
}

.alert-success, .prose-success {
    border-left: 4px solid #2ec27e;
}

.alert-warning, .prose-warning {
    border-left: 4px solid #e5a50a;
}

.alert-danger, .prose-danger {
    border-left: 4px solid #e01b24;
}

.panorama-tab-bar {
    padding: 4px;
    border-radius: 10px;
    background-color: alpha(currentColor, 0.05);
}

.panorama-tab-btn {
    padding: 6px 12px;
    border-radius: 8px;
}

.table-header-cell {
    opacity: 0.78;
}

.table-first-col {
    font-weight: 600;
}

.sidebar-project-row {
    padding: 8px 6px;
}

.reorder-arrow-btn {
    min-width: 24px;
    min-height: 22px;
    padding: 2px;
}

.hw-limit-row {
    padding: 6px 8px;
    border-radius: 8px;
    background-color: alpha(currentColor, 0.03);
}

.bar-ok trough progress {
    background-color: #2ec27e;
}

.bar-warn trough progress {
    background-color: #e5a50a;
}

.bar-err trough progress {
    background-color: #e01b24;
}

.icon-card {
    padding: 16px;
}

.icon-badge {
    padding: 10px;
    border-radius: 12px;
}

.tone-info { background-color: alpha(#3584e4, 0.16); color: #3584e4; }
.tone-success { background-color: alpha(#2ec27e, 0.16); color: #26a269; }
.tone-warning { background-color: alpha(#e5a50a, 0.18); color: #c88800; }
.tone-danger { background-color: alpha(#e01b24, 0.16); color: #e01b24; }
.tone-neutral { background-color: alpha(currentColor, 0.08); }

.expander-icon.icon-neutral { color: #3584e4; }
.expander-icon.icon-ok { color: #2ec27e; }
.expander-icon.icon-warn { color: #e5a50a; }
.expander-icon.icon-err { color: #e01b24; }
"#;

enum GuiEvent {
    ModelsLoaded {
        catalog: Result<TraycerCatalog, String>,
        silent: bool,
    },
    AgentFinished {
        slug: String,
        result: Result<AgentActionResult, String>,
    },
    HardwareSnapshotReady {
        specs: SystemHardwareSpecs,
        stats: HashMap<String, LiveHardwareStats>,
    },
    HardwareLimitError {
        message: String,
    },
}

pub fn run_gui() -> i32 {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    app.connect_startup(|_| {
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(CUSTOM_CSS);
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });

    app.connect_activate(build_main_window);
    let code = app.run_with_args::<&str>(&[]);
    code.into()
}

struct AppController {
    projects_dir: PathBuf,
    projects: RefCell<Vec<ProjectDocument>>,
    selected_slug: RefCell<Option<String>>,
    filter_query: RefCell<String>,
    show_archived: Cell<bool>,
    fingerprint: Cell<u64>,
    rebuilding_sidebar: Cell<bool>,
    debounce_scheduled: Cell<bool>,
    ui_state: UiViewState,

    catalog: RefCell<TraycerCatalog>,
    hw_specs: RefCell<SystemHardwareSpecs>,
    hw_stats: RefCell<HashMap<String, LiveHardwareStats>>,
    hw_poll_inflight: Cell<bool>,

    tx: mpsc::Sender<GuiEvent>,
    window: adw::ApplicationWindow,
    toast_overlay: adw::ToastOverlay,
    split_view: adw::NavigationSplitView,
    sidebar_list: gtk4::ListBox,
    sidebar_win_title: adw::WindowTitle,
    content_title: adw::WindowTitle,
    update_btn: gtk4::Button,
    archive_btn: gtk4::Button,
    delete_btn: gtk4::Button,
    error_banner: adw::Banner,
    content_scroller: gtk4::ScrolledWindow,

    _monitor: RefCell<Option<gio::FileMonitor>>,
}

fn build_main_window(app: &adw::Application) {
    let projects_dir = resolve_projects_dir();
    let _ = ensure_projects_dir(&projects_dir);

    let (tx, rx) = mpsc::channel::<GuiEvent>();

    // === SIDEBAR ===
    let sidebar_header = adw::HeaderBar::new();
    let sidebar_win_title = adw::WindowTitle::new("Claude Panorama", "Projetos");
    sidebar_header.set_title_widget(Some(&sidebar_win_title));

    let new_btn = gtk4::Button::from_icon_name("list-add-symbolic");
    new_btn.add_css_class("suggested-action");
    new_btn.set_tooltip_text(Some("Novo panorama"));
    sidebar_header.pack_start(&new_btn);

    let archived_toggle = gtk4::ToggleButton::new();
    archived_toggle.set_icon_name("package-x-generic-symbolic");
    archived_toggle.set_tooltip_text(Some("Mostrar projetos arquivados"));
    sidebar_header.pack_end(&archived_toggle);

    let open_dir_btn = gtk4::Button::from_icon_name("folder-open-symbolic");
    open_dir_btn.set_tooltip_text(Some("Abrir pasta de JSONs dos projetos"));
    sidebar_header.pack_end(&open_dir_btn);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Filtrar projetos...")
        .hexpand(true)
        .margin_start(10)
        .margin_end(10)
        .margin_top(6)
        .margin_bottom(6)
        .build();

    let sidebar_list = gtk4::ListBox::new();
    sidebar_list.add_css_class("navigation-sidebar");
    sidebar_list.set_selection_mode(gtk4::SelectionMode::Single);
    sidebar_list.set_activate_on_single_click(true);

    let sidebar_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .child(&sidebar_list)
        .build();

    let sidebar_body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    sidebar_body.append(&search_entry);
    sidebar_body.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    sidebar_body.append(&sidebar_scroll);

    let sidebar_toolbar = adw::ToolbarView::new();
    sidebar_toolbar.add_top_bar(&sidebar_header);
    sidebar_toolbar.set_content(Some(&sidebar_body));

    let sidebar_page = adw::NavigationPage::builder()
        .title("Projetos")
        .tag("sidebar")
        .child(&sidebar_toolbar)
        .build();

    // === CONTEÚDO ===
    let content_header = adw::HeaderBar::new();
    let content_title = adw::WindowTitle::new("Claude Panorama", "Monitoramento em tempo real");
    content_header.set_title_widget(Some(&content_title));

    let update_btn_content = adw::ButtonContent::builder()
        .icon_name("view-refresh-symbolic")
        .label("Atualizar")
        .build();
    let update_btn = gtk4::Button::builder()
        .child(&update_btn_content)
        .build();
    update_btn.set_tooltip_text(Some(
        "Enviar pedido ao subagente dedicado para atualizar este panorama",
    ));
    content_header.pack_start(&update_btn);

    let archive_btn = gtk4::Button::from_icon_name("package-x-generic-symbolic");
    archive_btn.set_tooltip_text(Some("Arquivar ou desarquivar este projeto"));
    content_header.pack_end(&archive_btn);

    let delete_btn = gtk4::Button::from_icon_name("user-trash-symbolic");
    delete_btn.set_tooltip_text(Some("Deletar este panorama permanentemente"));
    content_header.pack_end(&delete_btn);

    let live_pill = gtk4::Label::new(Some("Ao vivo"));
    live_pill.add_css_class("mini-pill");
    live_pill.add_css_class("pill-pass");
    live_pill.set_valign(gtk4::Align::Center);
    live_pill.set_tooltip_text(Some(
        "Monitorando alterações atômicas via GIO FileMonitor + verificação a cada 2s + hardware 1s",
    ));
    content_header.pack_end(&live_pill);

    let error_banner = adw::Banner::new("");
    error_banner.set_revealed(false);

    let content_scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .build();

    let content_body = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content_body.append(&error_banner);
    content_body.append(&content_scroller);

    let content_toolbar = adw::ToolbarView::new();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.set_content(Some(&content_body));

    let content_page = adw::NavigationPage::builder()
        .title("Panorama")
        .tag("content")
        .child(&content_toolbar)
        .build();

    let split_view = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .min_sidebar_width(305.0)
        .max_sidebar_width(390.0)
        .sidebar_width_fraction(0.28)
        .build();

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&split_view));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Claude Panorama")
        .default_width(1360)
        .default_height(880)
        .width_request(420)
        .height_request(480)
        .content(&toast_overlay)
        .build();

    let bp = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        820.0,
        adw::LengthUnit::Sp,
    ));
    bp.add_setter(&split_view, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(bp);

    let ctrl = Rc::new(AppController {
        projects_dir: projects_dir.clone(),
        projects: RefCell::new(Vec::new()),
        selected_slug: RefCell::new(None),
        filter_query: RefCell::new(String::new()),
        show_archived: Cell::new(false),
        fingerprint: Cell::new(0),
        rebuilding_sidebar: Cell::new(false),
        debounce_scheduled: Cell::new(false),
        ui_state: UiViewState::default(),
        catalog: RefCell::new(TraycerCatalog::default()),
        hw_specs: RefCell::new(SystemHardwareSpecs::detect()),
        hw_stats: RefCell::new(HashMap::new()),
        hw_poll_inflight: Cell::new(false),
        tx,
        window: window.clone(),
        toast_overlay,
        split_view,
        sidebar_list,
        sidebar_win_title,
        content_title,
        update_btn: update_btn.clone(),
        archive_btn: archive_btn.clone(),
        delete_btn: delete_btn.clone(),
        error_banner,
        content_scroller,
        _monitor: RefCell::new(None),
    });

    // Seleção na lista lateral
    {
        let sidebar_list_ref = ctrl.sidebar_list.clone();
        let ctrl = ctrl.clone();
        sidebar_list_ref.connect_row_selected(move |_, row_opt| {
            if ctrl.rebuilding_sidebar.get() {
                return;
            }
            if let Some(row) = row_opt {
                let name = row.widget_name().to_string();
                if name == "__empty__" || name.is_empty() {
                    return;
                }
                *ctrl.selected_slug.borrow_mut() = Some(name);
                render_selected_content(&ctrl, false);
                ctrl.split_view.set_show_content(true);
            }
        });
    }

    // Filtro de busca
    {
        let ctrl = ctrl.clone();
        search_entry.connect_search_changed(move |entry| {
            *ctrl.filter_query.borrow_mut() = entry.text().trim().to_lowercase();
            rebuild_sidebar_rows(&ctrl);
        });
    }

    // Toggle arquivados
    {
        let ctrl = ctrl.clone();
        archived_toggle.connect_toggled(move |btn| {
            ctrl.show_archived.set(btn.is_active());
            ensure_valid_selection(&ctrl);
            rebuild_sidebar_rows(&ctrl);
            render_selected_content(&ctrl, false);
        });
    }

    // Novo Panorama
    {
        let ctrl = ctrl.clone();
        new_btn.connect_clicked(move |_| {
            open_new_panorama_dialog(&ctrl);
        });
    }

    // Botão Atualizar -> envia pedido para o agente dedicado do projeto
    {
        let ctrl = ctrl.clone();
        update_btn.connect_clicked(move |_| {
            trigger_agent_update(&ctrl);
        });
    }

    // Botão Arquivar / Desarquivar
    {
        let ctrl = ctrl.clone();
        archive_btn.connect_clicked(move |_| {
            toggle_archive_selected(&ctrl);
        });
    }

    // Botão Deletar (com confirmação)
    {
        let ctrl = ctrl.clone();
        delete_btn.connect_clicked(move |_| {
            confirm_delete_selected(&ctrl);
        });
    }

    // Botão Abrir Pasta
    {
        let dir = projects_dir.clone();
        let ctrl = ctrl.clone();
        open_dir_btn.connect_clicked(move |_| {
            let uri = gio::File::for_path(&dir).uri();
            if gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE).is_err() {
                show_toast(&ctrl, &format!("Pasta: {}", dir.display()));
            }
        });
    }

    // GIO FileMonitor no diretório de projetos
    let gdir = gio::File::for_path(&projects_dir);
    if let Ok(mon) = gdir.monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE) {
        let ctrl_weak = Rc::downgrade(&ctrl);
        mon.connect_changed(move |_, file, _, event| {
            let Some(ctrl) = ctrl_weak.upgrade() else {
                return;
            };
            let is_relevant = file
                .basename()
                .and_then(|b| b.to_str().map(|s| s.ends_with(".json")))
                .unwrap_or(true);
            if !is_relevant {
                return;
            }
            match event {
                gio::FileMonitorEvent::ChangesDoneHint
                | gio::FileMonitorEvent::Created
                | gio::FileMonitorEvent::Deleted
                | gio::FileMonitorEvent::MovedIn
                | gio::FileMonitorEvent::MovedOut
                | gio::FileMonitorEvent::Renamed
                | gio::FileMonitorEvent::AttributeChanged => {
                    schedule_debounced_reload(&ctrl);
                }
                _ => {}
            }
        });
        *ctrl._monitor.borrow_mut() = Some(mon);
    }

    // Bomba de eventos assíncronos (modelos Traycer, agentes, telemetria 1s)
    {
        let ctrl_weak = Rc::downgrade(&ctrl);
        glib::timeout_add_local(Duration::from_millis(80), move || {
            let Some(ctrl) = ctrl_weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            while let Ok(ev) = rx.try_recv() {
                handle_gui_event(&ctrl, ev);
            }
            glib::ControlFlow::Continue
        });
    }

    // Polling de fallback a cada 2s para mudanças no disco
    {
        let ctrl_weak = Rc::downgrade(&ctrl);
        glib::timeout_add_local(Duration::from_secs(2), move || {
            let Some(ctrl) = ctrl_weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            reload_from_disk(&ctrl, false);
            glib::ControlFlow::Continue
        });
    }

    // Polling de telemetria de hardware em tempo real a cada 1s
    {
        let ctrl_weak = Rc::downgrade(&ctrl);
        glib::timeout_add_local(Duration::from_secs(1), move || {
            let Some(ctrl) = ctrl_weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if !ctrl.hw_poll_inflight.get() {
                let pairs: Vec<(String, String)> = ctrl
                    .projects
                    .borrow()
                    .iter()
                    .filter(|p| !p.archived)
                    .map(|p| (p.slug.clone(), p.path.clone()))
                    .collect();
                if !pairs.is_empty() {
                    ctrl.hw_poll_inflight.set(true);
                    let tx = ctrl.tx.clone();
                    std::thread::spawn(move || {
                        let snap = client::get_hardware_snapshot(pairs);
                        let _ = tx.send(GuiEvent::HardwareSnapshotReady {
                            specs: snap.specs,
                            stats: snap.stats,
                        });
                    });
                }
            }
            glib::ControlFlow::Continue
        });
    }

    reload_from_disk(&ctrl, true);
    fetch_models_async(&ctrl, false, true);
    window.present();

    if let Ok(ms_str) = std::env::var("CLAUDE_PANORAMA_SMOKE_EXIT_MS") {
        if let Ok(ms) = ms_str.parse::<u64>() {
            let app_clone = app.clone();
            glib::timeout_add_local_once(Duration::from_millis(ms), move || {
                app_clone.quit();
            });
        }
    }
}

fn show_toast(ctrl: &Rc<AppController>, msg: &str) {
    let toast = adw::Toast::new(msg);
    toast.set_timeout(4);
    ctrl.toast_overlay.add_toast(toast);
}

fn fetch_models_async(ctrl: &Rc<AppController>, refresh: bool, silent: bool) {
    let tx = ctrl.tx.clone();
    std::thread::spawn(move || {
        let catalog = client::list_traycer_models(refresh);
        let _ = tx.send(GuiEvent::ModelsLoaded { catalog, silent });
    });
}

fn handle_gui_event(ctrl: &Rc<AppController>, ev: GuiEvent) {
    match ev {
        GuiEvent::ModelsLoaded { catalog, silent } => match catalog {
            Ok(cat) => {
                *ctrl.catalog.borrow_mut() = cat;
                render_selected_content(ctrl, true);
                if !silent {
                    show_toast(ctrl, "Modelos do Traycer atualizados.");
                }
            }
            Err(err) => {
                if !silent {
                    show_toast(ctrl, &format!("Erro ao listar modelos do Traycer: {err}"));
                }
            }
        },
        GuiEvent::AgentFinished { slug, result } => match result {
            Ok(res) => {
                show_toast(ctrl, &res.message);
                let fp = compute_dir_fingerprint(&ctrl.projects_dir);
                ctrl.fingerprint.set(fp);
                reload_from_disk(ctrl, true);
                if ctrl.selected_slug.borrow().as_deref() == Some(slug.as_str()) {
                    render_selected_content(ctrl, true);
                }
            }
            Err(err) => {
                show_toast(ctrl, &format!("Agente ({slug}): {err}"));
            }
        },
        GuiEvent::HardwareSnapshotReady { specs, stats } => {
            ctrl.hw_poll_inflight.set(false);
            *ctrl.hw_specs.borrow_mut() = specs.clone();
            *ctrl.hw_stats.borrow_mut() = stats.clone();

            let selected_slug = ctrl.selected_slug.borrow().clone();
            if let Some(slug) = selected_slug {
                if let Some(st) = stats.get(&slug) {
                    let doc_opt = ctrl
                        .projects
                        .borrow()
                        .iter()
                        .find(|p| p.slug == slug)
                        .cloned();
                    if let Some(doc) = doc_opt {
                        let new_needs = detect_project_hardware(&doc, Some(st));
                        let need_rebuild = {
                            let w_opt = ctrl.ui_state.live_hw_widgets.borrow();
                            match w_opt.as_ref() {
                                Some(w) if w.slug == slug => {
                                    w.show_cpu != new_needs.show_cpu
                                        || w.show_ram != new_needs.show_ram
                                        || w.show_ssd != new_needs.show_ssd
                                }
                                None => new_needs.any(),
                                Some(_) => false,
                            }
                        };
                        if need_rebuild {
                            render_selected_content(ctrl, true);
                        } else if let Some(w) = ctrl.ui_state.live_hw_widgets.borrow().as_ref() {
                            if w.slug == slug {
                                w.update(st, &specs);
                            }
                        }
                    }
                }
            }
        }
        GuiEvent::HardwareLimitError { message } => {
            show_toast(ctrl, &format!("Hardware: {message}"));
        }
    }
}

fn schedule_debounced_reload(ctrl: &Rc<AppController>) {
    if ctrl.debounce_scheduled.replace(true) {
        return;
    }
    let ctrl_weak = Rc::downgrade(ctrl);
    glib::timeout_add_local_once(Duration::from_millis(120), move || {
        if let Some(c) = ctrl_weak.upgrade() {
            c.debounce_scheduled.set(false);
            reload_from_disk(&c, false);
        }
    });
}

fn visible_projects_filtered(ctrl: &AppController) -> Vec<ProjectDocument> {
    let query = ctrl.filter_query.borrow().clone();
    let show_archived = ctrl.show_archived.get();
    ctrl.projects
        .borrow()
        .iter()
        .filter(|proj| {
            if !show_archived && proj.archived {
                return false;
            }
            if !query.is_empty() {
                let matches = proj.title.to_lowercase().contains(&query)
                    || proj.slug.to_lowercase().contains(&query)
                    || proj.path.to_lowercase().contains(&query)
                    || proj.phase.to_lowercase().contains(&query)
                    || proj.status.label().contains(&query);
                if !matches {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

fn ensure_valid_selection(ctrl: &AppController) {
    let visible = visible_projects_filtered(ctrl);
    let prev_slug = ctrl.selected_slug.borrow().clone();
    let still_visible = prev_slug
        .as_deref()
        .is_some_and(|s| visible.iter().any(|p| p.slug == s));
    if !still_visible {
        *ctrl.selected_slug.borrow_mut() = visible.first().map(|p| p.slug.clone());
    }
}

fn reload_from_disk(ctrl: &Rc<AppController>, force: bool) {
    let fp = compute_dir_fingerprint(&ctrl.projects_dir);
    if !force && fp == ctrl.fingerprint.get() {
        return;
    }
    ctrl.fingerprint.set(fp);

    let loaded = load_all_projects(&ctrl.projects_dir);
    if loaded.errors.is_empty() {
        ctrl.error_banner.set_revealed(false);
    } else {
        let msg = loaded
            .errors
            .iter()
            .map(|(f, e)| format!("{f}: {e}"))
            .collect::<Vec<_>>()
            .join(" | ");
        ctrl.error_banner.set_title(&msg);
        ctrl.error_banner.set_revealed(true);
    }

    let active_count = loaded.projects.iter().filter(|p| !p.archived).count();
    let arch_count = loaded.projects.iter().filter(|p| p.archived).count();
    if arch_count > 0 {
        ctrl.sidebar_win_title
            .set_subtitle(&format!("{active_count} ativos · {arch_count} arquiv."));
    } else {
        ctrl.sidebar_win_title
            .set_subtitle(&format!("{active_count} projetos"));
    }

    let prev_slug = ctrl.selected_slug.borrow().clone();
    *ctrl.projects.borrow_mut() = loaded.projects;
    ensure_valid_selection(ctrl);
    let next_slug = ctrl.selected_slug.borrow().clone();
    let preserve_scroll = prev_slug == next_slug && prev_slug.is_some();

    rebuild_sidebar_rows(ctrl);
    render_selected_content(ctrl, preserve_scroll);
}

fn rebuild_sidebar_rows(ctrl: &Rc<AppController>) {
    ctrl.rebuilding_sidebar.set(true);
    while let Some(child) = ctrl.sidebar_list.first_child() {
        ctrl.sidebar_list.remove(&child);
    }

    let visible = visible_projects_filtered(ctrl);
    let visible_slugs: Vec<String> = visible.iter().map(|p| p.slug.clone()).collect();
    let selected = ctrl.selected_slug.borrow().clone();

    let mut row_to_select: Option<gtk4::ListBoxRow> = None;

    for (idx, proj) in visible.iter().enumerate() {
        let is_first = idx == 0;
        let is_last = idx + 1 == visible.len();
        let row = build_sidebar_row(ctrl, proj, is_first, is_last, &visible_slugs);
        if selected.as_deref() == Some(proj.slug.as_str()) {
            row_to_select = Some(row.clone());
        }
        ctrl.sidebar_list.append(&row);
    }

    if visible.is_empty() {
        let msg = if ctrl.projects.borrow().is_empty() {
            "Nenhum projeto registrado"
        } else {
            "Nenhum projeto visível neste filtro"
        };
        let empty_lbl = gtk4::Label::new(Some(msg));
        empty_lbl.add_css_class("dim-label");
        empty_lbl.set_margin_top(24);
        empty_lbl.set_margin_bottom(24);
        let empty_row = gtk4::ListBoxRow::builder()
            .selectable(false)
            .activatable(false)
            .name("__empty__")
            .child(&empty_lbl)
            .build();
        ctrl.sidebar_list.append(&empty_row);
    }

    if let Some(r) = row_to_select {
        ctrl.sidebar_list.select_row(Some(&r));
    }
    ctrl.rebuilding_sidebar.set(false);
}

fn build_sidebar_row(
    ctrl: &Rc<AppController>,
    proj: &ProjectDocument,
    is_first: bool,
    is_last: bool,
    visible_slugs: &[String],
) -> gtk4::ListBoxRow {
    let row_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row_hbox.add_css_class("sidebar-project-row");

    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 5);
    outer.set_hexpand(true);

    let top = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let icon = gtk4::Image::from_icon_name(&proj.icon);
    icon.set_pixel_size(18);
    icon.set_valign(gtk4::Align::Center);
    top.append(&icon);

    let title_lbl = gtk4::Label::new(Some(&proj.title));
    title_lbl.add_css_class("heading");
    title_lbl.set_xalign(0.0);
    title_lbl.set_hexpand(true);
    title_lbl.set_ellipsize(pango::EllipsizeMode::End);
    top.append(&title_lbl);

    if proj.archived {
        let arch_lbl = gtk4::Label::new(Some("Arquivado"));
        arch_lbl.add_css_class("mini-pill");
        arch_lbl.add_css_class("pill-pend");
        arch_lbl.set_valign(gtk4::Align::Center);
        top.append(&arch_lbl);
    }

    let status_lbl = gtk4::Label::new(Some(proj.status.label()));
    status_lbl.add_css_class("mini-pill");
    status_lbl.add_css_class(proj.status.css_class());
    status_lbl.set_valign(gtk4::Align::Center);
    top.append(&status_lbl);
    outer.append(&top);

    let short_repo = shorten_path(&proj.path, &proj.slug);
    let repo_lbl = gtk4::Label::new(Some(&short_repo));
    repo_lbl.add_css_class("caption");
    repo_lbl.add_css_class("dim-label");
    repo_lbl.add_css_class("monospace");
    repo_lbl.set_xalign(0.0);
    repo_lbl.set_ellipsize(pango::EllipsizeMode::Middle);
    outer.append(&repo_lbl);

    let metrics_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let prog_pill = gtk4::Label::new(Some(&format!("Prog {:.1}%", proj.progresso)));
    prog_pill.add_css_class("mini-pill");
    prog_pill.add_css_class("pill-run");

    let conf_pill = gtk4::Label::new(Some(&format!("Conf {:.1}%", proj.confianca)));
    conf_pill.add_css_class("mini-pill");
    conf_pill.add_css_class(if proj.confianca >= 75.0 {
        "pill-pass"
    } else if proj.confianca >= 45.0 {
        "pill-bloc"
    } else {
        "pill-fail"
    });

    metrics_row.append(&prog_pill);
    metrics_row.append(&conf_pill);
    outer.append(&metrics_row);

    row_hbox.append(&outer);

    // Setas ↑ / ↓ para reordenar na lista
    let arrows_col = gtk4::Box::new(gtk4::Orientation::Vertical, 1);
    arrows_col.set_valign(gtk4::Align::Center);

    let up_btn = gtk4::Button::from_icon_name("go-up-symbolic");
    up_btn.add_css_class("flat");
    up_btn.add_css_class("reorder-arrow-btn");
    up_btn.set_sensitive(!is_first);
    up_btn.set_tooltip_text(Some("Mover projeto para cima"));

    let down_btn = gtk4::Button::from_icon_name("go-down-symbolic");
    down_btn.add_css_class("flat");
    down_btn.add_css_class("reorder-arrow-btn");
    down_btn.set_sensitive(!is_last);
    down_btn.set_tooltip_text(Some("Mover projeto para baixo"));

    {
        let ctrl = ctrl.clone();
        let slug = proj.slug.clone();
        let vis = visible_slugs.to_vec();
        up_btn.connect_clicked(move |_| {
            if let Err(e) = move_project_in_order(&ctrl.projects_dir, &slug, true, Some(&vis)) {
                show_toast(&ctrl, &e);
            } else {
                reload_from_disk(&ctrl, true);
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let slug = proj.slug.clone();
        let vis = visible_slugs.to_vec();
        down_btn.connect_clicked(move |_| {
            if let Err(e) = move_project_in_order(&ctrl.projects_dir, &slug, false, Some(&vis)) {
                show_toast(&ctrl, &e);
            } else {
                reload_from_disk(&ctrl, true);
            }
        });
    }

    arrows_col.append(&up_btn);
    arrows_col.append(&down_btn);
    row_hbox.append(&arrows_col);

    gtk4::ListBoxRow::builder()
        .name(&proj.slug)
        .child(&row_hbox)
        .build()
}

fn render_selected_content(ctrl: &Rc<AppController>, preserve_scroll: bool) {
    let saved_vscroll = if preserve_scroll {
        Some(ctrl.content_scroller.vadjustment().value())
    } else {
        None
    };

    let selected = ctrl.selected_slug.borrow().clone();
    let found = selected
        .as_deref()
        .and_then(|s| ctrl.projects.borrow().iter().find(|p| p.slug == s).cloned());

    if let Some(doc) = found {
        let sub = if doc.phase.trim().is_empty() {
            doc.slug.clone()
        } else {
            format!("{} — {}", doc.slug, doc.phase)
        };
        ctrl.content_title.set_title(&doc.title);
        ctrl.content_title.set_subtitle(&sub);

        let has_linked_agent = doc
            .agent
            .as_ref()
            .and_then(|a| a.agent_id.as_deref())
            .is_some_and(|aid| !aid.trim().is_empty())
            && doc
                .agent
                .as_ref()
                .and_then(|a| a.epic_id.as_deref())
                .is_some_and(|eid| !eid.trim().is_empty());
        ctrl.update_btn.set_sensitive(has_linked_agent);
        ctrl.update_btn.set_tooltip_text(Some(if has_linked_agent {
            "Enviar pedido ao subagente dedicado para atualizar este panorama"
        } else {
            "Aguardando o maestro da conversa no Traycer criar e vincular o subagente"
        }));
        ctrl.archive_btn.set_sensitive(true);
        ctrl.delete_btn.set_sensitive(true);

        if doc.archived {
            ctrl.archive_btn.set_icon_name("view-restore-symbolic");
            ctrl.archive_btn
                .set_tooltip_text(Some("Desarquivar este projeto"));
        } else {
            ctrl.archive_btn.set_icon_name("package-x-generic-symbolic");
            ctrl.archive_btn
                .set_tooltip_text(Some("Arquivar este projeto"));
        }

        let callbacks = ProjectViewCallbacks {
            catalog: ctrl.catalog.borrow().clone(),
            hw_specs: ctrl.hw_specs.borrow().clone(),
            live_hw: ctrl.hw_stats.borrow().get(&doc.slug).cloned(),
            on_start_or_configure_agent: {
                let ctrl = ctrl.clone();
                let slug = doc.slug.clone();
                let title = doc.title.clone();
                let path = doc.path.clone();
                let cur_agent = doc.agent.clone();
                Rc::new(move |harness_id, model, effort| {
                    start_or_configure_project_agent(
                        &ctrl,
                        &slug,
                        &title,
                        &path,
                        cur_agent.as_ref(),
                        &harness_id,
                        &model,
                        effort,
                    );
                })
            },
            on_request_update: {
                let ctrl = ctrl.clone();
                Rc::new(move || {
                    trigger_agent_update(&ctrl);
                })
            },
            on_refresh_models: {
                let ctrl = ctrl.clone();
                Rc::new(move || {
                    show_toast(&ctrl, "Buscando modelos disponíveis no Traycer...");
                    fetch_models_async(&ctrl, true, false);
                })
            },
            on_hardware_changed: {
                let ctrl = ctrl.clone();
                let slug = doc.slug.clone();
                let path = doc.path.clone();
                Rc::new(move |hw_cfg| {
                    apply_and_save_hardware_limits(&ctrl, &slug, &path, hw_cfg);
                })
            },
        };

        let view = render_project_view(&doc, &ctrl.ui_state, &callbacks);
        ctrl.content_scroller.set_child(Some(&view));

        if let Some(scroll_y) = saved_vscroll {
            let scroller = ctrl.content_scroller.clone();
            glib::idle_add_local_once(move || {
                scroller.vadjustment().set_value(scroll_y);
            });
        } else {
            ctrl.content_scroller.vadjustment().set_value(0.0);
        }
    } else {
        *ctrl.ui_state.live_hw_widgets.borrow_mut() = None;
        ctrl.update_btn.set_sensitive(false);
        ctrl.archive_btn.set_sensitive(false);
        ctrl.delete_btn.set_sensitive(false);
        ctrl.content_title.set_title("Claude Panorama");
        ctrl.content_title
            .set_subtitle("Nenhum projeto selecionado");
        let empty = adw::StatusPage::builder()
            .icon_name("applications-engineering-symbolic")
            .title("Nenhum projeto registrado ainda")
            .description(format!(
                "Clique no botão + na barra lateral para criar um panorama.\n\nDiretório monitorado:\n{}",
                ctrl.projects_dir.display()
            ))
            .vexpand(true)
            .build();
        ctrl.content_scroller.set_child(Some(&empty));
    }
}

#[allow(clippy::too_many_arguments)]
fn start_or_configure_project_agent(
    ctrl: &Rc<AppController>,
    slug: &str,
    _title: &str,
    _path: &str,
    cur_agent: Option<&PanoramaAgentConfig>,
    harness_id: &str,
    model: &str,
    effort: Option<String>,
) {
    let Some(ag) = cur_agent.filter(|a| {
        a.agent_id.as_deref().is_some_and(|s| !s.trim().is_empty())
            && a.epic_id.as_deref().is_some_and(|s| !s.trim().is_empty())
    }) else {
        show_toast(
            ctrl,
            "Nenhum subagente vinculado. Peça ao maestro da conversa no Traycer para criar e vincular o subagente.",
        );
        return;
    };

    let mut state = load_state(&ctrl.projects_dir);
    state.default_harness = Some(harness_id.to_string());
    state.default_model = Some(model.to_string());
    state.default_reasoning_effort = effort.clone();
    let _ = save_state(&ctrl.projects_dir, &state);

    let projects_dir = ctrl.projects_dir.clone();
    let slug_owned = slug.to_string();
    let harness_owned = harness_id.to_string();
    let model_owned = model.to_string();
    let tx = ctrl.tx.clone();
    let aid = ag.agent_id.clone().unwrap_or_default();
    let eid = ag.epic_id.clone();

    show_toast(
        ctrl,
        &format!("Configurando modelo do agente ({harness_id} · {model})..."),
    );
    std::thread::spawn(move || {
        let res = client::configure_panorama_agent(
            &projects_dir,
            &slug_owned,
            &aid,
            eid.as_deref(),
            &harness_owned,
            &model_owned,
            effort.as_deref(),
        );
        let _ = tx.send(GuiEvent::AgentFinished {
            slug: slug_owned,
            result: res,
        });
    });
}

fn trigger_agent_update(ctrl: &Rc<AppController>) {
    let Some(slug) = ctrl.selected_slug.borrow().clone() else {
        return;
    };
    let doc_opt = ctrl
        .projects
        .borrow()
        .iter()
        .find(|p| p.slug == slug)
        .cloned();
    let Some(doc) = doc_opt else {
        return;
    };

    let ag = doc.agent.clone();
    let agent_id = ag
        .as_ref()
        .and_then(|a| a.agent_id.clone())
        .filter(|s| !s.trim().is_empty());
    let epic_id = ag
        .as_ref()
        .and_then(|a| a.epic_id.clone())
        .filter(|s| !s.trim().is_empty());

    if agent_id.is_none() || epic_id.is_none() {
        show_toast(
            ctrl,
            "Nenhum subagente vinculado. Peça ao maestro da conversa no Traycer para criar e vincular o subagente.",
        );
        return;
    }

    show_toast(
        ctrl,
        &format!("Enviando pedido de atualização para o subagente de `{}`...", doc.title),
    );

    let projects_dir = ctrl.projects_dir.clone();
    let state = load_state(&projects_dir);
    let harness = ag
        .as_ref()
        .map(|a| a.harness_id.clone())
        .filter(|s| !s.is_empty())
        .or(state.default_harness);
    let model = ag
        .as_ref()
        .map(|a| a.model.clone())
        .filter(|s| !s.is_empty())
        .or(state.default_model);
    let effort = ag
        .as_ref()
        .and_then(|a| a.reasoning_effort.clone())
        .or(state.default_reasoning_effort);

    let tx = ctrl.tx.clone();
    std::thread::spawn(move || {
        let res = client::request_panorama_update(
            &projects_dir,
            &doc.slug,
            &doc.title,
            &doc.path,
            agent_id.as_deref(),
            epic_id.as_deref(),
            harness.as_deref(),
            model.as_deref(),
            effort.as_deref(),
        );
        let _ = tx.send(GuiEvent::AgentFinished {
            slug: doc.slug,
            result: res,
        });
    });
}

fn toggle_archive_selected(ctrl: &Rc<AppController>) {
    let Some(slug) = ctrl.selected_slug.borrow().clone() else {
        return;
    };
    let doc_opt = ctrl
        .projects
        .borrow()
        .iter()
        .find(|p| p.slug == slug)
        .cloned();
    let Some(doc) = doc_opt else {
        return;
    };

    let target_archived = !doc.archived;
    match set_project_archived(&ctrl.projects_dir, &slug, target_archived) {
        Ok(updated) => {
            if target_archived {
                let slug_rm = slug.clone();
                std::thread::spawn(move || {
                    let _ = client::remove_hardware_limits(&slug_rm);
                });
            }
            if let Some(ag) = updated.agent {
                if let Some(aid) = ag.agent_id {
                    let eid = ag.epic_id;
                    std::thread::spawn(move || {
                        let _ = client::archive_traycer_agent(&aid, eid.as_deref(), target_archived);
                    });
                }
            }
            show_toast(
                ctrl,
                if target_archived {
                    "Projeto arquivado."
                } else {
                    "Projeto restaurado dos arquivados."
                },
            );
            reload_from_disk(ctrl, true);
        }
        Err(err) => {
            show_toast(ctrl, &format!("Erro ao arquivar: {err}"));
        }
    }
}

fn confirm_delete_selected(ctrl: &Rc<AppController>) {
    let Some(slug) = ctrl.selected_slug.borrow().clone() else {
        return;
    };
    let doc_opt = ctrl
        .projects
        .borrow()
        .iter()
        .find(|p| p.slug == slug)
        .cloned();
    let Some(doc) = doc_opt else {
        return;
    };

    let dialog = adw::AlertDialog::builder()
        .heading("Deletar panorama permanentemente?")
        .body(&format!(
            "Isso removerá o arquivo `{}.json` ({}) do Claude Panorama. O código do projeto na pasta original não será apagado.",
            doc.slug, doc.title
        ))
        .close_response("cancel")
        .default_response("cancel")
        .build();
    dialog.add_responses(&[("cancel", "Cancelar"), ("delete", "Deletar")]);
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

    let ctrl_c = ctrl.clone();
    dialog.connect_response(None, move |_, resp| {
        if resp == "delete" {
            let slug_rm = doc.slug.clone();
            std::thread::spawn(move || {
                let _ = client::remove_hardware_limits(&slug_rm);
            });
            match delete_project(&ctrl_c.projects_dir, &doc.slug) {
                Ok(()) => {
                    show_toast(&ctrl_c, &format!("Projeto `{}` deletado.", doc.title));
                    *ctrl_c.selected_slug.borrow_mut() = None;
                    reload_from_disk(&ctrl_c, true);
                }
                Err(err) => {
                    show_toast(&ctrl_c, &format!("Erro ao deletar: {err}"));
                }
            }
        }
    });
    dialog.present(Some(&ctrl.window));
}

fn apply_and_save_hardware_limits(
    ctrl: &Rc<AppController>,
    slug: &str,
    path: &str,
    hw_cfg: HardwareConfig,
) {
    if let Err(err) = update_project_hardware(&ctrl.projects_dir, slug, hw_cfg.clone()) {
        show_toast(ctrl, &format!("Erro ao salvar limites: {err}"));
        return;
    }

    // Atualiza estado em memória + fingerprint sem recriar os controles durante o arraste
    ctrl.fingerprint
        .set(compute_dir_fingerprint(&ctrl.projects_dir));
    for proj in ctrl.projects.borrow_mut().iter_mut() {
        if proj.slug == slug {
            proj.hardware = Some(hw_cfg.clone());
        }
    }

    let slug_owned = slug.to_string();
    let path_owned = path.to_string();
    let tx = ctrl.tx.clone();
    std::thread::spawn(move || {
        let res = if hw_cfg.cpu_cores.is_some()
            || hw_cfg.ssd_percent.is_some()
            || hw_cfg.max_ram_gb.is_some()
        {
            client::apply_hardware_limits(
                &slug_owned,
                &path_owned,
                hw_cfg.cpu_cores,
                hw_cfg.ssd_percent,
                hw_cfg.max_ram_gb,
            )
        } else {
            client::remove_hardware_limits(&slug_owned)
        };
        if let Err(e) = res {
            let _ = tx.send(GuiEvent::HardwareLimitError { message: e });
        }
    });
}

fn open_new_panorama_dialog(ctrl: &Rc<AppController>) {
    let dialog = adw::Dialog::builder()
        .title("Novo Panorama")
        .content_width(480)
        .build();

    let header = adw::HeaderBar::new();
    let cancel_btn = gtk4::Button::with_label("Cancelar");
    let create_btn = gtk4::Button::with_label("Criar Panorama");
    create_btn.add_css_class("suggested-action");
    header.pack_start(&cancel_btn);
    header.pack_end(&create_btn);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    content.set_margin_top(16);
    content.set_margin_bottom(20);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let basic_group = adw::PreferencesGroup::builder()
        .title("Dados do Projeto")
        .description("Cria o registro do panorama. Para montar e atualizar os dados, peça ao maestro da conversa do projeto no Traycer para criar e vincular o subagente na aba Agents.")
        .build();

    let title_row = adw::EntryRow::builder().title("Nome do projeto").build();
    let slug_row = adw::EntryRow::builder().title("Identificador (slug)").build();
    let phase_row = adw::EntryRow::builder()
        .title("Fase inicial (opcional)")
        .text("Análise inicial")
        .build();

    let path_row = adw::EntryRow::builder().title("Pasta do projeto").build();
    let browse_btn = gtk4::Button::from_icon_name("folder-open-symbolic");
    browse_btn.add_css_class("flat");
    browse_btn.set_valign(gtk4::Align::Center);
    browse_btn.set_tooltip_text(Some("Escolher pasta do projeto"));
    path_row.add_suffix(&browse_btn);

    basic_group.add(&title_row);
    basic_group.add(&slug_row);
    basic_group.add(&path_row);
    basic_group.add(&phase_row);
    content.append(&basic_group);

    let slug_edited_manually = Rc::new(Cell::new(false));
    {
        let slug_row = slug_row.clone();
        let slug_edited_manually = slug_edited_manually.clone();
        title_row.connect_changed(move |tr| {
            if !slug_edited_manually.get() {
                slug_row.set_text(&slugify(&tr.text()));
            }
        });
    }
    {
        let slug_edited_manually = slug_edited_manually.clone();
        slug_row.connect_changed(move |sr| {
            if sr.has_focus() {
                slug_edited_manually.set(true);
            }
        });
    }
    {
        let window = ctrl.window.clone();
        let path_row = path_row.clone();
        let title_row = title_row.clone();
        browse_btn.connect_clicked(move |_| {
            let fd = gtk4::FileDialog::builder()
                .title("Selecionar pasta do projeto")
                .modal(true)
                .build();
            let path_row = path_row.clone();
            let title_row = title_row.clone();
            fd.select_folder(Some(&window), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(p) = file.path() {
                        path_row.set_text(&p.to_string_lossy());
                        if title_row.text().trim().is_empty() {
                            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                                title_row.set_text(name);
                            }
                        }
                    }
                }
            });
        });
    }

    let dlg_toolbar = adw::ToolbarView::new();
    dlg_toolbar.add_top_bar(&header);
    dlg_toolbar.set_content(Some(&content));
    dialog.set_child(Some(&dlg_toolbar));

    {
        let dialog = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            dialog.close();
        });
    }

    {
        let ctrl = ctrl.clone();
        let dialog = dialog.clone();
        create_btn.connect_clicked(move |_| {
            let title = title_row.text().trim().to_string();
            let slug = slugify(slug_row.text().trim());
            let path = path_row.text().trim().to_string();
            let phase = phase_row.text().trim().to_string();

            if title.is_empty() || slug.is_empty() {
                show_toast(&ctrl, "Informe um nome e um identificador válidos.");
                return;
            }

            match init_project(
                &ctrl.projects_dir,
                InitOptions {
                    slug: slug.clone(),
                    title: title.clone(),
                    path,
                    progresso: 0.0,
                    confianca: 50.0,
                    phase,
                    agent: None,
                    hardware: None,
                },
            ) {
                Ok(_) => {
                    *ctrl.selected_slug.borrow_mut() = Some(slug);
                    dialog.close();
                    reload_from_disk(&ctrl, true);
                    show_toast(
                        &ctrl,
                        &format!("Panorama `{title}` criado. Peça ao maestro da conversa no Traycer para vincular o subagente."),
                    );
                }
                Err(err) => {
                    show_toast(&ctrl, &format!("Erro ao criar panorama: {err}"));
                }
            }
        });
    }

    dialog.present(Some(&ctrl.window));
}
fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in input.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn shorten_path(path: &str, fallback_slug: &str) -> String {
    if path.trim().is_empty() {
        return format!("slug: {fallback_slug}");
    }
    if let Some(rest) = path.strip_prefix("/home/sacer/") {
        return format!("~/{rest}");
    }
    path.to_string()
}
