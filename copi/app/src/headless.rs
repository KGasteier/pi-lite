//! Fensterloser Screenshot: egui-Frames ohne GPU tessellieren und per CPU rastern.
use std::collections::HashMap;

use egui::epaint::{ClippedPrimitive, ImageData, Primitive, TextureId};
use egui::{Color32, Pos2, RawInput, Rect, Vec2, ViewportId, ViewportInfo};

use crate::app::App;

struct Tex {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

fn apply(textures: &mut HashMap<TextureId, Tex>, delta: &mut egui::TexturesDelta) {
    for (id, list) in std::mem::take(&mut delta.set) {
        for d in list {
            #[allow(unreachable_patterns)]
            let img = match &d.image {
                ImageData::Color(c) => c.clone(),
                _ => continue,
            };
            let [w, h] = img.size;
            match d.pos {
                None => {
                    textures.insert(id, Tex { w, h, px: img.pixels.clone() });
                }
                Some([x0, y0]) => {
                    if let Some(t) = textures.get_mut(&id) {
                        for y in 0..h {
                            for x in 0..w {
                                if x0 + x < t.w && y0 + y < t.h {
                                    t.px[(y0 + y) * t.w + x0 + x] = img.pixels[y * w + x];
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for id in std::mem::take(&mut delta.free) {
        textures.remove(&id);
    }
}

fn sample(t: &Tex, u: f32, v: f32) -> [f32; 4] {
    let x = (u * t.w as f32 - 0.5).clamp(0.0, (t.w - 1) as f32);
    let y = (v * t.h as f32 - 0.5).clamp(0.0, (t.h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let g = |xx: usize, yy: usize| t.px[yy * t.w + xx].to_array();
    let (a, b, c, d) = (g(x0, y0), g(x1, y0), g(x0, y1), g(x1, y1));
    let mut o = [0.0; 4];
    for i in 0..4 {
        let top = a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx;
        let bot = c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx;
        o[i] = (top * (1.0 - fy) + bot * fy) / 255.0;
    }
    o
}

fn raster(prims: &[ClippedPrimitive], textures: &HashMap<TextureId, Tex>, ppp: f32, w: usize, h: usize, bg: Color32) -> Vec<[f32; 3]> {
    let bgc = bg.to_array();
    let mut buf = vec![[bgc[0] as f32 / 255.0, bgc[1] as f32 / 255.0, bgc[2] as f32 / 255.0]; w * h];
    for cp in prims {
        let Primitive::Mesh(mesh) = &cp.primitive else { continue };
        let Some(tex) = textures.get(&mesh.texture_id) else { continue };
        let clip = Rect::from_min_max(Pos2::new(cp.clip_rect.min.x * ppp, cp.clip_rect.min.y * ppp), Pos2::new(cp.clip_rect.max.x * ppp, cp.clip_rect.max.y * ppp));
        let cx0 = clip.min.x.max(0.0).floor() as i64;
        let cy0 = clip.min.y.max(0.0).floor() as i64;
        let cx1 = clip.max.x.min(w as f32).ceil() as i64;
        let cy1 = clip.max.y.min(h as f32).ceil() as i64;
        for tri in mesh.indices.chunks_exact(3) {
            let v = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
            let p: Vec<Vec2> = v.iter().map(|v| v.pos.to_vec2() * ppp).collect();
            let area = (p[1] - p[0]).x * (p[2] - p[0]).y - (p[1] - p[0]).y * (p[2] - p[0]).x;
            if area.abs() < 1e-6 {
                continue;
            }
            let minx = (p.iter().map(|q| q.x).fold(f32::MAX, f32::min).floor() as i64).max(cx0);
            let maxx = (p.iter().map(|q| q.x).fold(f32::MIN, f32::max).ceil() as i64).min(cx1);
            let miny = (p.iter().map(|q| q.y).fold(f32::MAX, f32::min).floor() as i64).max(cy0);
            let maxy = (p.iter().map(|q| q.y).fold(f32::MIN, f32::max).ceil() as i64).min(cy1);
            let cols: Vec<[f32; 4]> = v.iter().map(|v| v.color.to_array().map(|c| c as f32 / 255.0)).collect();
            for y in miny..maxy {
                for x in minx..maxx {
                    let s = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let e = |a: Vec2, b: Vec2| (b - a).x * (s - a).y - (b - a).y * (s - a).x;
                    let w0 = e(p[1], p[2]) / area;
                    let w1 = e(p[2], p[0]) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                    let vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                    let t = sample(tex, u, vv);
                    let mut c = [0.0f32; 4];
                    for i in 0..4 {
                        c[i] = (w0 * cols[0][i] + w1 * cols[1][i] + w2 * cols[2][i]) * t[i];
                    }
                    let d = &mut buf[y as usize * w + x as usize];
                    for i in 0..3 {
                        d[i] = c[i] + d[i] * (1.0 - c[3]);
                    }
                }
            }
        }
    }
    buf
}

/// Rendert die App fensterlos und schreibt ein PNG.
pub fn shot(path: &str, size: (u32, u32), ppp: f32, scene: Option<String>) -> Result<(), String> {
    let ctx = egui::Context::default();
    crate::setup_ctx(&ctx);
    let mut app = App::new(ctx.clone(), true);
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(size.0 as f32 / ppp, size.1 as f32 / ppp));
    let mut textures = HashMap::new();
    let mut last = None;
    for frame in 0..8 {
        if frame == 1 {
            if let Some(s) = &scene {
                app.demo_scene(s);
            }
        }
        let mut viewports = egui::ViewportIdMap::default();
        viewports.insert(ViewportId::ROOT, ViewportInfo { native_pixels_per_point: Some(ppp), focused: Some(true), ..Default::default() });
        let input = RawInput {
            viewport_id: ViewportId::ROOT,
            viewports,
            screen_rect: Some(screen),
            max_texture_side: Some(8192),
            time: Some(frame as f64 * 0.1),
            predicted_dt: 0.1,
            focused: true,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| app.draw(ui));
        apply(&mut textures, &mut out.textures_delta);
        last = Some(ctx.tessellate(out.shapes, out.pixels_per_point));
    }
    let prims = last.unwrap_or_default();
    let (w, h) = (size.0 as usize, size.1 as usize);
    let buf = raster(&prims, &textures, ppp, w, h, crate::theme::BG);
    let mut data = Vec::with_capacity(w * h * 3);
    for p in buf {
        for c in p {
            data.push((c.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(&data).map_err(|e| e.to_string())?;
    Ok(())
}
