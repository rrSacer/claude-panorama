pub mod charts;
pub mod renderer;

use crate::gui::renderer::{render_project_view, UiViewState};
use crate::model::ProjectDocument;
use crate::store::{
    compute_dir_fingerprint, ensure_projects_dir, load_all_projects, resolve_projects_dir,
};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
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

.bar-ok trough progress {
    background-color: #2ec27e;
}

.bar-warn trough progress {
    background-color: #e5a50a;
}

.bar-err trough progress {
    background-color: #e01b24;
}
"#;

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
    fingerprint: Cell<u64>,
    rebuilding_sidebar: Cell<bool>,
    debounce_scheduled: Cell<bool>,
    ui_state: UiViewState,

    split_view: adw::NavigationSplitView,
    sidebar_list: gtk4::ListBox,
    content_title: adw::WindowTitle,
    error_banner: adw::Banner,
    content_scroller: gtk4::ScrolledWindow,

    _monitor: RefCell<Option<gio::FileMonitor>>,
}

fn build_main_window(app: &adw::Application) {
    let projects_dir = resolve_projects_dir();
    let _ = ensure_projects_dir(&projects_dir);

    // === SIDEBAR ===
    let sidebar_header = adw::HeaderBar::new();
    let sidebar_win_title = adw::WindowTitle::new("Claude Panorama", "Projetos");
    sidebar_header.set_title_widget(Some(&sidebar_win_title));

    let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
    refresh_btn.set_tooltip_text(Some("Recarregar projetos do disco agora"));
    sidebar_header.pack_start(&refresh_btn);

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

    let live_pill = gtk4::Label::new(Some("Ao vivo"));
    live_pill.add_css_class("mini-pill");
    live_pill.add_css_class("pill-pass");
    live_pill.set_valign(gtk4::Align::Center);
    live_pill.set_tooltip_text(Some(
        "Monitorando alterações atômicas via GIO FileMonitor + verificação a cada 2s",
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
        .min_sidebar_width(290.0)
        .max_sidebar_width(370.0)
        .sidebar_width_fraction(0.27)
        .build();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Claude Panorama")
        .default_width(1320)
        .default_height(860)
        .width_request(420)
        .height_request(480)
        .content(&split_view)
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
        fingerprint: Cell::new(0),
        rebuilding_sidebar: Cell::new(false),
        debounce_scheduled: Cell::new(false),
        ui_state: UiViewState::default(),
        split_view,
        sidebar_list: sidebar_list.clone(),
        content_title,
        error_banner,
        content_scroller,
        _monitor: RefCell::new(None),
    });

    // Seleção de item na sidebar
    {
        let ctrl_sel = ctrl.clone();
        sidebar_list.connect_row_selected(move |_, row| {
            if ctrl_sel.rebuilding_sidebar.get() {
                return;
            }
            let Some(row) = row else {
                return;
            };
            let slug = row.widget_name().to_string();
            if slug.is_empty() || slug == "__empty__" {
                return;
            }
            let changed = ctrl_sel.selected_slug.borrow().as_deref() != Some(slug.as_str());
            *ctrl_sel.selected_slug.borrow_mut() = Some(slug);
            ctrl_sel.split_view.set_show_content(true);
            render_selected_content(&ctrl_sel, !changed);
        });
    }

    // Filtro de busca na sidebar
    {
        let ctrl_search = ctrl.clone();
        search_entry.connect_search_changed(move |entry| {
            *ctrl_search.filter_query.borrow_mut() = entry.text().to_lowercase();
            rebuild_sidebar_rows(&ctrl_search);
        });
    }

    // Botão atualizar
    {
        let ctrl_ref = ctrl.clone();
        refresh_btn.connect_clicked(move |_| {
            reload_from_disk(&ctrl_ref, true);
        });
    }

    // Botão abrir diretório
    {
        let dir_for_open = projects_dir.clone();
        open_dir_btn.connect_clicked(move |_| {
            let uri = gio::File::for_path(&dir_for_open).uri();
            let _ = gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE);
        });
    }

    // GIO FileMonitor em tempo real
    let gdir = gio::File::for_path(&projects_dir);
    if let Ok(monitor) =
        gdir.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
    {
        let ctrl_mon = ctrl.clone();
        monitor.connect_changed(move |_, _, _, _| {
            schedule_debounced_reload(&ctrl_mon);
        });
        *ctrl._monitor.borrow_mut() = Some(monitor);
    }

    // Fallback leve de 2 segundos checando mtime/fingerprint
    {
        let ctrl_poll = Rc::downgrade(&ctrl);
        glib::timeout_add_seconds_local(2, move || {
            let Some(c) = ctrl_poll.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let fp = compute_dir_fingerprint(&c.projects_dir);
            if fp != c.fingerprint.get() {
                reload_from_disk(&c, false);
            }
            glib::ControlFlow::Continue
        });
    }

    reload_from_disk(&ctrl, true);
    window.present();

    // Suporte opcional para smoke test automatizado (fecha limpo após N ms)
    if let Ok(ms_str) = std::env::var("CLAUDE_PANORAMA_SMOKE_EXIT_MS") {
        if let Ok(ms) = ms_str.parse::<u64>() {
            let app_clone = app.clone();
            glib::timeout_add_local_once(Duration::from_millis(ms), move || {
                app_clone.quit();
            });
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

    let prev_slug = ctrl.selected_slug.borrow().clone();
    let next_slug = prev_slug
        .as_deref()
        .filter(|s| loaded.projects.iter().any(|p| p.slug == *s))
        .map(|s| s.to_string())
        .or_else(|| loaded.projects.first().map(|p| p.slug.clone()));

    let preserve_scroll = prev_slug == next_slug && prev_slug.is_some();
    *ctrl.projects.borrow_mut() = loaded.projects;
    *ctrl.selected_slug.borrow_mut() = next_slug;

    rebuild_sidebar_rows(ctrl);
    render_selected_content(ctrl, preserve_scroll);
}

fn rebuild_sidebar_rows(ctrl: &Rc<AppController>) {
    ctrl.rebuilding_sidebar.set(true);
    while let Some(child) = ctrl.sidebar_list.first_child() {
        ctrl.sidebar_list.remove(&child);
    }

    let query = ctrl.filter_query.borrow().clone();
    let selected = ctrl.selected_slug.borrow().clone();
    let projects = ctrl.projects.borrow();

    let mut row_to_select: Option<gtk4::ListBoxRow> = None;
    let mut count = 0usize;

    for proj in projects.iter() {
        if !query.is_empty() {
            let matches = proj.title.to_lowercase().contains(&query)
                || proj.slug.to_lowercase().contains(&query)
                || proj.path.to_lowercase().contains(&query)
                || proj.phase.to_lowercase().contains(&query)
                || proj.status.label().contains(&query);
            if !matches {
                continue;
            }
        }
        count += 1;
        let row = build_sidebar_row(proj);
        if selected.as_deref() == Some(proj.slug.as_str()) {
            row_to_select = Some(row.clone());
        }
        ctrl.sidebar_list.append(&row);
    }

    if count == 0 {
        let empty_lbl = gtk4::Label::new(Some("Nenhum projeto encontrado"));
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

fn build_sidebar_row(proj: &ProjectDocument) -> gtk4::ListBoxRow {
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 5);
    outer.add_css_class("sidebar-project-row");

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

    // Mini-indicadores de Progresso % e Confiança %
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

    gtk4::ListBoxRow::builder()
        .name(&proj.slug)
        .child(&outer)
        .build()
}

fn render_selected_content(ctrl: &Rc<AppController>, preserve_scroll: bool) {
    let saved_vscroll = if preserve_scroll {
        Some(ctrl.content_scroller.vadjustment().value())
    } else {
        None
    };

    let selected = ctrl.selected_slug.borrow().clone();
    let projects = ctrl.projects.borrow();
    let found = selected
        .as_deref()
        .and_then(|s| projects.iter().find(|p| p.slug == s));

    if let Some(doc) = found {
        let sub = if doc.phase.trim().is_empty() {
            doc.slug.clone()
        } else {
            format!("{} — {}", doc.slug, doc.phase)
        };
        ctrl.content_title.set_title(&doc.title);
        ctrl.content_title.set_subtitle(&sub);

        let view = render_project_view(doc, &ctrl.ui_state);
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
        ctrl.content_title.set_title("Claude Panorama");
        ctrl.content_title
            .set_subtitle("Nenhum projeto registrado");
        let empty = adw::StatusPage::builder()
            .icon_name("applications-engineering-symbolic")
            .title("Nenhum projeto registrado ainda")
            .description(format!(
                "Crie um projeto com:\npanorama init meu-projeto --title \"Meu Projeto\"\n\nDiretório monitorado:\n{}",
                ctrl.projects_dir.display()
            ))
            .vexpand(true)
            .build();
        ctrl.content_scroller.set_child(Some(&empty));
    }
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
