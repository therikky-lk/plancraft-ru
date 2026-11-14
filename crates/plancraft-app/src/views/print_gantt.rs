//! Paper layout uses the existing task grid and Gantt renderer. Windows owns printing/PDF.
mod raster;
#[cfg(windows)]
mod windows;

use crate::columns::{self, CellCtx, TaskCol};
use crate::state::{GanttState, UiState, ViewKind};
use crate::theme;
use crate::widgets::grid::{self, GridConfig, GridState};
use chrono::{Duration, NaiveDateTime, NaiveTime};
use egui::{Align2, ColorImage, FontId, Rect, TextureHandle, pos2, vec2};
use plancraft_core::prelude::*;

const PAGE_WIDTH: f32 = 1120.0;
const MARGIN: f32 = 32.0;
const ROW: f32 = 26.0;
const HEADER: f32 = 42.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    A4,
    A3,
}

pub struct PrintSettings {
    pub paper: Paper,
    pub landscape: bool,
    pub days: u16,
    pub current_scale: bool,
    pub resources: bool,
    pub page: usize,
    pub zoom: f32,
    native_paper: Option<plancraft_core::print::Paper>,
    cache: Option<(String, TextureHandle)>,
}
impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            paper: Paper::A4,
            landscape: true,
            days: 28,
            current_scale: false,
            resources: false,
            page: 0,
            zoom: 0.9,
            native_paper: None,
            cache: None,
        }
    }
}
impl PrintSettings {
    fn mm(&self) -> (f32, f32) {
        if let Some(paper) = self.native_paper {
            let (w, h) = paper.size();
            let (w, h) = (w * 25.4 / 72.0, h * 25.4 / 72.0);
            return if self.landscape { (h, w) } else { (w, h) };
        }
        let (w, h) = match self.paper {
            Paper::A4 => (210.0, 297.0),
            Paper::A3 => (297.0, 420.0),
        };
        if self.landscape { (h, w) } else { (w, h) }
    }
}

/// Rasterizes the shared PDF/preview page for the existing Windows printer boundary.
#[cfg(any(windows, test))]
fn render_document_page(parent: &egui::Context, page: &plancraft_core::print::Page) -> anyhow::Result<ColorImage> {
    let size = vec2(page.width, page.height);
    let ctx = egui::Context::default();
    ctx.set_fonts(parent.fonts(|f| f.definitions().clone()));
    ctx.set_pixels_per_point(300.0 / 72.0);
    let mut textures = raster::Textures::default();
    let mut image = None;
    for pass in 0..2 {
        let mut output =
            ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)), ..Default::default() }, |ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
                    crate::dialogs::print::paint_page(ui.painter(), Rect::from_min_size(egui::Pos2::ZERO, size), page, 1.0);
                });
            });
        let applied = textures.update(&output.textures_delta);
        output.textures_delta.clear();
        applied?;
        if pass == 1 {
            let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
            image = Some(raster::render(&primitives, &textures, size, output.pixels_per_point)?);
        }
    }
    image.ok_or_else(|| anyhow::anyhow!("No print image"))
}

#[cfg(windows)]
pub(crate) fn print_document(
    ctx: &egui::Context,
    document: &plancraft_core::print::Document,
    setup: &plancraft_core::print::PageSetup,
) -> anyhow::Result<bool> {
    let settings = PrintSettings {
        native_paper: Some(setup.paper),
        landscape: setup.orientation == plancraft_core::print::Orientation::Landscape,
        ..Default::default()
    };
    windows::print(&document.title, &settings, &|_| Ok(document.pages.len()), &|_, index| {
        let page = document.pages.get(index).ok_or_else(|| anyhow::anyhow!("Invalid print page"))?;
        render_document_page(ctx, page)
    })
}

fn preview_image(image: ColorImage, max_side: usize) -> ColorImage {
    let [w, h] = image.size;
    let step = w.max(h).div_ceil(max_side.max(1)).max(1);
    if step == 1 {
        return image;
    }
    let size = [w.div_ceil(step), h.div_ceil(step)];
    let pixels = (0..size[1])
        .flat_map(|y| {
            let image = &image;
            (0..size[0]).map(move |x| {
                image
                    .pixels
                    .get(y.saturating_mul(step).saturating_mul(w).saturating_add(x.saturating_mul(step)))
                    .copied()
                    .unwrap_or_default()
            })
        })
        .collect();
    ColorImage::new(size, pixels)
}

/// Immutable row/date pagination, shared by the preview and the printer.
struct Layout {
    rows: Vec<super::Row>,
    start: NaiveDateTime,
    days: usize,
    horizontal: usize,
    per_page: usize,
    pages: usize,
    size: egui::Vec2,
    paper_mm: (f32, f32),
    cols: Vec<TaskCol>,
    widths: Vec<f32>,
    table_width: f32,
}
impl Layout {
    fn new(p: &Project, st: &UiState, s: &PrintSettings, paper_mm: (f32, f32)) -> anyhow::Result<Self> {
        let rows = super::visible_rows(p, st);
        anyhow::ensure!(!rows.is_empty(), label("No tasks to print", "印刷対象のタスクがありません"));
        // A constant physical font/row size across A4/A3 and both orientations.
        let size = vec2(PAGE_WIDTH * paper_mm.0 / 297.0, PAGE_WIDTH * paper_mm.1 / 297.0);
        let per_page = ((size.y - 2.0 * MARGIN - 76.0 - HEADER - 30.0) / ROW).floor().max(1.0) as usize;
        let mut start = None;
        let mut finish = None;
        for idx in &rows {
            if let Some(t) = idx.task().and_then(|i| p.tasks.get(i)) {
                for d in
                    [t.sched.start, t.baseline.filter(|_| st.gantt.show_baseline || st.view == ViewKind::TrackingGantt).map(|b| b.start)]
                        .into_iter()
                        .flatten()
                {
                    start = Some(start.map_or(d, |old: NaiveDateTime| old.min(d)));
                }
                for d in
                    [t.sched.finish, t.baseline.filter(|_| st.gantt.show_baseline || st.view == ViewKind::TrackingGantt).map(|b| b.finish)]
                        .into_iter()
                        .flatten()
                {
                    finish = Some(finish.map_or(d, |old: NaiveDateTime| old.max(d)));
                }
            } else if let Some(group) = super::group_header(p, idx) {
                if let Some(d) = group.start {
                    start = Some(start.map_or(d, |old| old.min(d)));
                }
                if let Some(d) = group.finish {
                    finish = Some(finish.map_or(d, |old| old.max(d)));
                }
            }
        }
        let start = start
            .unwrap_or(p.start)
            .date()
            .and_time(NaiveTime::MIN)
            .checked_sub_signed(Duration::days(1))
            .ok_or_else(|| anyhow::anyhow!("Print start date is out of range"))?;
        let span = (finish.unwrap_or(p.start) - start).num_days().saturating_add(3).max(1) as usize;
        let mut cols = vec![TaskCol::Name, TaskCol::Duration, TaskCol::Start, TaskCol::Finish];
        // Printed dates use a compact complete date, without the screen's weekday suffix.
        let mut widths = vec![224.0, 66.0, 94.0, 94.0];
        if s.resources {
            cols.push(TaskCol::Resources);
            widths.push(130.0);
        }
        if size.x < 900.0 {
            widths = if s.resources { vec![126.0, 66.0, 94.0, 94.0, 90.0] } else { vec![150.0, 66.0, 94.0, 94.0] };
        }
        let table_width = widths.iter().sum::<f32>() + 34.0;
        let chart_width = size.x - MARGIN * 2.0 - table_width - 4.0;
        let days = if s.current_scale {
            (chart_width / st.gantt.px_per_day.clamp(0.5, 160.0)).floor().max(1.0) as usize
        } else {
            s.days.clamp(7, 112) as usize
        };
        let horizontal = span.div_ceil(days);
        let pages = horizontal.saturating_mul(rows.len().div_ceil(per_page));
        anyhow::ensure!(
            pages <= 1000,
            label("Too many pages; widen the date range", "ページ数が1000を超えます。1枚の日数を増やしてください")
        );
        Ok(Self { rows, start, days, horizontal, per_page, pages, size, paper_mm, cols, widths, table_width })
    }
    fn date(&self, page: usize) -> anyhow::Result<NaiveDateTime> {
        self.start
            .checked_add_signed(Duration::days(((page % self.horizontal) * self.days) as i64))
            .ok_or_else(|| anyhow::anyhow!("Print date is out of range"))
    }
    fn render(&self, parent: &egui::Context, p: &Project, st: &UiState, page: usize) -> anyhow::Result<ColorImage> {
        anyhow::ensure!(page < self.pages, "Invalid print page");
        let date = self.date(page)?;
        let row_start = (page / self.horizontal) * self.per_page;
        let rows = self.rows.get(row_start..row_start.saturating_add(self.per_page).min(self.rows.len())).unwrap_or(&[]);
        let scale = (self.paper_mm.0 / 25.4 * 300.0) / self.size.x;
        let ctx = egui::Context::default();
        ctx.set_fonts(parent.fonts(|f| f.definitions().clone()));
        ctx.set_visuals(egui::Visuals::light());
        ctx.set_pixels_per_point(scale);
        let raw = || egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.size)), ..Default::default() };
        let mut textures = raster::Textures::default();
        let mut image = None;
        // The first pass populates the font atlas; the second uses the final layout.
        for pass in 0..2 {
            let mut output = theme::on_paper(|| {
                ctx.run_ui(raw(), |ui| {
                    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
                        let painter = ui.painter().clone();
                        painter.rect_filled(ui.max_rect(), 0.0, theme::pal().BG);
                        painter.text(pos2(MARGIN, MARGIN), Align2::LEFT_TOP, &p.name, FontId::proportional(23.0), theme::pal().TEXT);
                        let end = date.checked_add_signed(Duration::days(self.days as i64)).unwrap_or(date);
                        painter.text(
                            pos2(MARGIN, MARGIN + 33.0),
                            Align2::LEFT_TOP,
                            format!("{} – {}    {} / {}", date.format("%Y/%m/%d"), end.format("%Y/%m/%d"), page + 1, self.pages),
                            FontId::proportional(12.0),
                            theme::pal().TEXT_WEAK,
                        );
                        let top = MARGIN + 70.0;
                        let height = HEADER + rows.len() as f32 * ROW;
                        let table = Rect::from_min_size(pos2(MARGIN, top), vec2(self.table_width, height));
                        let defs = self
                            .cols
                            .iter()
                            .zip(&self.widths)
                            .map(|(c, w)| {
                                let mut d = c.def(p);
                                d.width = *w;
                                d
                            })
                            .collect::<Vec<_>>();
                        let cfg = GridConfig {
                            id: egui::Id::new("print_table"),
                            columns: &defs,
                            row_count: rows.len(),
                            blank_rows: 0,
                            row_numbers: true,
                            show_vscroll: false,
                            header_height: HEADER,
                            row_height: ROW,
                            spreadsheet: false,
                            row_label: Some(&|row| {
                                rows.get(row)
                                    .and_then(|idx| idx.task().and_then(|i| p.tasks.get(i)))
                                    .and_then(|t| p.row_number(t.id))
                                    .map(|n| n.to_string())
                                    .unwrap_or_default()
                            }),
                        };
                        let blank = |_id: TaskId| false;
                        let cell_ctx = CellCtx { overallocated: &blank };
                        let mut grid_state = GridState { widths: self.widths.clone(), ..Default::default() };
                        let mut table_ui = ui.new_child(egui::UiBuilder::new().max_rect(table));
                        table_ui.set_clip_rect(table);
                        grid::show(
                            &mut table_ui,
                            Rect::from_min_max(table.min, table.max + vec2(0.0, 12.0)),
                            &mut grid_state,
                            &cfg,
                            &|_| false,
                            &mut |r, c| {
                                let (Some(idx), Some(col)) = (rows.get(r), self.cols.get(c)) else { return grid::Cell::default() };
                                if let Some(t) = idx.task().and_then(|i| p.tasks.get(i)) {
                                    let mut cell = idx.task().map(|i| columns::cell(*col, p, i, &cell_ctx)).unwrap_or_default();
                                    if *col == TaskCol::Start {
                                        cell.text = t.sched.start.map(|d| d.format("%Y/%m/%d").to_string()).unwrap_or_default();
                                    }
                                    if *col == TaskCol::Finish {
                                        cell.text = t.sched.finish.map(|d| d.format("%Y/%m/%d").to_string()).unwrap_or_default();
                                    }
                                    cell
                                } else if let Some(group) = super::group_header(p, idx) {
                                    grid::Cell {
                                        text: if *col == TaskCol::Name { group.label } else { String::new() },
                                        bold: true,
                                        bg: Some(theme::pal().HEADER_BG),
                                        ..Default::default()
                                    }
                                } else {
                                    grid::Cell::default()
                                }
                            },
                        );
                        let chart = Rect::from_min_max(pos2(table.right() + 4.0, top), pos2(self.size.x - MARGIN, top + height));
                        let mut chart_ui = ui.new_child(egui::UiBuilder::new().max_rect(chart));
                        chart_ui.set_clip_rect(chart);
                        let mut g: GanttState = st.gantt.clone();
                        g.left = date;
                        g.px_per_day = chart.width() / self.days as f32;
                        g.initialized_for = Some(p.id);
                        g.drag = None;
                        let input = super::gantt_chart::ChartInput {
                            project: p,
                            rows,
                            selected: &|_| false,
                            kind: match st.view {
                                ViewKind::TrackingGantt => super::gantt_chart::ChartKind::Tracking,
                                ViewKind::MultipleBaselines => super::gantt_chart::ChartKind::Baselines,
                                _ => super::gantt_chart::ChartKind::Gantt,
                            },
                            row_height: ROW,
                            header_height: HEADER,
                            blank_rows: 0,
                        };
                        super::gantt_chart::show(
                            &mut chart_ui,
                            Rect::from_min_max(chart.min, chart.max + vec2(12.0, 12.0)),
                            &mut g,
                            &mut 0.0,
                            &input,
                        );
                        painter.text(
                            pos2(MARGIN, self.size.y - MARGIN),
                            Align2::LEFT_BOTTOM,
                            label("PlanCraft · continued tasks/dates on subsequent sheets", "PlanCraft · 続くタスク・期間は次の用紙に表示"),
                            FontId::proportional(11.0),
                            theme::pal().TEXT_WEAK,
                        );
                        painter.text(
                            pos2(self.size.x - MARGIN, self.size.y - MARGIN),
                            Align2::RIGHT_BOTTOM,
                            format!("{} / {}", page + 1, self.pages),
                            FontId::proportional(12.0),
                            theme::pal().TEXT_WEAK,
                        );
                    });
                })
            });
            let texture_result = textures.update(&output.textures_delta);
            output.textures_delta.clear();
            texture_result?;
            if pass == 1 {
                let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
                image = Some(raster::render(&primitives, &textures, self.size, output.pixels_per_point)?);
            }
        }
        image.ok_or_else(|| anyhow::anyhow!("No print image"))
    }
}
fn label(en: &'static str, ja: &'static str) -> &'static str {
    if plancraft_core::i18n::is_ja() { ja } else { en }
}

/// Returns true when Back is clicked. Native printing handles its own cancellation.
pub fn show(ui: &mut egui::Ui, p: &Project, st: &UiState, s: &mut PrintSettings, font: &str) -> bool {
    let mut back = false;
    let mut print = false;
    ui.horizontal_wrapped(|ui| {
        back = ui.button(label("Back", "戻る")).clicked();
        ui.separator();
        ui.label(label("Paper", "用紙"));
        egui::ComboBox::from_id_salt("print_paper")
            .selected_text(match s.paper {
                Paper::A4 => "A4",
                Paper::A3 => "A3",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut s.paper, Paper::A4, "A4");
                ui.selectable_value(&mut s.paper, Paper::A3, "A3");
            });
        ui.selectable_value(&mut s.landscape, true, label("Landscape", "横向き"));
        ui.selectable_value(&mut s.landscape, false, label("Portrait", "縦向き"));
        ui.checkbox(&mut s.current_scale, label("Screen timescale", "画面の時間軸倍率"));
        ui.add_enabled(!s.current_scale, egui::DragValue::new(&mut s.days).range(7..=112).prefix(label("Days / sheet: ", "1枚の日数: ")));
        ui.checkbox(&mut s.resources, label("Resources", "リソース名"));
        print = ui.button(label("Print…", "印刷…")).clicked();
    });
    ui.label(label(
        "300 dpi · white paper · choose Microsoft Print to PDF in the printer dialog to save a PDF.",
        "300 dpi・白い用紙で印刷します。PDF保存は印刷画面で Microsoft Print to PDF を選んでください。",
    ));
    let layout = match Layout::new(p, st, s, s.mm()) {
        Ok(l) => l,
        Err(e) => {
            ui.colored_label(theme::pal().ERROR, e.to_string());
            return back;
        }
    };
    s.page = s.page.min(layout.pages.saturating_sub(1));
    ui.horizontal(|ui| {
        if ui.add_enabled(s.page > 0, egui::Button::new(label("Previous", "前のページ"))).clicked() {
            s.page = s.page.saturating_sub(1);
        }
        ui.label(format!("{} / {}", s.page + 1, layout.pages));
        if ui.add_enabled(s.page + 1 < layout.pages, egui::Button::new(label("Next", "次のページ"))).clicked() {
            s.page += 1;
        }
        ui.add(egui::Slider::new(&mut s.zoom, 0.4..=1.6).text(label("Preview zoom", "プレビュー倍率")));
    });
    // A project fingerprint invalidates preview after any data/format change.
    let key = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{:?}:{:?}:{:?}:{:?}:{}:{}",
        p.id,
        p.tasks.len(),
        layout.start,
        layout.days,
        layout.pages,
        s.page,
        s.landscape,
        s.resources,
        st.gantt.show_links,
        st.gantt.show_baseline,
        st.gantt.show_critical,
        st.gantt.show_progress,
        serde_json::to_string(p).unwrap_or_default(),
        layout.size,
        layout.rows,
        st.gantt,
        st.view,
        font,
        plancraft_core::i18n::is_ja()
    );
    if s.cache.as_ref().is_none_or(|(old, _)| old != &key) {
        match layout.render(ui.ctx(), p, st, s.page) {
            Ok(img) => {
                let max_side = ui.ctx().input(|i| i.max_texture_side);
                let img = preview_image(img, max_side);
                s.cache = Some((key, ui.ctx().load_texture("gantt_print_page", img, egui::TextureOptions::LINEAR)));
            }
            Err(e) => {
                s.cache = None;
                ui.colored_label(theme::pal().ERROR, e.to_string());
            }
        }
    }
    if let Some((_, texture)) = &s.cache {
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let size = layout.size * s.zoom;
            ui.add(egui::Image::new(texture).fit_to_exact_size(size));
        });
    }
    if print {
        #[cfg(windows)]
        let result = windows::print(&p.name, s, &|mm| Layout::new(p, st, s, mm).map(|l| l.pages), &|mm, page| {
            Layout::new(p, st, s, mm)?.render(ui.ctx(), p, st, page)
        });
        #[cfg(not(windows))]
        let result: anyhow::Result<bool> = Err(anyhow::anyhow!("Native printing is currently supported on Windows"));
        if let Err(e) = result {
            rfd::MessageDialog::new()
                .set_title(label("Printing failed", "印刷できませんでした"))
                .set_description(e.to_string())
                .set_level(rfd::MessageLevel::Error)
                .show();
        }
    }
    back
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_respects_small_texture_limits_and_shared_pages_rasterize() {
        let image = ColorImage::filled([3000, 2200], egui::Color32::WHITE);
        let preview = preview_image(image, 1024);
        assert!(preview.size.into_iter().all(|side| side <= 1024));
        let page = plancraft_core::print::Page { width: 180.0, height: 120.0, items: Vec::new() };
        let image = render_document_page(&context(), &page).unwrap();
        assert_eq!(image.size, [750, 500]);
        assert!(image.pixels.iter().all(|c| *c == egui::Color32::WHITE));
    }
    fn project() -> Project {
        plancraft_core::sample::demo_project(chrono::NaiveDate::from_ymd_opt(2026, 10, 5).unwrap())
    }
    pub(super) fn context() -> egui::Context {
        let ctx = egui::Context::default();
        let mut initial = ctx.run_ui(egui::RawInput::default(), |_| {});
        initial.textures_delta.clear();
        crate::fonts::install(&ctx, "Yu Gothic UI");
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        ctx
    }
    #[test]
    fn every_row_and_date_tile_is_paginated_without_dropping_the_last_row() {
        let mut p = project();
        for i in 0..60 {
            p.add_task(format!("Additional task {i}"), None);
        }
        let st = UiState::default();
        let s = PrintSettings::default();
        for landscape in [true, false] {
            let s = PrintSettings { landscape, ..PrintSettings::default() };
            let l = Layout::new(&p, &st, &s, s.mm()).unwrap();
            let mut seen = Vec::new();
            for page in (0..l.pages).step_by(l.horizontal) {
                let from = page / l.horizontal * l.per_page;
                seen.extend(l.rows.get(from..(from + l.per_page).min(l.rows.len())).unwrap().iter().cloned());
            }
            assert_eq!(seen, l.rows);
            assert!(l.days * l.horizontal >= (p.sched.finish.unwrap() - l.start).num_days() as usize);
            assert_eq!((l.date(1).unwrap() - l.date(0).unwrap()).num_days(), l.days as i64);
        }
        assert!(Layout::new(&p, &st, &s, s.mm()).unwrap().pages > 3);
    }
    #[test]
    fn paper_renders_egui_fonts_and_bars_at_300_dpi_without_unhandled_texture_deltas() {
        let p = project();
        let st = UiState::default();
        let s = PrintSettings::default();
        let l = Layout::new(&p, &st, &s, s.mm()).unwrap();
        let img = l.render(&context(), &p, &st, 0).unwrap();
        let [w, h] = img.size;
        assert!((w as i64 - 3508).abs() <= 2 && (h as i64 - 2480).abs() <= 2);
        assert!(img.pixels.iter().filter(|c| c.b() > c.r().saturating_add(25)).count() > 2000, "task bars are missing");
        assert!(img.pixels.iter().filter(|c| c.r() < 100 && c.g() < 100 && c.b() < 100).count() > 3000, "text is missing");
    }
}
