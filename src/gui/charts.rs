use crate::model::{
    confidence_semantic_rgb, palette_rgb, CellState, ChartBar, ChartSlice, FlowEdge, FlowNode,
    HistoryPoint, MatrixCell,
};
use gtk4::cairo;
use gtk4::gdk;
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeKind {
    Progresso,
    Confianca,
}

pub fn build_ring_gauge(percent: f64, kind: GaugeKind) -> gtk4::DrawingArea {
    let clamped = percent.clamp(0.0, 100.0);
    let area = gtk4::DrawingArea::builder()
        .content_width(136)
        .content_height(136)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .build();

    let style_mgr = adw::StyleManager::default();
    style_mgr.connect_dark_notify(glib::clone!(
        #[weak]
        area,
        move |_| {
            area.queue_draw();
        }
    ));

    area.set_draw_func(move |_, cr, width, height| {
        let dark = adw::StyleManager::default().is_dark();
        let w = width as f64;
        let h = height as f64;
        let cx = w * 0.5;
        let cy = h * 0.5;
        let radius = (w.min(h) * 0.5) - 11.0;
        let thickness = 11.0;

        let (accent_r, accent_g, accent_b) = match kind {
            GaugeKind::Progresso => {
                if clamped >= 99.95 {
                    (0.180, 0.761, 0.494)
                } else if dark {
                    (0.384, 0.627, 0.918) // #62a0ea
                } else {
                    (0.208, 0.518, 0.894) // #3584e4
                }
            }
            GaugeKind::Confianca => confidence_semantic_rgb(clamped),
        };

        // Fundo sutil interno
        cr.set_source_rgba(accent_r, accent_g, accent_b, if dark { 0.08 } else { 0.06 });
        cr.arc(cx, cy, radius - thickness * 0.65, 0.0, TAU);
        let _ = cr.fill();

        // Trilha circular
        if dark {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.12);
        } else {
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.10);
        }
        cr.set_line_width(thickness);
        cr.arc(cx, cy, radius, 0.0, TAU);
        let _ = cr.stroke();

        // Arco ativo
        if clamped > 0.05 {
            let start = -FRAC_PI_2;
            let end = start + TAU * (clamped / 100.0);
            cr.set_source_rgb(accent_r, accent_g, accent_b);
            cr.set_line_width(thickness);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.arc(cx, cy, radius, start, end);
            let _ = cr.stroke();
        }

        // Texto percentual centralizado
        let pct_str = format!("{clamped:.1}%");
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(21.0);
        if dark {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.96);
        } else {
            cr.set_source_rgba(0.10, 0.10, 0.12, 0.95);
        }
        if let Ok(ext) = cr.text_extents(&pct_str) {
            cr.move_to(cx - ext.width() * 0.5 - ext.x_bearing(), cy + 2.0);
            let _ = cr.show_text(&pct_str);
        }

        // Rótulo inferior no anel
        let caption = match kind {
            GaugeKind::Progresso => "PRONTO",
            GaugeKind::Confianca => "CONFIANÇA",
        };
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(9.5);
        if dark {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.56);
        } else {
            cr.set_source_rgba(0.12, 0.12, 0.15, 0.58);
        }
        if let Ok(ext) = cr.text_extents(caption) {
            cr.move_to(cx - ext.width() * 0.5 - ext.x_bearing(), cy + 19.0);
            let _ = cr.show_text(caption);
        }
    });

    area
}

pub fn build_sparkline(history: &[HistoryPoint]) -> gtk4::Widget {
    let container = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    container.set_margin_top(6);

    let pts: Vec<HistoryPoint> = history.to_vec();
    let latest = pts.last().cloned();

    let header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let title_lbl = gtk4::Label::new(Some(&format!(
        "Histórico ({} ponto{})",
        pts.len(),
        if pts.len() == 1 { "" } else { "s" }
    )));
    title_lbl.add_css_class("caption-heading");
    title_lbl.add_css_class("dim-label");
    title_lbl.set_xalign(0.0);
    title_lbl.set_hexpand(true);

    let legend_lbl = gtk4::Label::new(Some("— Confiança   -- Progresso"));
    legend_lbl.add_css_class("caption");
    legend_lbl.add_css_class("dim-label");
    header_box.append(&title_lbl);
    header_box.append(&legend_lbl);
    container.append(&header_box);

    let area = gtk4::DrawingArea::builder()
        .content_height(64)
        .hexpand(true)
        .build();

    let hover_idx: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));
    let readout = gtk4::Label::new(None);
    readout.add_css_class("caption");
    readout.add_css_class("dim-label");
    readout.set_xalign(0.0);
    readout.set_wrap(true);
    readout.set_wrap_mode(pango::WrapMode::WordChar);
    if let Some(last) = &latest {
        let note_suffix = if last.note.is_empty() {
            String::new()
        } else {
            format!(" · {}", last.note)
        };
        readout.set_text(&format!(
            "Último: {} — Confiança {:.1}% | Progresso {:.1}%{}",
            shorten_ts(&last.timestamp),
            last.confianca,
            last.progresso,
            note_suffix
        ));
    }

    let style_mgr = adw::StyleManager::default();
    style_mgr.connect_dark_notify(glib::clone!(
        #[weak]
        area,
        move |_| {
            area.queue_draw();
        }
    ));

    let pts_draw = pts.clone();
    let hover_draw = hover_idx.clone();
    area.set_draw_func(move |_, cr, width, height| {
        let dark = adw::StyleManager::default().is_dark();
        let w = width as f64;
        let h = height as f64;
        let pad_x = 8.0;
        let pad_y = 8.0;
        let plot_w = (w - pad_x * 2.0).max(10.0);
        let plot_h = (h - pad_y * 2.0).max(10.0);

        // Fundo arredondado da sparkline
        rounded_rect(cr, 0.5, 0.5, w - 1.0, h - 1.0, 8.0);
        if dark {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.04);
        } else {
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.035);
        }
        let _ = cr.fill();

        // Linhas guia em 75% e 45%
        cr.set_line_width(1.0);
        cr.set_dash(&[3.0, 3.0], 0.0);
        let y75 = pad_y + plot_h * (1.0 - 0.75);
        cr.set_source_rgba(0.180, 0.761, 0.494, 0.28);
        cr.move_to(pad_x, y75);
        cr.line_to(pad_x + plot_w, y75);
        let _ = cr.stroke();

        let y45 = pad_y + plot_h * (1.0 - 0.45);
        cr.set_source_rgba(0.898, 0.647, 0.039, 0.28);
        cr.move_to(pad_x, y45);
        cr.line_to(pad_x + plot_w, y45);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);

        if pts_draw.is_empty() {
            return;
        }

        let x_at = |i: usize| -> f64 {
            if pts_draw.len() <= 1 {
                pad_x + plot_w * 0.5
            } else {
                pad_x + plot_w * (i as f64 / (pts_draw.len() - 1) as f64)
            }
        };
        let y_at = |val: f64| -> f64 { pad_y + plot_h * (1.0 - val.clamp(0.0, 100.0) / 100.0) };

        // Traço secundário de Progresso (tracejado azul)
        if pts_draw.len() >= 2 {
            cr.set_dash(&[4.0, 3.0], 0.0);
            cr.set_line_width(1.6);
            if dark {
                cr.set_source_rgba(0.384, 0.627, 0.918, 0.72);
            } else {
                cr.set_source_rgba(0.208, 0.518, 0.894, 0.72);
            }
            for (i, p) in pts_draw.iter().enumerate() {
                let x = x_at(i);
                let y = y_at(p.progresso);
                if i == 0 {
                    cr.move_to(x, y);
                } else {
                    cr.line_to(x, y);
                }
            }
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
        }

        // Área preenchida e curva principal de Confiança
        let last_conf = pts_draw.last().map(|p| p.confianca).unwrap_or(50.0);
        let (cr_r, cr_g, cr_b) = confidence_semantic_rgb(last_conf);

        if pts_draw.len() >= 2 {
            cr.move_to(x_at(0), pad_y + plot_h);
            for (i, p) in pts_draw.iter().enumerate() {
                cr.line_to(x_at(i), y_at(p.confianca));
            }
            cr.line_to(x_at(pts_draw.len() - 1), pad_y + plot_h);
            cr.close_path();
            cr.set_source_rgba(cr_r, cr_g, cr_b, if dark { 0.16 } else { 0.13 });
            let _ = cr.fill();

            cr.set_line_width(2.2);
            cr.set_source_rgb(cr_r, cr_g, cr_b);
            for (i, p) in pts_draw.iter().enumerate() {
                let x = x_at(i);
                let y = y_at(p.confianca);
                if i == 0 {
                    cr.move_to(x, y);
                } else {
                    cr.line_to(x, y);
                }
            }
            let _ = cr.stroke();
        }

        // Pontos e destaque de hover
        let active_i = hover_draw.get().unwrap_or(pts_draw.len() - 1);
        for (i, p) in pts_draw.iter().enumerate() {
            let x = x_at(i);
            let y = y_at(p.confianca);
            let is_focus = i == active_i;
            if is_focus {
                cr.set_line_width(1.0);
                cr.set_source_rgba(cr_r, cr_g, cr_b, 0.40);
                cr.move_to(x, pad_y);
                cr.line_to(x, pad_y + plot_h);
                let _ = cr.stroke();
            }
            cr.set_source_rgb(cr_r, cr_g, cr_b);
            cr.arc(x, y, if is_focus { 4.2 } else { 2.5 }, 0.0, TAU);
            let _ = cr.fill();
        }
    });

    let motion = gtk4::EventControllerMotion::new();
    let pts_motion = pts.clone();
    let hover_motion = hover_idx.clone();
    let readout_motion = readout.clone();
    let area_motion = area.clone();
    motion.connect_motion(move |_, x, _| {
        if pts_motion.is_empty() {
            return;
        }
        let w = area_motion.width().max(20) as f64;
        let pad_x = 8.0;
        let plot_w = (w - pad_x * 2.0).max(10.0);
        let ratio = ((x - pad_x) / plot_w).clamp(0.0, 1.0);
        let idx = if pts_motion.len() <= 1 {
            0
        } else {
            (ratio * (pts_motion.len() - 1) as f64).round() as usize
        }
        .min(pts_motion.len() - 1);

        if hover_motion.get() != Some(idx) {
            hover_motion.set(Some(idx));
            let p = &pts_motion[idx];
            let note_suffix = if p.note.is_empty() {
                String::new()
            } else {
                format!(" · {}", p.note)
            };
            readout_motion.set_text(&format!(
                "[#{}/{}] {} — Confiança {:.1}% | Progresso {:.1}%{}",
                idx + 1,
                pts_motion.len(),
                shorten_ts(&p.timestamp),
                p.confianca,
                p.progresso,
                note_suffix
            ));
            area_motion.queue_draw();
        }
    });

    let hover_leave = hover_idx;
    let pts_leave = pts;
    let readout_leave = readout.clone();
    let area_leave = area.clone();
    motion.connect_leave(move |_| {
        hover_leave.set(None);
        if let Some(last) = pts_leave.last() {
            let note_suffix = if last.note.is_empty() {
                String::new()
            } else {
                format!(" · {}", last.note)
            };
            readout_leave.set_text(&format!(
                "Último: {} — Confiança {:.1}% | Progresso {:.1}%{}",
                shorten_ts(&last.timestamp),
                last.confianca,
                last.progresso,
                note_suffix
            ));
        }
        area_leave.queue_draw();
    });
    area.add_controller(motion);

    container.append(&area);
    container.append(&readout);
    container.upcast()
}

#[derive(Clone, Copy)]
struct RectBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl RectBox {
    fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

pub fn build_matrix_widget(
    title: Option<&str>,
    subtitle: Option<&str>,
    columns_hint: Option<u32>,
    cells: &[MatrixCell],
) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");

    // Cabeçalho + legenda de contagem por estado
    let top_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let title_col = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    title_col.set_hexpand(true);

    if let Some(t) = title.filter(|s| !s.trim().is_empty()) {
        let lbl = gtk4::Label::new(Some(t));
        lbl.add_css_class("heading");
        lbl.set_xalign(0.0);
        lbl.set_wrap(true);
        lbl.set_wrap_mode(pango::WrapMode::WordChar);
        title_col.append(&lbl);
    }
    let sub_txt = subtitle
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Passe o mouse ou clique em uma célula para inspecionar os detalhes.");
    let sub_lbl = gtk4::Label::new(Some(sub_txt));
    sub_lbl.add_css_class("caption");
    sub_lbl.add_css_class("dim-label");
    sub_lbl.set_xalign(0.0);
    sub_lbl.set_wrap(true);
    sub_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    title_col.append(&sub_lbl);
    top_row.append(&title_col);

    let counts_box = build_state_summary_pills(cells.iter().map(|c| c.state));
    top_row.append(&counts_box);
    card.append(&top_row);

    let cells_vec: Rc<Vec<MatrixCell>> = Rc::new(cells.to_vec());
    let cell_rects: Rc<RefCell<Vec<RectBox>>> = Rc::new(RefCell::new(Vec::new()));
    let hovered: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));
    let selected: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));
    let last_height: Rc<Cell<i32>> = Rc::new(Cell::new(0));

    let init_cols = columns_hint.unwrap_or(4).clamp(1, 12) as usize;
    let init_rows = cells_vec.len().div_ceil(init_cols).max(1);
    let cell_h = 54;
    let gap = 8;
    let init_h = (init_rows as i32) * cell_h + ((init_rows.saturating_sub(1)) as i32) * gap + 8;
    last_height.set(init_h);

    let area = gtk4::DrawingArea::builder()
        .content_height(init_h)
        .hexpand(true)
        .build();

    let style_mgr = adw::StyleManager::default();
    style_mgr.connect_dark_notify(glib::clone!(
        #[weak]
        area,
        move |_| {
            area.queue_draw();
        }
    ));

    let compute_cols = move |width: i32| -> usize {
        if let Some(c) = columns_hint {
            let max_fit = ((width - 8).max(120) / 108).max(1) as usize;
            (c as usize).clamp(1, max_fit.max(1))
        } else {
            (((width - 8).max(140)) / 148).clamp(2, 8) as usize
        }
    };

    let cells_resize = cells_vec.clone();
    let last_h_resize = last_height.clone();
    area.connect_resize(move |da, width, _| {
        if width <= 20 {
            return;
        }
        let cols = compute_cols(width);
        let rows = cells_resize.len().div_ceil(cols).max(1);
        let needed_h = (rows as i32) * cell_h + ((rows.saturating_sub(1)) as i32) * gap + 8;
        if last_h_resize.get() != needed_h {
            last_h_resize.set(needed_h);
            let da_clone = da.clone();
            glib::idle_add_local_once(move || {
                da_clone.set_content_height(needed_h);
            });
        }
    });

    let cells_draw = cells_vec.clone();
    let rects_draw = cell_rects.clone();
    let hov_draw = hovered.clone();
    let sel_draw = selected.clone();
    area.set_draw_func(move |_, cr, width, _| {
        let dark = adw::StyleManager::default().is_dark();
        let w = width.max(140) as f64;
        let cols = compute_cols(width);
        let gap_f = gap as f64;
        let cw = ((w - gap_f * ((cols.saturating_sub(1)) as f64) - 4.0) / (cols as f64)).max(72.0);
        let ch = cell_h as f64;

        let mut rects = Vec::with_capacity(cells_draw.len());
        for (idx, cell) in cells_draw.iter().enumerate() {
            let r = idx / cols;
            let c = idx % cols;
            let x = 2.0 + (c as f64) * (cw + gap_f);
            let y = 4.0 + (r as f64) * (ch + gap_f);
            rects.push(RectBox {
                x,
                y,
                w: cw,
                h: ch,
            });

            let (sr, sg, sb) = cell.state.rgb(dark);
            let is_hover = hov_draw.get() == Some(idx);
            let is_sel = sel_draw.get() == Some(idx);

            // Preenchimento da célula
            rounded_rect(cr, x, y, cw, ch, 8.0);
            let fill_alpha = if is_sel {
                0.24
            } else if is_hover {
                0.18
            } else if dark {
                0.11
            } else {
                0.09
            };
            cr.set_source_rgba(sr, sg, sb, fill_alpha);
            let _ = cr.fill_preserve();

            // Borda da célula
            cr.set_line_width(if is_sel {
                2.0
            } else if is_hover {
                1.5
            } else {
                1.0
            });
            cr.set_source_rgba(
                sr,
                sg,
                sb,
                if is_sel || is_hover { 0.90 } else { 0.42 },
            );
            let _ = cr.stroke();

            // Indicador circular + tag curta no canto superior
            cr.arc(x + 11.0, y + 14.0, 4.0, 0.0, TAU);
            cr.set_source_rgb(sr, sg, sb);
            let _ = cr.fill();

            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(9.5);
            cr.set_source_rgba(sr, sg, sb, 0.95);
            let tag = cell.state.short_tag();
            if let Ok(ext) = cr.text_extents(tag) {
                cr.move_to(x + cw - ext.width() - 8.0, y + 17.0);
                let _ = cr.show_text(tag);
            }

            // Rótulo principal
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(11.5);
            if dark {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.95);
            } else {
                cr.set_source_rgba(0.10, 0.10, 0.12, 0.95);
            }
            let lbl_trunc = fit_text_width(cr, &cell.label, (cw - 52.0).max(28.0));
            cr.move_to(x + 20.0, y + 18.0);
            let _ = cr.show_text(&lbl_trunc);

            // Subrótulo / resumo curto
            let sub_source = if !cell.sublabel.is_empty() {
                cell.sublabel.as_str()
            } else {
                cell.detail.as_str()
            };
            if !sub_source.is_empty() {
                cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                cr.set_font_size(10.0);
                if dark {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.66);
                } else {
                    cr.set_source_rgba(0.12, 0.12, 0.15, 0.68);
                }
                let sub_trunc = fit_text_width(cr, sub_source, (cw - 16.0).max(28.0));
                cr.move_to(x + 8.0, y + 38.0);
                let _ = cr.show_text(&sub_trunc);
            }
        }
        *rects_draw.borrow_mut() = rects;
    });

    // Painel compacto de inspeção abaixo da matriz + Popover ao clicar
    let inspector_lbl = gtk4::Label::new(Some(
        "Clique num quadrado para ver o detalhe.",
    ));
    inspector_lbl.add_css_class("caption");
    inspector_lbl.add_css_class("dim-label");
    inspector_lbl.set_xalign(0.0);
    inspector_lbl.set_wrap(true);
    inspector_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    inspector_lbl.set_selectable(true);

    let (popover, pop_title, pop_state, pop_body) = build_detail_popover(&area);

    let motion = gtk4::EventControllerMotion::new();
    let rects_m = cell_rects.clone();
    let cells_m = cells_vec.clone();
    let hov_m = hovered.clone();
    let sel_m = selected.clone();
    let insp_m = inspector_lbl.clone();
    let area_m = area.clone();
    motion.connect_motion(move |_, mx, my| {
        let found = rects_m
            .borrow()
            .iter()
            .position(|r| r.contains(mx, my));
        if hov_m.get() != found {
            hov_m.set(found);
            if let Some(idx) = found {
                let c = &cells_m[idx];
                let detail = if c.detail.is_empty() {
                    c.sublabel.as_str()
                } else {
                    c.detail.as_str()
                };
                area_m.set_tooltip_text(Some(&format!(
                    "{} [{}]\n{}",
                    c.label,
                    c.state.label_pt(),
                    detail
                )));
                if sel_m.get().is_none() {
                    insp_m.set_text(&format!(
                        "{} ({}) — {}",
                        c.label,
                        c.state.label_pt(),
                        if detail.is_empty() { "Sem detalhe adicional." } else { detail }
                    ));
                }
            } else {
                area_m.set_tooltip_text(None);
            }
            area_m.queue_draw();
        }
    });
    let hov_leave = hovered;
    let area_leave = area.clone();
    motion.connect_leave(move |_| {
        hov_leave.set(None);
        area_leave.queue_draw();
    });
    area.add_controller(motion);

    let click = gtk4::GestureClick::new();
    let rects_c = cell_rects;
    let cells_c = cells_vec;
    let sel_c = selected;
    let insp_c = inspector_lbl.clone();
    let area_c = area.clone();
    click.connect_released(move |_, _, cx, cy| {
        let hit = rects_c
            .borrow()
            .iter()
            .copied()
            .enumerate()
            .find(|(_, r)| r.contains(cx, cy));
        if let Some((idx, rect)) = hit {
            sel_c.set(Some(idx));
            let c = &cells_c[idx];
            let detail = if !c.detail.is_empty() {
                c.detail.clone()
            } else if !c.sublabel.is_empty() {
                c.sublabel.clone()
            } else {
                "Sem detalhe adicional informado pelo agente.".to_string()
            };
            insp_c.set_text(&format!(
                "Selecionado: {} ({}) — {}",
                c.label,
                c.state.label_pt(),
                detail
            ));
            pop_title.set_text(&c.label);
            let sub_meta = if c.sublabel.is_empty() {
                c.state.label_pt().to_string()
            } else {
                format!("{} · {}", c.state.label_pt(), c.sublabel)
            };
            pop_state.set_text(&sub_meta);
            pop_body.set_text(&detail);
            popover.set_pointing_to(Some(&gdk::Rectangle::new(
                rect.x as i32,
                rect.y as i32,
                rect.w.max(8.0) as i32,
                rect.h.max(8.0) as i32,
            )));
            popover.popup();
            area_c.queue_draw();
        }
    });
    area.add_controller(click);

    card.append(&area);
    card.append(&inspector_lbl);
    card.upcast()
}

pub fn build_flow_graph_widget(
    title: Option<&str>,
    subtitle: Option<&str>,
    nodes: &[FlowNode],
    edges: &[FlowEdge],
) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");

    let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let title_col = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    title_col.set_hexpand(true);
    if let Some(t) = title.filter(|s| !s.trim().is_empty()) {
        let lbl = gtk4::Label::new(Some(t));
        lbl.add_css_class("heading");
        lbl.set_xalign(0.0);
        lbl.set_wrap(true);
        lbl.set_wrap_mode(pango::WrapMode::WordChar);
        title_col.append(&lbl);
    }
    let sub_txt = subtitle
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Diagrama vetorial Cairo — clique em qualquer nó para inspecionar.");
    let sub_lbl = gtk4::Label::new(Some(sub_txt));
    sub_lbl.add_css_class("caption");
    sub_lbl.add_css_class("dim-label");
    sub_lbl.set_xalign(0.0);
    sub_lbl.set_wrap(true);
    sub_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    title_col.append(&sub_lbl);
    header.append(&title_col);
    header.append(&build_state_summary_pills(nodes.iter().map(|n| n.state)));
    card.append(&header);

    // Agrupar nós por coluna (estágio)
    let mut cols_map: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut max_col = 0usize;
    for (idx, n) in nodes.iter().enumerate() {
        max_col = max_col.max(n.col);
        cols_map.entry(n.col).or_default().push(idx);
    }
    let mut active_cols: Vec<usize> = (0..=max_col)
        .filter(|c| cols_map.contains_key(c))
        .collect();
    if active_cols.is_empty() {
        active_cols.push(0);
    }
    let max_rows = active_cols
        .iter()
        .map(|c| cols_map.get(c).map(|v| v.len()).unwrap_or(0))
        .max()
        .unwrap_or(1)
        .max(1);

    let node_w = 176.0_f64;
    let node_h = 62.0_f64;
    let col_gap = 64.0_f64;
    let row_gap = 20.0_f64;
    let pad_x = 18.0_f64;
    let pad_y = 18.0_f64;

    let min_canvas_w = (pad_x * 2.0
        + (active_cols.len() as f64) * node_w
        + (active_cols.len().saturating_sub(1) as f64) * col_gap)
        .ceil() as i32;
    let canvas_h = (pad_y * 2.0
        + (max_rows as f64) * node_h
        + (max_rows.saturating_sub(1) as f64) * row_gap)
        .ceil()
        .max(110.0) as i32;

    let area = gtk4::DrawingArea::builder()
        .content_width(min_canvas_w)
        .content_height(canvas_h)
        .hexpand(true)
        .build();

    let style_mgr = adw::StyleManager::default();
    style_mgr.connect_dark_notify(glib::clone!(
        #[weak]
        area,
        move |_| {
            area.queue_draw();
        }
    ));

    let nodes_rc = Rc::new(nodes.to_vec());
    let edges_rc = Rc::new(edges.to_vec());
    let node_rects: Rc<RefCell<Vec<RectBox>>> =
        Rc::new(RefCell::new(vec![
            RectBox {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0
            };
            nodes.len()
        ]));
    let hovered_node: Rc<Cell<Option<usize>>> = Rc::new(Cell::new(None));

    let nodes_draw = nodes_rc.clone();
    let edges_draw = edges_rc.clone();
    let rects_draw = node_rects.clone();
    let hov_draw = hovered_node.clone();
    area.set_draw_func(move |_, cr, width, height| {
        let dark = adw::StyleManager::default().is_dark();
        let w = (width as f64).max(min_canvas_w as f64);
        let h = (height as f64).max(canvas_h as f64);

        // Fundo técnico com grade sutil
        rounded_rect(cr, 1.0, 1.0, w - 2.0, h - 2.0, 10.0);
        if dark {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.03);
        } else {
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.025);
        }
        let _ = cr.fill();

        let num_cols = active_cols.len();
        let effective_col_gap = if num_cols > 1 {
            ((w - pad_x * 2.0 - (num_cols as f64) * node_w) / ((num_cols - 1) as f64)).max(col_gap)
        } else {
            0.0
        };

        let mut computed = vec![
            RectBox {
                x: 0.0,
                y: 0.0,
                w: node_w,
                h: node_h,
            };
            nodes_draw.len()
        ];
        let mut id_to_idx: HashMap<&str, usize> = HashMap::new();

        for (col_pos, orig_col) in active_cols.iter().enumerate() {
            let Some(indices) = cols_map.get(orig_col) else {
                continue;
            };
            let col_total_h = (indices.len() as f64) * node_h
                + (indices.len().saturating_sub(1) as f64) * row_gap;
            let start_y = ((h - col_total_h) * 0.5).max(pad_y);
            let x = if num_cols == 1 {
                ((w - node_w) * 0.5).max(pad_x)
            } else {
                pad_x + (col_pos as f64) * (node_w + effective_col_gap)
            };
            for (row_pos, &n_idx) in indices.iter().enumerate() {
                let y = start_y + (row_pos as f64) * (node_h + row_gap);
                computed[n_idx] = RectBox {
                    x,
                    y,
                    w: node_w,
                    h: node_h,
                };
                id_to_idx.insert(nodes_draw[n_idx].id.as_str(), n_idx);
            }
        }

        // Desenhar arestas direcionadas primeiro (abaixo dos nós)
        for edge in edges_draw.iter() {
            let (Some(&from_i), Some(&to_i)) = (
                id_to_idx.get(edge.from.as_str()),
                id_to_idx.get(edge.to.as_str()),
            ) else {
                continue;
            };
            let a = computed[from_i];
            let b = computed[to_i];

            let (sx, sy, tx, ty, c1x, c1y, c2x, c2y, angle);
            if b.x > a.x + a.w * 0.5 {
                sx = a.x + a.w;
                sy = a.y + a.h * 0.5;
                tx = b.x;
                ty = b.y + b.h * 0.5;
                let dx = ((tx - sx) * 0.48).max(28.0);
                c1x = sx + dx;
                c1y = sy;
                c2x = tx - dx;
                c2y = ty;
                angle = (ty - c2y).atan2(tx - c2x);
            } else if (b.x - a.x).abs() <= 4.0 {
                // Mesma coluna: conexão vertical
                sx = a.x + a.w * 0.5;
                if b.y >= a.y {
                    sy = a.y + a.h;
                    tx = b.x + b.w * 0.5;
                    ty = b.y;
                    angle = FRAC_PI_2;
                } else {
                    sy = a.y;
                    tx = b.x + b.w * 0.5;
                    ty = b.y + b.h;
                    angle = -FRAC_PI_2;
                }
                c1x = sx;
                c1y = (sy + ty) * 0.5;
                c2x = tx;
                c2y = (sy + ty) * 0.5;
            } else {
                // Retorno para estágio anterior: arco inferior suave
                sx = a.x;
                sy = a.y + a.h * 0.72;
                tx = b.x + b.w;
                ty = b.y + b.h * 0.72;
                let lift = 34.0;
                c1x = sx - 36.0;
                c1y = sy + lift;
                c2x = tx + 36.0;
                c2y = ty + lift;
                angle = (ty - c2y).atan2(tx - c2x);
            }

            if dark {
                cr.set_source_rgba(0.75, 0.82, 0.94, 0.58);
            } else {
                cr.set_source_rgba(0.22, 0.30, 0.44, 0.58);
            }
            cr.set_line_width(1.8);
            cr.move_to(sx, sy);
            cr.curve_to(c1x, c1y, c2x, c2y, tx, ty);
            let _ = cr.stroke();

            // Seta direcional
            let arrow_len = 8.5;
            let spread = 0.45;
            cr.move_to(tx, ty);
            cr.line_to(
                tx - arrow_len * (angle - spread).cos(),
                ty - arrow_len * (angle - spread).sin(),
            );
            cr.line_to(
                tx - arrow_len * (angle + spread).cos(),
                ty - arrow_len * (angle + spread).sin(),
            );
            cr.close_path();
            let _ = cr.fill();

            // Rótulo de aresta no ponto médio da curva Bézier (t = 0.5)
            if !edge.label.is_empty() {
                let mx = 0.125 * sx + 0.375 * c1x + 0.375 * c2x + 0.125 * tx;
                let my = 0.125 * sy + 0.375 * c1y + 0.375 * c2y + 0.125 * ty;
                cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
                cr.set_font_size(9.5);
                let lbl = fit_text_width(cr, &edge.label, 110.0);
                if let Ok(ext) = cr.text_extents(&lbl) {
                    let pw = ext.width() + 10.0;
                    let ph = 16.0;
                    rounded_rect(cr, mx - pw * 0.5, my - ph * 0.5, pw, ph, 6.0);
                    if dark {
                        cr.set_source_rgba(0.14, 0.15, 0.18, 0.92);
                    } else {
                        cr.set_source_rgba(0.96, 0.97, 0.99, 0.94);
                    }
                    let _ = cr.fill_preserve();
                    if dark {
                        cr.set_source_rgba(1.0, 1.0, 1.0, 0.18);
                    } else {
                        cr.set_source_rgba(0.0, 0.0, 0.0, 0.15);
                    }
                    cr.set_line_width(1.0);
                    let _ = cr.stroke();

                    if dark {
                        cr.set_source_rgba(0.92, 0.94, 0.98, 0.92);
                    } else {
                        cr.set_source_rgba(0.16, 0.18, 0.24, 0.92);
                    }
                    cr.move_to(mx - ext.width() * 0.5 - ext.x_bearing(), my + 3.5);
                    let _ = cr.show_text(&lbl);
                }
            }
        }

        // Desenhar nós
        for (idx, node) in nodes_draw.iter().enumerate() {
            let r = computed[idx];
            let (sr, sg, sb) = node.state.rgb(dark);
            let is_hov = hov_draw.get() == Some(idx);

            rounded_rect(cr, r.x, r.y, r.w, r.h, 9.0);
            if dark {
                cr.set_source_rgba(0.18, 0.19, 0.22, if is_hov { 0.98 } else { 0.93 });
            } else {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.97);
            }
            let _ = cr.fill_preserve();

            cr.set_line_width(if is_hov { 2.0 } else { 1.2 });
            cr.set_source_rgba(sr, sg, sb, if is_hov { 0.95 } else { 0.58 });
            let _ = cr.stroke();

            // Barra lateral semântica esquerda
            cr.save().ok();
            rounded_rect(cr, r.x, r.y, r.w, r.h, 9.0);
            cr.clip();
            cr.rectangle(r.x, r.y, 6.0, r.h);
            cr.set_source_rgb(sr, sg, sb);
            let _ = cr.fill();
            cr.restore().ok();

            // Tag de estado no canto superior direito
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(9.0);
            let tag = node.state.short_tag();
            cr.set_source_rgba(sr, sg, sb, 0.95);
            if let Ok(ext) = cr.text_extents(tag) {
                cr.move_to(r.x + r.w - ext.width() - 8.0, r.y + 16.0);
                let _ = cr.show_text(tag);
            }

            // Título do nó
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(11.5);
            if dark {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.96);
            } else {
                cr.set_source_rgba(0.08, 0.09, 0.11, 0.96);
            }
            let title_fit = fit_text_width(cr, &node.label, r.w - 50.0);
            cr.move_to(r.x + 13.0, r.y + 23.0);
            let _ = cr.show_text(&title_fit);

            // Subtítulo do nó
            if !node.sublabel.is_empty() {
                cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
                cr.set_font_size(10.0);
                if dark {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.68);
                } else {
                    cr.set_source_rgba(0.15, 0.16, 0.20, 0.72);
                }
                let sub_fit = fit_text_width(cr, &node.sublabel, r.w - 22.0);
                cr.move_to(r.x + 13.0, r.y + 44.0);
                let _ = cr.show_text(&sub_fit);
            }
        }

        *rects_draw.borrow_mut() = computed;
    });

    let (popover, pop_title, pop_state, pop_body) = build_detail_popover(&area);

    let motion = gtk4::EventControllerMotion::new();
    let rects_m = node_rects.clone();
    let nodes_m = nodes_rc.clone();
    let hov_m = hovered_node.clone();
    let area_m = area.clone();
    motion.connect_motion(move |_, mx, my| {
        let hit = rects_m.borrow().iter().position(|r| r.contains(mx, my));
        if hov_m.get() != hit {
            hov_m.set(hit);
            if let Some(i) = hit {
                let n = &nodes_m[i];
                let d = if n.detail.is_empty() {
                    n.sublabel.as_str()
                } else {
                    n.detail.as_str()
                };
                area_m.set_tooltip_text(Some(&format!(
                    "{} [{}]\n{}",
                    n.label,
                    n.state.label_pt(),
                    d
                )));
            } else {
                area_m.set_tooltip_text(None);
            }
            area_m.queue_draw();
        }
    });
    let hov_l = hovered_node;
    let area_l = area.clone();
    motion.connect_leave(move |_| {
        hov_l.set(None);
        area_l.queue_draw();
    });
    area.add_controller(motion);

    let click = gtk4::GestureClick::new();
    let rects_c = node_rects;
    let nodes_c = nodes_rc;
    click.connect_released(move |_, _, cx, cy| {
        let hit = rects_c
            .borrow()
            .iter()
            .copied()
            .enumerate()
            .find(|(_, r)| r.contains(cx, cy));
        if let Some((idx, rect)) = hit {
            let n = &nodes_c[idx];
            pop_title.set_text(&n.label);
            let meta = if n.sublabel.is_empty() {
                n.state.label_pt().to_string()
            } else {
                format!("{} · {}", n.state.label_pt(), n.sublabel)
            };
            pop_state.set_text(&meta);
            pop_body.set_text(if n.detail.is_empty() {
                "Sem observações adicionais registradas neste nó."
            } else {
                &n.detail
            });
            popover.set_pointing_to(Some(&gdk::Rectangle::new(
                rect.x as i32,
                rect.y as i32,
                rect.w as i32,
                rect.h as i32,
            )));
            popover.popup();
        }
    });
    area.add_controller(click);

    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .hexpand(true)
        .child(&area)
        .build();

    card.append(&scroller);
    card.upcast()
}

fn build_detail_popover(
    parent_area: &gtk4::DrawingArea,
) -> (gtk4::Popover, gtk4::Label, gtk4::Label, gtk4::Label) {
    let pop_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    pop_box.set_margin_top(10);
    pop_box.set_margin_bottom(10);
    pop_box.set_margin_start(12);
    pop_box.set_margin_end(12);

    let title_lbl = gtk4::Label::new(None);
    title_lbl.add_css_class("heading");
    title_lbl.set_xalign(0.0);
    title_lbl.set_wrap(true);
    title_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    title_lbl.set_max_width_chars(48);

    let state_lbl = gtk4::Label::new(None);
    state_lbl.add_css_class("caption-heading");
    state_lbl.add_css_class("dim-label");
    state_lbl.set_xalign(0.0);
    state_lbl.set_wrap(true);
    state_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    state_lbl.set_max_width_chars(48);

    let body_lbl = gtk4::Label::new(None);
    body_lbl.set_xalign(0.0);
    body_lbl.set_wrap(true);
    body_lbl.set_wrap_mode(pango::WrapMode::WordChar);
    body_lbl.set_max_width_chars(52);
    body_lbl.set_selectable(true);

    pop_box.append(&title_lbl);
    pop_box.append(&state_lbl);
    pop_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    pop_box.append(&body_lbl);

    let popover = gtk4::Popover::builder()
        .has_arrow(true)
        .autohide(true)
        .child(&pop_box)
        .build();
    popover.set_parent(parent_area);

    parent_area.connect_destroy({
        let popover = popover.clone();
        move |_| {
            popover.unparent();
        }
    });

    (popover, title_lbl, state_lbl, body_lbl)
}

fn build_state_summary_pills(states: impl Iterator<Item = CellState>) -> gtk4::Box {
    let mut pass = 0usize;
    let mut run = 0usize;
    let mut fail = 0usize;
    let mut pend = 0usize;
    let mut bloc = 0usize;
    let mut resv = 0usize;
    for s in states {
        match s {
            CellState::Pass => pass += 1,
            CellState::Running => run += 1,
            CellState::Fail => fail += 1,
            CellState::Pending => pend += 1,
            CellState::Blocked => bloc += 1,
            CellState::Reserved => resv += 1,
        }
    }

    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    row.set_valign(gtk4::Align::Start);
    for (count, tag, css) in [
        (pass, "pronto", "pill-pass"),
        (run, "em andamento", "pill-run"),
        (fail, "com falha", "pill-fail"),
        (bloc, "travado", "pill-bloc"),
        (pend, "a fazer", "pill-pend"),
        (resv, "reservado", "pill-resv"),
    ] {
        if count > 0 {
            let l = gtk4::Label::new(Some(&format!("{count} {tag}")));
            l.add_css_class("mini-pill");
            l.add_css_class(css);
            row.append(&l);
        }
    }
    row
}

fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, PI + FRAC_PI_2);
    cr.close_path();
}

fn fit_text_width(cr: &cairo::Context, text: &str, max_w: f64) -> String {
    if text.is_empty() {
        return String::new();
    }
    if let Ok(ext) = cr.text_extents(text) {
        if ext.width() <= max_w {
            return text.to_string();
        }
    }
    let chars: Vec<char> = text.chars().collect();
    let mut hi = chars.len();
    while hi > 1 {
        hi -= 1;
        let candidate: String = chars[..hi].iter().collect::<String>() + "…";
        if let Ok(ext) = cr.text_extents(&candidate) {
            if ext.width() <= max_w {
                return candidate;
            }
        }
    }
    chars.first().map(|c| c.to_string()).unwrap_or_default()
}

fn shorten_ts(ts: &str) -> String {
    if ts.len() >= 16 {
        ts.chars().take(16).collect::<String>().replace('T', " ")
    } else {
        ts.to_string()
    }
}


/// Cor de uma fatia/barra: `color` explícito da paleta GNOME, senão a cor semântica do `state`.
pub fn series_rgb(color: Option<&str>, state: CellState, dark: bool) -> (f64, f64, f64) {
    color.and_then(palette_rgb).unwrap_or_else(|| state.rgb(dark))
}

fn redraw_on_theme_change(area: &gtk4::DrawingArea) {
    adw::StyleManager::default().connect_dark_notify(glib::clone!(
        #[weak]
        area,
        move |_| area.queue_draw()
    ));
}

fn block_header(card: &gtk4::Box, title: Option<&str>, subtitle: Option<&str>) {
    if let Some(t) = title.filter(|s| !s.trim().is_empty()) {
        let l = gtk4::Label::new(Some(t));
        l.add_css_class("heading");
        l.set_xalign(0.0);
        l.set_wrap(true);
        l.set_wrap_mode(pango::WrapMode::WordChar);
        card.append(&l);
    }
    if let Some(st) = subtitle.filter(|s| !s.trim().is_empty()) {
        let l = gtk4::Label::new(Some(st));
        l.add_css_class("caption");
        l.add_css_class("dim-label");
        l.set_xalign(0.0);
        l.set_wrap(true);
        l.set_wrap_mode(pango::WrapMode::WordChar);
        card.append(&l);
    }
}

fn color_swatch(rgb_fn: impl Fn(bool) -> (f64, f64, f64) + 'static) -> gtk4::DrawingArea {
    let sw = gtk4::DrawingArea::builder()
        .content_width(12)
        .content_height(12)
        .valign(gtk4::Align::Center)
        .build();
    redraw_on_theme_change(&sw);
    sw.set_draw_func(move |_, cr, w, h| {
        let (r, g, b) = rgb_fn(adw::StyleManager::default().is_dark());
        cr.set_source_rgb(r, g, b);
        rounded_rect(cr, 0.0, 0.0, w as f64, h as f64, 3.0);
        let _ = cr.fill();
    });
    sw
}

fn format_number(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        let raw = format!("{}", v.round() as i64);
        // Separador de milhar pt-BR
        let (sign, digits) = raw.strip_prefix('-').map_or(("", raw.as_str()), |d| ("-", d));
        let mut out = String::new();
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push('.');
            }
            out.push(ch);
        }
        format!("{sign}{out}")
    } else {
        format!("{v:.1}").replace('.', ",")
    }
}

/// Gráfico de rosca: mostra como um total se divide em partes.
pub fn build_donut_widget(
    title: Option<&str>,
    subtitle: Option<&str>,
    center_value: Option<&str>,
    center_label: Option<&str>,
    slices: &[ChartSlice],
) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");
    block_header(&card, title, subtitle);

    let total: f64 = slices.iter().map(|s| s.value).sum();
    let body = gtk4::Box::new(gtk4::Orientation::Horizontal, 18);

    let area = gtk4::DrawingArea::builder()
        .content_width(170)
        .content_height(170)
        .valign(gtk4::Align::Center)
        .build();
    redraw_on_theme_change(&area);
    let data: Vec<(f64, CellState, Option<String>)> = slices
        .iter()
        .map(|s| (s.value, s.state, s.color.clone()))
        .collect();
    let center_value = center_value
        .map(str::to_string)
        .unwrap_or_else(|| format_number(total));
    let center_label = center_label.unwrap_or("total").to_string();
    area.set_draw_func(move |_, cr, w, h| {
        let dark = adw::StyleManager::default().is_dark();
        let (cx, cy) = (w as f64 * 0.5, h as f64 * 0.5);
        let thickness = 22.0;
        let radius = (w.min(h) as f64) * 0.5 - thickness * 0.5 - 2.0;
        let gap = if data.iter().filter(|d| d.0 > 0.0).count() > 1 { 0.025 } else { 0.0 };
        let mut angle = -FRAC_PI_2;
        cr.set_line_width(thickness);
        for (value, state, color) in &data {
            if *value <= 0.0 || total <= 0.0 {
                continue;
            }
            let sweep = TAU * value / total;
            let (r, g, b) = series_rgb(color.as_deref(), *state, dark);
            cr.set_source_rgb(r, g, b);
            let start = angle + gap * 0.5;
            let end = (angle + sweep - gap * 0.5).max(start + 0.001);
            cr.arc(cx, cy, radius, start, end);
            let _ = cr.stroke();
            angle += sweep;
        }
        let fg = if dark { (1.0, 1.0, 1.0) } else { (0.10, 0.10, 0.12) };
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(22.0);
        cr.set_source_rgba(fg.0, fg.1, fg.2, 0.95);
        let txt = fit_text_width(cr, &center_value, radius * 1.4);
        if let Ok(ext) = cr.text_extents(&txt) {
            cr.move_to(cx - ext.width() * 0.5 - ext.x_bearing(), cy + 4.0);
            let _ = cr.show_text(&txt);
        }
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
        cr.set_font_size(11.0);
        cr.set_source_rgba(fg.0, fg.1, fg.2, 0.6);
        let lbl = fit_text_width(cr, &center_label, radius * 1.4);
        if let Ok(ext) = cr.text_extents(&lbl) {
            cr.move_to(cx - ext.width() * 0.5 - ext.x_bearing(), cy + 21.0);
            let _ = cr.show_text(&lbl);
        }
    });
    body.append(&area);

    let legend = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    legend.set_hexpand(true);
    legend.set_valign(gtk4::Align::Center);
    for s in slices {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let (color, state) = (s.color.clone(), s.state);
        row.append(&color_swatch(move |dark| series_rgb(color.as_deref(), state, dark)));

        let text_col = gtk4::Box::new(gtk4::Orientation::Vertical, 1);
        text_col.set_hexpand(true);
        let name = gtk4::Label::new(Some(&s.label));
        name.set_xalign(0.0);
        name.set_wrap(true);
        name.set_wrap_mode(pango::WrapMode::WordChar);
        text_col.append(&name);
        if !s.detail.trim().is_empty() {
            let det = gtk4::Label::new(Some(&s.detail));
            det.add_css_class("caption");
            det.add_css_class("dim-label");
            det.set_xalign(0.0);
            det.set_wrap(true);
            det.set_wrap_mode(pango::WrapMode::WordChar);
            text_col.append(&det);
        }
        row.append(&text_col);

        let pct = if total > 0.0 { s.value / total * 100.0 } else { 0.0 };
        let val = gtk4::Label::new(Some(&format!(
            "{} · {}%",
            format_number(s.value),
            format_number((pct * 10.0).round() / 10.0)
        )));
        val.add_css_class("caption-heading");
        val.add_css_class("numeric");
        val.set_valign(gtk4::Align::Start);
        row.append(&val);
        legend.append(&row);
    }
    body.append(&legend);
    card.append(&body);
    card.upcast()
}

/// Barras horizontais com valores absolutos (contagens, horas, arquivos...).
pub fn build_bar_chart_widget(
    title: Option<&str>,
    subtitle: Option<&str>,
    unit: Option<&str>,
    max: Option<f64>,
    bars: &[ChartBar],
) -> gtk4::Widget {
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.add_css_class("panorama-block-card");
    block_header(&card, title, subtitle);

    let scale_max = max
        .unwrap_or_else(|| bars.iter().map(|b| b.value).fold(0.0, f64::max))
        .max(f64::MIN_POSITIVE);

    let grid = gtk4::Grid::builder()
        .row_spacing(4)
        .column_spacing(12)
        .hexpand(true)
        .build();
    let mut row_idx = 0;
    for b in bars {
        let name = gtk4::Label::new(Some(&b.label));
        name.set_xalign(0.0);
        name.set_wrap(true);
        name.set_wrap_mode(pango::WrapMode::WordChar);
        name.set_max_width_chars(28);
        name.set_width_chars(14);
        grid.attach(&name, 0, row_idx, 1, 1);

        let area = gtk4::DrawingArea::builder()
            .content_height(16)
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();
        redraw_on_theme_change(&area);
        let frac = (b.value / scale_max).clamp(0.0, 1.0);
        let (color, state) = (b.color.clone(), b.state);
        area.set_draw_func(move |_, cr, w, h| {
            let dark = adw::StyleManager::default().is_dark();
            let (w, h) = (w as f64, h as f64);
            if dark {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
            } else {
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.07);
            }
            rounded_rect(cr, 0.0, 0.0, w, h, h * 0.5);
            let _ = cr.fill();
            if frac > 0.0 {
                let (r, g, bl) = series_rgb(color.as_deref(), state, dark);
                cr.set_source_rgb(r, g, bl);
                rounded_rect(cr, 0.0, 0.0, (w * frac).max(h), h, h * 0.5);
                let _ = cr.fill();
            }
        });
        if !b.detail.trim().is_empty() {
            area.set_tooltip_text(Some(&b.detail));
            name.set_tooltip_text(Some(&b.detail));
        }
        grid.attach(&area, 1, row_idx, 1, 1);

        let value_txt = b.value_label.clone().unwrap_or_else(|| match unit {
            Some(u) if !u.is_empty() => format!("{} {u}", format_number(b.value)),
            _ => format_number(b.value),
        });
        let val = gtk4::Label::new(Some(&value_txt));
        val.add_css_class("caption-heading");
        val.add_css_class("numeric");
        val.set_xalign(1.0);
        grid.attach(&val, 2, row_idx, 1, 1);
        row_idx += 1;

        if !b.detail.trim().is_empty() {
            let det = gtk4::Label::new(Some(&b.detail));
            det.add_css_class("caption");
            det.add_css_class("dim-label");
            det.set_xalign(0.0);
            det.set_wrap(true);
            det.set_wrap_mode(pango::WrapMode::WordChar);
            det.set_margin_bottom(6);
            grid.attach(&det, 0, row_idx, 3, 1);
            row_idx += 1;
        }
    }
    card.append(&grid);
    card.upcast()
}

/// Círculo numerado de uma etapa com meias-linhas de conexão às etapas vizinhas.
pub fn build_step_marker(
    number: usize,
    state: CellState,
    has_prev: bool,
    has_next: bool,
    prev_done: bool,
) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::builder()
        .content_height(40)
        .content_width(40)
        .hexpand(true)
        .build();
    redraw_on_theme_change(&area);
    area.set_draw_func(move |_, cr, w, h| {
        let dark = adw::StyleManager::default().is_dark();
        let (w, h) = (w as f64, h as f64);
        let (cx, cy, r) = (w * 0.5, h * 0.5, 15.0);
        let muted = if dark { (1.0, 1.0, 1.0, 0.16) } else { (0.0, 0.0, 0.0, 0.14) };
        let done_rgb = CellState::Pass.rgb(dark);
        cr.set_line_width(3.0);
        if has_prev {
            if prev_done {
                cr.set_source_rgb(done_rgb.0, done_rgb.1, done_rgb.2);
            } else {
                cr.set_source_rgba(muted.0, muted.1, muted.2, muted.3);
            }
            cr.move_to(0.0, cy);
            cr.line_to(cx - r, cy);
            let _ = cr.stroke();
        }
        if has_next {
            if state == CellState::Pass {
                cr.set_source_rgb(done_rgb.0, done_rgb.1, done_rgb.2);
            } else {
                cr.set_source_rgba(muted.0, muted.1, muted.2, muted.3);
            }
            cr.move_to(cx + r, cy);
            cr.line_to(w, cy);
            let _ = cr.stroke();
        }
        let (sr, sg, sb) = state.rgb(dark);
        let filled = matches!(state, CellState::Pass | CellState::Running | CellState::Fail | CellState::Blocked);
        cr.arc(cx, cy, r, 0.0, TAU);
        if filled {
            cr.set_source_rgb(sr, sg, sb);
            let _ = cr.fill();
        } else {
            cr.set_source_rgba(sr, sg, sb, 0.15);
            let _ = cr.fill_preserve();
            cr.set_source_rgb(sr, sg, sb);
            cr.set_line_width(2.0);
            let _ = cr.stroke();
        }
        if state == CellState::Running {
            cr.set_source_rgba(sr, sg, sb, 0.35);
            cr.set_line_width(3.0);
            cr.arc(cx, cy, r + 4.0, 0.0, TAU);
            let _ = cr.stroke();
        }
        let txt = if state == CellState::Pass { "✓".to_string() } else { number.to_string() };
        cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        cr.set_font_size(14.0);
        if filled {
            cr.set_source_rgb(1.0, 1.0, 1.0);
        } else {
            cr.set_source_rgb(sr, sg, sb);
        }
        if let Ok(ext) = cr.text_extents(&txt) {
            cr.move_to(cx - ext.width() * 0.5 - ext.x_bearing(), cy - ext.height() * 0.5 - ext.y_bearing());
            let _ = cr.show_text(&txt);
        }
    });
    area
}
