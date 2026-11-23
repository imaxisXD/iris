//! Hand lettering (`HandWord`) and the wordmark (`IrisLogo`): every letter its own, drawn from
//! the glyph outlines of Knewave, stood upright, each tilted and dropped off the line; hollow
//! letters with the pink plate printed off register inside, fat ones in printed ink.

use gpui::{Div, IntoElement, ParentElement, Styled, div, px};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider, instance::{LocationRef, Size}};
use tiny_skia::{FillRule, FilterQuality, Mask, Paint, Pattern, PathBuilder, Pixmap, SpreadMode, Stroke, LineJoin, Transform};

use crate::parts::at;
use crate::riso::{self, Plate, Rgb8, Rng, col, hue, ink};

pub const KNEWAVE: &[u8] = include_bytes!("../../Sources/Iris/Resources/Fonts/Knewave-Regular.ttf");
pub const RUBIK: &[u8] = include_bytes!("../../Sources/Iris/Resources/Fonts/RubikDoodleShadow-Regular.ttf");
pub const LONDRINA: &[u8] = include_bytes!("../../Sources/Iris/Resources/Fonts/LondrinaSolid-Black.ttf");
pub const MANSALVA: &[u8] = include_bytes!("../../Sources/Iris/Resources/Fonts/Mansalva-Regular.ttf");

#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    Fat,
    Sketch,
    Hollow,
    Scribble,
    Small,
}

/// After the first fat letter, the rest take turns, so no two neighbours are drawn alike.
const TURNS: [Style; 7] = [Style::Sketch, Style::Fat, Style::Hollow, Style::Scribble, Style::Fat, Style::Sketch, Style::Fat];

/// A glyph outline as a tiny-skia path, through `transform` from font units already scaled
/// to points (y up), the way `CTFontCreatePathForGlyph` hands it over.
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

struct Letter {
    path: tiny_skia::Path,
    center: (f32, f32),
    tilt: f32,
    style: Style,
}

/// `HandLayout.make`: where each letter goes and how it's drawn. With no `pattern`, the first
/// letter is fat and a size bigger, any i is small, and the rest take turns.
fn layout(word: &str, size: f32, seed: i64, pattern: &[Style]) -> (Vec<Letter>, f32, f32) {
    let fat = FontRef::new(KNEWAVE).unwrap();
    let thin = FontRef::new(MANSALVA).unwrap();
    let metrics = fat.metrics(Size::new(size), LocationRef::default());
    let mut rng = Rng::new(seed + word.chars().count() as i64);
    let (ascent, descent) = (metrics.ascent, -metrics.descent);
    let pad = size * 0.1;
    let baseline = pad + ascent * 1.08;
    let mut x = pad;
    let mut turn = (seed.rem_euclid(TURNS.len() as i64)) as usize;
    let mut out = Vec::new();
    for (i, ch) in word.chars().enumerate() {
        if ch == ' ' {
            x += size * 0.32;
            continue;
        }
        let style = if !pattern.is_empty() {
            pattern[i % pattern.len()]
        } else if i == 0 {
            Style::Fat
        } else if ch.to_ascii_lowercase() == 'i' {
            Style::Small
        } else {
            let s = TURNS[turn % TURNS.len()];
            turn += 1;
            s
        };
        // Mansalva's caps are shorter than Knewave's: scaled so the sketched letters stand as tall.
        let (font, fsize) = if style == Style::Sketch { (&thin, size * 1.22) } else { (&fat, size) };
        let shown = if style == Style::Small { 'i' } else { ch.to_ascii_uppercase() };
        let cm = font.charmap();
        let Some(gid) = cm.map(shown) else { continue };
        let Some(glyph) = font.outline_glyphs().get(gid) else { continue };
        let grow = if i == 0 && pattern.is_empty() { 1.14 } else { 1.0 };
        let drop = (rng.f() - 0.5) * size * 0.08;
        // Knewave leans right; take the lean out (x − 0.2y). Then flip onto the page at the baseline.
        let shear = if style == Style::Sketch { 0.0 } else { -0.2 };
        let mut pen = Outline { b: PathBuilder::new(), m: [grow, 0.0, shear * grow, -grow, x, baseline + drop] };
        let _ = glyph.draw(DrawSettings::unhinted(Size::new(fsize), LocationRef::default()), &mut pen);
        let Some(path) = pen.b.finish() else { continue };
        let bx = path.bounds();
        let tilt = (rng.f() - 0.5) * 8.0;
        out.push(Letter { path, center: ((bx.left() + bx.right()) / 2.0, (bx.top() + bx.bottom()) / 2.0), tilt, style });
        let adv = font.glyph_metrics(Size::new(fsize), LocationRef::default()).advance_width(gid).unwrap_or(0.0);
        x += adv * grow - if style == Style::Sketch { 0.0 } else { size * 0.03 };
    }
    (out, x + pad + size * 0.08, baseline + descent * 0.6 + pad)
}

/// A riso screen 96 points square, a dot a point, as a pattern pixmap.
fn tile(rgb: Rgb8, seed: i64, grid: f64, specks: f64) -> Pixmap {
    let dots = riso::screen(96, 96, rgb, seed, grid, specks);
    let mut pm = Pixmap::new(96, 96).unwrap();
    for (px, c) in pm.pixels_mut().iter_mut().zip(dots) {
        *px = tiny_skia::ColorU8::from_rgba(c.0, c.1, c.2, 255).premultiply();
    }
    pm
}

/// `HandWord(word:size:pattern:weight:)` drawn once: (picture, width, height) in points.
pub fn hand_word(word: &str, size: f32, seed: i64, pattern: &[Style], weight: f32, scale: f32) -> (crate::riso::Img, f32, f32) {
    let (letters, w, h) = layout(word, size, seed, pattern);
    let plate = Plate { w, h, scale };
    let px = riso::cached(format!("hand-{word}-{size}-{seed}-{weight}"), &plate, || {
        let (pw, ph) = plate.px();
        let mut pm = Pixmap::new(pw, ph).unwrap();
        let ink_tile = tile(ink::INK, seed + 3, 0.22, 0.35);
        let pink_tile = tile(ink::PINK, seed + 5, 0.55, 0.6);
        fn shader(t: &Pixmap) -> tiny_skia::Shader<'_> { Pattern::new(t.as_ref(), SpreadMode::Repeat, FilterQuality::Nearest, 1.0, Transform::identity()) }
        for l in &letters {
            let turn = Transform::from_scale(scale, scale).pre_concat(Transform::from_rotate_at(l.tilt, l.center.0, l.center.1));
            // One more pass of the pen: the letter nudged and turned about its middle.
            let pass = |n: i32, spread: f32, turn_deg: f32| {
                let d = size * spread * n as f32;
                turn.pre_translate(l.center.0, l.center.1)
                    .pre_concat(Transform::from_rotate(n as f32 * turn_deg - turn_deg))
                    .pre_scale(1.0 + n as f32 * 0.02, 1.0 - n as f32 * 0.015)
                    .pre_translate(-l.center.0 + if n == 1 { d } else { -d * 0.5 }, -l.center.1 + if n == 2 { d * 0.6 } else { -d * 0.3 })
            };
            let mut ink_pen = Paint::default();
            ink_pen.set_color_rgba8(ink::INK.0, ink::INK.1, ink::INK.2, 255);
            ink_pen.anti_alias = true;
            let line = |w: f32| Stroke { width: w, line_cap: tiny_skia::LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
            match l.style {
                Style::Sketch => {
                    for n in 0..2 {
                        pm.stroke_path(&l.path, &ink_pen, &line(size * if n == 0 { 0.026 } else { 0.02 }), pass(n, 0.03, 3.0), None);
                    }
                }
                Style::Scribble => {
                    for n in 0..3 {
                        pm.stroke_path(&l.path, &ink_pen, &line(size * [0.042, 0.03, 0.024][n as usize]), pass(n, 0.026, 2.6), None);
                    }
                }
                Style::Fat | Style::Small => {
                    let paint = Paint { shader: shader(&ink_tile), anti_alias: true, ..Default::default() };
                    pm.fill_path(&l.path, &paint, FillRule::Winding, turn, None);
                }
                Style::Hollow => {
                    let off = turn.pre_translate(size * 0.05, size * 0.04);
                    let paint = Paint { shader: shader(&pink_tile), anti_alias: true, ..Default::default() };
                    pm.fill_path(&l.path, &paint, FillRule::Winding, off, None);
                    let mut pen = Paint::default();
                    pen.set_color_rgba8(ink::INK.0, ink::INK.1, ink::INK.2, 255);
                    pen.anti_alias = true;
                    let st = Stroke { width: size * 0.05 * weight, line_join: LineJoin::Round, ..Default::default() };
                    pm.stroke_path(&l.path, &pen, &st, turn, None);
                }
            }
        }
        // Straight alpha out.
        pm.pixels().iter().flat_map(|p| { let c = p.demultiply(); [c.red(), c.green(), c.blue(), c.alpha()] }).collect()
    });
    (px, w, h)
}

fn face_metrics(bytes: &[u8], size: f32) -> (f32, f32) {
    let m = FontRef::new(bytes).unwrap().metrics(Size::new(size), LocationRef::default());
    (m.ascent, -m.descent)
}

fn advance(bytes: &[u8], s: &str, size: f32) -> f32 {
    let f = FontRef::new(bytes).unwrap();
    let gm = f.glyph_metrics(Size::new(size), LocationRef::default());
    let cm = f.charmap();
    s.chars().filter_map(|c| cm.map(c).and_then(|g| gm.advance_width(g))).sum()
}

/// `IrisLogo`: "IRiS", the doodle I, a little room, then a hollow R, a small i and a hollow S
/// in a heavy pen. Draws 30% bigger than `size`. Returns it with its frame's size.
pub fn iris_logo(size: f32, scale: f32) -> (Div, f32, f32) {
    let s = size * 1.3;
    // The doodle I as type, on a line box as tall as the face.
    let (asc, desc) = face_metrics(RUBIK, s);
    let i_box = (asc + desc).ceil();
    let i_base = asc + (i_box - asc - desc) / 2.0;
    let i_w = advance(RUBIK, "I", s);
    let (word, w, h) = hand_word("RIS", s * 0.9, 4, &[Style::Hollow, Style::Small, Style::Hollow], 1.6, scale);
    // HandWord padded in by s/10 all round, its last baseline s/5 above its padded bottom.
    let inset = s * 0.1;
    let frame_top = i_base + s * 0.2 - (h - 2.0 * inset); // the padded frame's top, in the I's box
    let top = frame_top.min(0.0);
    let bottom = (frame_top + h - 2.0 * inset).max(i_box);
    let x = i_w + s * 0.02;
    // The Mac sets the I as type. Windows' DirectWrite set it thinner, with ClearType's coloured
    // fringes (seen in a Windows 11 VM): there it's drawn from the font's outline instead.
    let i = if crate::keys::is_mac() {
        div().font_family("Rubik Doodle Shadow").text_size(px(s)).line_height(px(i_box)).whitespace_nowrap()
            .text_color(col(hue::I12)).child("I").into_any_element()
    } else {
        crate::parts::printed(i_w, i_box, &doodle_i(s, i_w, i_box, i_base, scale))
    };
    let logo = crate::parts::sized(x + w - 2.0 * inset, bottom - top)
        .child(at(i, 0.0, -top))
        .child(at(crate::parts::printed(w, h, &word), x - inset, frame_top - inset - top));
    (logo, x + w - 2.0 * inset, bottom - top)
}

// MARK: - Ghost words

/// The pale riso inks a ghost word prints in (`GhostWord.Ink`).
#[derive(Clone, Copy)]
pub enum Ghost {
    Khaki,
    Pink,
    Sky,
    Tan,
}

impl Ghost {
    fn rgb(self) -> Rgb8 {
        let c = match self {
            Ghost::Khaki => (0.89, 0.87, 0.80),
            Ghost::Pink => (0.97, 0.83, 0.90),
            Ghost::Sky => (0.82, 0.88, 0.96),
            Ghost::Tan => (0.93, 0.85, 0.77),
        };
        ((c.0 * 255.0f32).round() as u8, (c.1 * 255.0f32).round() as u8, (c.2 * 255.0f32).round() as u8)
    }
}

/// Londrina shaped (kerning included): glyphs with where each starts, and the line's width.
pub fn shape_londrina(s: &str, size: f32) -> (Vec<(u32, f32)>, f32) {
    let face = rustybuzz::Face::from_slice(LONDRINA, 0).unwrap();
    let k = size / face.units_per_em() as f32;
    let mut buf = rustybuzz::UnicodeBuffer::new();
    buf.push_str(s);
    let out = rustybuzz::shape(&face, &[], buf);
    let mut x = 0.0;
    let mut glyphs = Vec::new();
    for (info, pos) in out.glyph_infos().iter().zip(out.glyph_positions()) {
        glyphs.push((info.glyph_id, x + pos.x_offset as f32 * k));
        x += pos.x_advance as f32 * k;
    }
    (glyphs, x)
}

/// `GhostWord`: chunky caps broken over two lines (CRU / SH), tight, in a pale ink salted with
/// paper-coloured grain. (picture, width, height) in points.
pub fn ghost_word(word: &str, size: f32, ink: Ghost, scale: f32) -> (crate::riso::Img, f32, f32) {
    let caps: Vec<char> = word.to_uppercase().chars().collect();
    let cut = (caps.len() + 1) / 2;
    let lines: Vec<String> = if caps.len() < 4 { vec![caps.iter().collect()] } else { vec![caps[..cut].iter().collect(), caps[cut..].iter().collect()] };
    let font = FontRef::new(LONDRINA).unwrap();
    let m = font.metrics(Size::new(size), LocationRef::default());
    let line_h = m.ascent - m.descent;
    let shaped: Vec<_> = lines.iter().map(|l| shape_londrina(l, size)).collect();
    let w = shaped.iter().map(|s| s.1).fold(0.0, f32::max);
    let h = line_h * lines.len() as f32 - size * 0.3 * (lines.len() as f32 - 1.0);
    let plate = Plate { w, h, scale };
    let px = riso::cached(format!("ghost-{word}-{size}"), &plate, || {
        let mut mask = plate.mask();
        let outlines = font.outline_glyphs();
        for (n, (glyphs, _)) in shaped.iter().enumerate() {
            let baseline = n as f32 * (line_h - size * 0.3) + m.ascent;
            let mut b = PathBuilder::new();
            for (gid, x) in glyphs {
                if let Some(o) = outlines.get(skrifa::GlyphId::new(*gid)) {
                    let mut pen = Outline { b, m: [1.0, 0.0, 0.0, -1.0, *x, baseline] };
                    let _ = o.draw(DrawSettings::unhinted(Size::new(size), LocationRef::default()), &mut pen);
                    b = pen.b;
                }
            }
            if let Some(p) = b.finish() {
                mask.fill_path(&p, FillRule::Winding, true, plate.transform());
            }
        }
        // The salt: a 128-pixel tile of paper specks shown at 64 points, at 90%.
        let mut rng = Rng::new(17);
        let salt: Vec<f32> = (0..128 * 128).map(|_| ((rng.next() - 0.62) * 2.2).clamp(0.0, 1.0) as f32).collect();
        let (pw, ph) = plate.px();
        let c = ink.rgb();
        let paper = (252.0, 249.0, 246.0);
        let mut out = vec![0u8; (pw * ph * 4) as usize];
        for y in 0..ph {
            for x in 0..pw {
                let i = (y * pw + x) as usize;
                let a = mask.data()[i];
                if a == 0 { continue }
                let s = salt[(((y as f32 * 2.0 / scale) as usize) % 128) * 128 + ((x as f32 * 2.0 / scale) as usize) % 128] * 0.9;
                let mix = |v: u8, p: f32| (v as f32 * (1.0 - s) + p * s).round() as u8;
                out[i * 4..i * 4 + 4].copy_from_slice(&[mix(c.0, paper.0), mix(c.1, paper.1), mix(c.2, paper.2), a]);
            }
        }
        out
    });
    (px, w, h)
}

/// A Martian Mono glyph at weight `wght` and width `wdth`, its baseline at (`x`, `base`).
pub fn martian_glyph_into(mask: &mut Mask, plate: &Plate, c: char, size: f32, wght: f32, wdth: f32, x: f32, base: f32) {
    let font = FontRef::new(crate::parts::MARTIAN).unwrap();
    let loc = font.axes().location([("wght", wght), ("wdth", wdth)]);
    let Some(gid) = font.charmap().map(c) else { return };
    let Some(g) = font.outline_glyphs().get(gid) else { return };
    let mut pen = Outline { b: PathBuilder::new(), m: [1.0, 0.0, 0.0, -1.0, x, base] };
    let _ = g.draw(DrawSettings::unhinted(Size::new(size), LocationRef::from(&loc)), &mut pen);
    if let Some(p) = pen.b.finish() {
        mask.fill_path(&p, FillRule::Winding, true, plate.transform());
    }
}

/// Martian Mono's advance at a weight and width.
pub fn martian_advance(size: f32, wght: f32, wdth: f32) -> f32 {
    let font = FontRef::new(crate::parts::MARTIAN).unwrap();
    let loc = font.axes().location([("wght", wght), ("wdth", wdth)]);
    let gid = font.charmap().map('M').unwrap();
    font.glyph_metrics(Size::new(size), LocationRef::from(&loc)).advance_width(gid).unwrap_or(size * 0.65)
}

/// "✻" as Core Text falls back for it (Zapf Dingbats; Martian has none): six teardrop spokes,
/// thin at the hub, round where they end, reaching `r` from the centre. Setup's headlines, the
/// words up Settings' rule and the Model page's faint lettering all set this one.
pub fn star_into(mask: &mut Mask, plate: &Plate, cx: f32, cy: f32, r: f32) {
    // Measured from the glyph at 38 points, where it reaches 12 (`k` scales from there).
    let k = r / 12.0;
    let (reach, bulb) = (9.2 * k, 2.8 * k);
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::PI / 3.0 - std::f32::consts::FRAC_PI_2;
        let (dx, dy) = (a.cos(), a.sin());
        let (nx, ny) = (-dy, dx);
        let at = |d: f32, n: f32| (cx + dx * d + nx * n, cy + dy * d + ny * n);
        // The neck, from the hub out to the bulb's sides, and the bulb: each filled on its own,
        // as they wind opposite ways (as one path the bulbs would cut holes in the necks).
        let mut b = PathBuilder::new();
        let (a0, a1) = (at(0.3 * k, 0.6 * k), at(0.3 * k, -0.6 * k));
        let (b0, b1) = (at(reach - 0.6 * k, bulb * 0.98), at(reach - 0.6 * k, -bulb * 0.98));
        let (c0, c1) = (at(reach * 0.6, 1.5 * k), at(reach * 0.6, -1.5 * k));
        b.move_to(a0.0, a0.1);
        b.quad_to(c0.0, c0.1, b0.0, b0.1);
        b.line_to(b1.0, b1.1);
        b.quad_to(c1.0, c1.1, a1.0, a1.1);
        b.close();
        let c = at(reach, 0.0);
        let parts = [b.finish(), PathBuilder::from_circle(c.0, c.1, bulb)];
        for p in parts.into_iter().flatten() {
            mask.fill_path(&p, FillRule::Winding, true, plate.transform());
        }
    }
}

/// The doodle I from Rubik Doodle Shadow's outline, its baseline at `base` in a `w`×`h` box.
fn doodle_i(s: f32, w: f32, h: f32, base: f32, scale: f32) -> riso::Img {
    let plate = Plate { w, h, scale };
    riso::cached(format!("logo-doodle-i-{s}"), &plate, || {
        let (pw, ph) = plate.px();
        let mut pm = Pixmap::new(pw, ph).unwrap();
        let f = FontRef::new(RUBIK).unwrap();
        if let Some(g) = f.charmap().map('I').and_then(|gid| f.outline_glyphs().get(gid)) {
            let mut pen = Outline { b: PathBuilder::new(), m: [1.0, 0.0, 0.0, -1.0, 0.0, base] };
            let _ = g.draw(DrawSettings::unhinted(Size::new(s), LocationRef::default()), &mut pen);
            if let Some(path) = pen.b.finish() {
                let mut paint = Paint::default();
                let (r, gr, b) = hue::I12;
                paint.set_color_rgba8(r, gr, b, 255);
                paint.anti_alias = true;
                let t = Transform::from_scale(scale, scale);
                pm.fill_path(&path, &paint, FillRule::Winding, t, None);
                // Core Text sets this face a touch heavier than its bare outline: a hairline round it.
                let pen = Stroke { width: s * 0.012, line_join: LineJoin::Round, ..Default::default() };
                pm.stroke_path(&path, &paint, &pen, t, None);
            }
        }
        pm.pixels().iter().flat_map(|p| { let c = p.demultiply(); [c.red(), c.green(), c.blue(), c.alpha()] }).collect()
    })
}
