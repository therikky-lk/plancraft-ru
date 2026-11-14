//! Native Windows printing of the same pages used by Print Preview and PDF export.
mod raster;
#[cfg(windows)]
mod windows;

use egui::{ColorImage, Rect, vec2};

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
    windows::print(&document.title, setup, &|_| Ok(document.pages.len()), &|_, index| {
        let page = document.pages.get(index).ok_or_else(|| anyhow::anyhow!("Invalid print page"))?;
        render_document_page(ctx, page)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_pages_rasterize_at_300_dpi() {
        let page = plancraft_core::print::Page { width: 180.0, height: 120.0, items: Vec::new() };
        let image = render_document_page(&context(), &page).unwrap();
        assert_eq!(image.size, [750, 500]);
        assert!(image.pixels.iter().all(|c| *c == egui::Color32::WHITE));
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
}
