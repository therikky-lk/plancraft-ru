//! CPU rendering of egui's tessellated meshes, including its font atlas.
//! Rasterization is independent of the native window/GPU and uses the same glyphs as egui.
use egui::epaint::{ClippedPrimitive, Primitive, Vertex};
use egui::{Color32, ColorImage, TextureId};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Textures(HashMap<TextureId, ColorImage>);
impl Textures {
    pub fn update(&mut self, delta: &egui::TexturesDelta) -> anyhow::Result<()> {
        for (id, changes) in &delta.set {
            for change in changes {
                let egui::ImageData::Color(image) = &change.image;
                if let Some([x, y]) = change.pos {
                    let target = self.0.get_mut(id).ok_or_else(|| anyhow::anyhow!("Missing print texture"))?;
                    let [w, h] = image.size;
                    let [tw, th] = target.size;
                    anyhow::ensure!(x.saturating_add(w) <= tw && y.saturating_add(h) <= th, "Invalid print texture update");
                    for row in 0..h {
                        let src = image.pixels.get(row * w..(row + 1) * w).ok_or_else(|| anyhow::anyhow!("Invalid font texture"))?;
                        let dst = target
                            .pixels
                            .get_mut((y + row) * tw + x..(y + row) * tw + x + w)
                            .ok_or_else(|| anyhow::anyhow!("Invalid font texture"))?;
                        dst.copy_from_slice(src);
                    }
                } else {
                    self.0.insert(*id, (**image).clone());
                }
            }
        }
        for id in &delta.free {
            self.0.remove(id);
        }
        Ok(())
    }
}

fn cross(a: egui::Pos2, b: egui::Pos2, p: egui::Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}
fn sample(t: &ColorImage, u: f32, v: f32) -> [f32; 4] {
    let [w, h] = t.size;
    let x = (u * w as f32 - 0.5).clamp(0.0, w.saturating_sub(1) as f32);
    let y = (v * h as f32 - 0.5).clamp(0.0, h.saturating_sub(1) as f32);
    let (ix, iy) = (x.floor() as usize, y.floor() as usize);
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let at = |x: usize, y: usize| t.pixels.get(y.saturating_mul(w).saturating_add(x)).copied().unwrap_or(Color32::TRANSPARENT).to_array();
    let a = at(ix, iy);
    let b = at((ix + 1).min(w.saturating_sub(1)), iy);
    let c = at(ix, (iy + 1).min(h.saturating_sub(1)));
    let d = at((ix + 1).min(w.saturating_sub(1)), (iy + 1).min(h.saturating_sub(1)));
    std::array::from_fn(|i| {
        let av = a.get(i).copied().unwrap_or(0) as f32;
        let bv = b.get(i).copied().unwrap_or(0) as f32;
        let cv = c.get(i).copied().unwrap_or(0) as f32;
        let dv = d.get(i).copied().unwrap_or(0) as f32;
        ((av * (1.0 - fx) + bv * fx) * (1.0 - fy) + (cv * (1.0 - fx) + dv * fx) * fy) / 255.0
    })
}

pub(super) fn render(primitives: &[ClippedPrimitive], textures: &Textures, size: egui::Vec2, scale: f32) -> anyhow::Result<ColorImage> {
    let width = (size.x * scale).ceil() as usize;
    let height = (size.y * scale).ceil() as usize;
    anyhow::ensure!(width > 0 && height > 0 && width.saturating_mul(height) <= 24_000_000, "Print page exceeds the image size limit");
    let mut img = ColorImage::filled([width, height], Color32::WHITE);
    for item in primitives {
        let Primitive::Mesh(mesh) = &item.primitive else { anyhow::bail!("Unsupported print drawing callback") };
        let tex = textures.0.get(&mesh.texture_id).ok_or_else(|| anyhow::anyhow!("Print texture not found"))?;
        for tri in mesh.indices.as_chunks::<3>().0 {
            let mut vertices = tri.iter().filter_map(|i| mesh.vertices.get(*i as usize));
            let (Some(a), Some(b), Some(c)) = (vertices.next(), vertices.next(), vertices.next()) else {
                anyhow::bail!("Invalid print mesh")
            };
            let convert = |v: &Vertex| Vertex { pos: egui::pos2(v.pos.x * scale, v.pos.y * scale), ..*v };
            let (a, b, c) = (convert(a), convert(b), convert(c));
            let area = cross(a.pos, b.pos, c.pos);
            if area.abs() < 1e-8 {
                continue;
            }
            let minx = a.pos.x.min(b.pos.x).min(c.pos.x).max(item.clip_rect.left() * scale).floor().clamp(0.0, width as f32) as usize;
            let maxx = a.pos.x.max(b.pos.x).max(c.pos.x).min(item.clip_rect.right() * scale).ceil().clamp(0.0, width as f32) as usize;
            let miny = a.pos.y.min(b.pos.y).min(c.pos.y).max(item.clip_rect.top() * scale).floor().clamp(0.0, height as f32) as usize;
            let maxy = a.pos.y.max(b.pos.y).max(c.pos.y).min(item.clip_rect.bottom() * scale).ceil().clamp(0.0, height as f32) as usize;
            let ac = a.color.to_array();
            let bc = b.color.to_array();
            let cc = c.color.to_array();
            for y in miny..maxy {
                for x in minx..maxx {
                    let point = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let wa = cross(b.pos, c.pos, point) / area;
                    let wb = cross(c.pos, a.pos, point) / area;
                    let wc = 1.0 - wa - wb;
                    // Half-open edge rule avoids double blending the shared diagonal of glyph quads.
                    let inside = |w: f32, p: egui::Pos2, q: egui::Pos2| {
                        let (p, q) = if area > 0.0 { (p, q) } else { (q, p) };
                        w > 0.0 || (w == 0.0 && (p.y < q.y || (p.y == q.y && p.x > q.x)))
                    };
                    if !inside(wa, b.pos, c.pos) || !inside(wb, c.pos, a.pos) || !inside(wc, a.pos, b.pos) {
                        continue;
                    }
                    let uv = a.uv.to_vec2() * wa + b.uv.to_vec2() * wb + c.uv.to_vec2() * wc;
                    let tx = sample(tex, uv.x, uv.y);
                    let color: [f32; 4] = std::array::from_fn(|i| {
                        let v = ac.get(i).copied().unwrap_or(0) as f32 * wa
                            + bc.get(i).copied().unwrap_or(0) as f32 * wb
                            + cc.get(i).copied().unwrap_or(0) as f32 * wc;
                        v / 255.0 * tx.get(i).copied().unwrap_or(0.0)
                    });
                    let alpha = color.get(3).copied().unwrap_or(0.0);
                    if alpha <= 0.0 {
                        continue;
                    }
                    if let Some(dst) = img.pixels.get_mut(y * width + x) {
                        let old = dst.to_array();
                        let channel = |i: usize| {
                            (color.get(i).copied().unwrap_or(0.0) * 255.0 + old.get(i).copied().unwrap_or(255) as f32 * (1.0 - alpha))
                                .round()
                                .clamp(0.0, 255.0) as u8
                        };
                        *dst = Color32::from_rgb(channel(0), channel(1), channel(2));
                    }
                }
            }
        }
    }
    Ok(img)
}
