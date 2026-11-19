//! Words and pen marks printed into pictures, for the things Iris sets a degree or two off
//! straight (the tape, "looks good", the red pen's note). GPUI can't turn text or images (only
//! monochrome SVGs take a transform), so those are drawn here from the glyph outlines, turned,
//! and shown as one picture.

use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider, instance::{LocationRef, Size}};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::riso::{Plate, Rgb8};

struct Outline {
    b: PathBuilder,
    m: [f32; 6],
}
impl Outline {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        let m = self.m;
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }
}
impl OutlinePen for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        self.b.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.p(x, y);
        self.b.line_to(x, y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (cx, cy) = self.p(cx, cy);
        let (x, y) = self.p(x, y);
        self.b.quad_to(cx, cy, x, y);
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        let (a, b) = self.p(a, b);
        let (c, d) = self.p(c, d);
        let (x, y) = self.p(x, y);
        self.b.cubic_to(a, b, c, d, x, y);
    }
    fn close(&mut self) {
        self.b.close();
    }
}

/// The glyphs and advances (points) of a line, shaped as Core Text shapes it: kerning and
/// contextual alternates on (Caveat swaps letters by their neighbours, which changes widths).
pub fn shape(font: &[u8], s: &str, size: f32) -> Vec<(skrifa::GlyphId, f32)> {
    let Some(face) = rustybuzz::Face::from_slice(font, 0) else { return vec![] };
    let k = size / face.units_per_em() as f32;
    let mut b = rustybuzz::UnicodeBuffer::new();
    b.push_str(s);
    let g = rustybuzz::shape(&face, &[], b);
    g.glyph_infos().iter().zip(g.glyph_positions()).map(|(i, p)| (skrifa::GlyphId::new(i.glyph_id), p.x_advance as f32 * k)).collect()
}

/// A line of text as one path in points, its baseline at (`x`, `base`).
pub fn text_path(font: &[u8], size: f32, s: &str, x: f32, base: f32, tracking: f32) -> Option<tiny_skia::Path> {
    let f = FontRef::new(font).ok()?;
    let glyphs = f.outline_glyphs();
    let mut pen = Outline { b: PathBuilder::new(), m: [1.0, 0.0, 0.0, -1.0, x, base] };
    for (gid, adv) in shape(font, s, size) {
        if let Some(g) = glyphs.get(gid) {
            let _ = g.draw(DrawSettings::unhinted(Size::new(size), LocationRef::default()), &mut pen);
        }
        pen.m[4] += adv + tracking;
    }
    pen.b.finish()
}

/// A picture built up from layers, each a filled path in its own colour, the whole turned
/// `degrees` about (`cx`, `cy`). Straight-alpha RGBA at the plate's scale.
pub struct Sheet {
    pub plate: Plate,
    pub pm: Pixmap,
    pub turn: Transform,
}

impl Sheet {
    pub fn new(plate: Plate, degrees: f32, cx: f32, cy: f32) -> Sheet {
        let (w, h) = plate.px();
        Sheet { pm: Pixmap::new(w.max(1), h.max(1)).unwrap(), turn: Transform::from_rotate_at(degrees, cx, cy), plate }
    }

    fn t(&self, dx: f32, dy: f32) -> Transform {
        self.plate.transform().pre_concat(self.turn).pre_translate(dx, dy)
    }

    pub fn fill(&mut self, path: &tiny_skia::Path, c: Rgb8, alpha: f32) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(c.0, c.1, c.2, (alpha * 255.0).round() as u8);
        paint.anti_alias = true;
        let t = self.t(0.0, 0.0);
        self.pm.fill_path(path, &paint, FillRule::Winding, t, None);
    }

    pub fn stroke(&mut self, path: &tiny_skia::Path, width: f32, c: Rgb8) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(c.0, c.1, c.2, 255);
        paint.anti_alias = true;
        let st = tiny_skia::Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        let t = self.t(0.0, 0.0);
        self.pm.stroke_path(path, &paint, &st, t, None);
    }

    /// A printed layer (straight RGBA at this plate's size) laid on, turned with the rest.
    pub fn picture(&mut self, rgba: &[u8], w: f32, h: f32, x: f32, y: f32) {
        let (pw, ph) = (Plate { w, h, scale: self.plate.scale }).px();
        let mut src = Pixmap::new(pw, ph).unwrap();
        for (o, i) in src.pixels_mut().iter_mut().zip(rgba.chunks_exact(4)) {
            *o = tiny_skia::ColorU8::from_rgba(i[0], i[1], i[2], i[3]).premultiply();
        }
        let s = self.plate.scale;
        let t = self.t(x, y).pre_scale(1.0 / s, 1.0 / s);
        let paint = tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
        self.pm.draw_pixmap(0, 0, src.as_ref(), &paint, t, None);
    }

    /// A picture `pw`×`ph` pixels (straight RGBA) shown `w`×`h` points, placed by `local` (from
    /// the picture's points to this sheet's), at `opacity`, smoothed.
    #[allow(clippy::too_many_arguments)]
    pub fn picture_at(&mut self, rgba: &[u8], pw: u32, ph: u32, w: f32, h: f32, local: Transform, opacity: f32) {
        self.picture_with(rgba, pw, ph, w, h, local, opacity, tiny_skia::FilterQuality::Bilinear);
    }

    /// The same, a whole source pixel at a time (SwiftUI's `.interpolation(.none)`): the garden's
    /// sprites (sun, cloud, butterflies), printed small and shown crisp, as Swift shows them.
    #[allow(clippy::too_many_arguments)]
    pub fn sprite_at(&mut self, rgba: &[u8], pw: u32, ph: u32, w: f32, h: f32, local: Transform, opacity: f32) {
        self.picture_with(rgba, pw, ph, w, h, local, opacity, tiny_skia::FilterQuality::Nearest);
    }

    #[allow(clippy::too_many_arguments)]
    fn picture_with(&mut self, rgba: &[u8], pw: u32, ph: u32, w: f32, h: f32, local: Transform, opacity: f32, quality: tiny_skia::FilterQuality) {
        if opacity <= 0.0 || pw == 0 || ph == 0 {
            return;
        }
        let mut src = Pixmap::new(pw, ph).unwrap();
        for (o, i) in src.pixels_mut().iter_mut().zip(rgba.chunks_exact(4)) {
            *o = tiny_skia::ColorU8::from_rgba(i[0], i[1], i[2], i[3]).premultiply();
        }
        let t = self.t(0.0, 0.0).pre_concat(local).pre_scale(w / pw as f32, h / ph as f32);
        let paint = tiny_skia::PixmapPaint { quality, opacity: opacity.min(1.0), ..Default::default() };
        self.pm.draw_pixmap(0, 0, src.as_ref(), &paint, t, None);
    }

    /// A path filled through `local`.
    pub fn fill_at(&mut self, path: &tiny_skia::Path, local: Transform, c: Rgb8, alpha: f32) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(c.0, c.1, c.2, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
        paint.anti_alias = true;
        let t = self.t(0.0, 0.0).pre_concat(local);
        self.pm.fill_path(path, &paint, FillRule::Winding, t, None);
    }

    /// A path stroked through `local`, round ends.
    pub fn stroke_at(&mut self, path: &tiny_skia::Path, local: Transform, width: f32, c: Rgb8) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(c.0, c.1, c.2, 255);
        paint.anti_alias = true;
        let st = tiny_skia::Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        let t = self.t(0.0, 0.0).pre_concat(local);
        self.pm.stroke_path(path, &paint, &st, t, None);
    }

    /// The picture as straight-alpha RGBA.
    pub fn rgba(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pm.data().len());
        for p in self.pm.pixels() {
            let c = p.demultiply();
            out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
        }
        out
    }
}
