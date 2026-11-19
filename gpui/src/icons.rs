//! Iris's own icons (`RisoIcons.swift`) and the garden's sprites (`Sprites` in `Garden.swift`),
//! printed like the logo's flower: a crema shadow plate, a colour plate and an espresso ink
//! plate, each ordered-dithered and a pixel or two off register. Drawn on an 80×80 canvas.

use crate::riso::Img as Pixels;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, Transform};

use crate::riso::{self, Plate, Rgb8, ink, print_icon, str_seed};

/// A plate being drawn: grey on white, in the 80-point canvas (`CGContext` in Swift).
pub struct Draw<'a> {
    pm: &'a mut Pixmap,
    t: Transform,
    pen: f32,
}

impl Draw<'_> {
    pub fn fill(&mut self, p: &Path, gray: u8) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(gray, gray, gray, 255);
        paint.anti_alias = true;
        self.pm.fill_path(p, &paint, FillRule::Winding, self.t, None);
    }
    /// `ctx.pen(width)`: near-black, round ends and joins.
    pub fn pen(&mut self, w: f32) {
        self.pen = w;
    }
    pub fn stroke(&mut self, p: &Path) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(25, 25, 25, 255);
        paint.anti_alias = true;
        let st = Stroke { width: self.pen, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
        self.pm.stroke_path(p, &paint, &st, self.t, None);
    }
}

type Art = fn(&mut Draw);

/// Plates printed at `pixels`: (ink, dx, dy, drawing).
pub fn print(pixels: u32, seed: i64, plates: &[(Rgb8, i32, i32, Art)]) -> Vec<u8> {
    let draws: Vec<Box<dyn Fn(&mut Pixmap, Transform)>> = plates.iter()
        .map(|&(_, _, _, art)| Box::new(move |pm: &mut Pixmap, t: Transform| art(&mut Draw { pm, t, pen: 4.8 })) as Box<dyn Fn(&mut Pixmap, Transform)>)
        .collect();
    let list: Vec<(Rgb8, i32, i32, &dyn Fn(&mut Pixmap, Transform))> =
        plates.iter().zip(&draws).map(|(p, d)| (p.0, p.1, p.2, d.as_ref())).collect();
    print_icon(pixels, seed, &list)
}

// MARK: - Shapes

pub fn circle(x: f32, y: f32, r: f32) -> Path {
    PathBuilder::from_circle(x, y, r).unwrap()
}
pub fn ellipse(cx: f32, cy: f32, w: f32, h: f32) -> Path {
    PathBuilder::from_oval(tiny_skia::Rect::from_xywh(cx - w / 2.0, cy - h / 2.0, w, h).unwrap()).unwrap()
}
pub fn line(x1: f32, y1: f32, x2: f32, y2: f32) -> Path {
    let mut b = PathBuilder::new();
    b.move_to(x1, y1);
    b.line_to(x2, y2);
    b.finish().unwrap()
}
pub fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let mut b = PathBuilder::new();
    push_rounded(&mut b, x, y, w, h, r);
    b.finish().unwrap()
}
/// A rounded rectangle, drawn as Core Graphics draws `CGPath(roundedRect:)`.
fn push_rounded(b: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let k = 0.552_284_8 * r;
    b.move_to(x + w, y + h - r);
    b.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    b.line_to(x + r, y + h);
    b.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    b.line_to(x, y + r);
    b.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    b.line_to(x + w - r, y);
    b.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    b.close();
}
fn poly(points: &[(f32, f32)], close: bool) -> Path {
    let mut b = PathBuilder::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 { b.move_to(p.0, p.1) } else { b.line_to(p.0, p.1) }
    }
    if close {
        b.close();
    }
    b.finish().unwrap()
}

// MARK: - Icons

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    General,
    Shortcuts,
    Tones,
    Apps,
    Model,
    Updates,
    Tray,
    Wand,
    Words,
    Privacy,
    About,
    Bug,
}

impl Icon {
    /// Every icon in `Icon`, in Swift's `RisoIcon.allCases` order.
    pub const ALL: [Icon; 12] = [Icon::General, Icon::Shortcuts, Icon::Tones, Icon::Words, Icon::Apps, Icon::Model,
        Icon::Privacy, Icon::Updates, Icon::About, Icon::Tray, Icon::Bug, Icon::Wand];

    pub fn raw(self) -> &'static str {
        match self {
            Icon::General => "general",
            Icon::Shortcuts => "shortcuts",
            Icon::Tones => "tones",
            Icon::Apps => "apps",
            Icon::Model => "model",
            Icon::Updates => "updates",
            Icon::Tray => "tray",
            Icon::Wand => "wand",
            Icon::Words => "words",
            Icon::Privacy => "privacy",
            Icon::About => "about",
            Icon::Bug => "bug",
        }
    }

    fn art(self) -> [Art; 3] {
        match self {
            Icon::General => [
                |d| for (y, x) in SLIDERS { d.fill(&circle(x + 2.0, y + 2.0, 8.0), 150) },
                |d| for (y, x) in SLIDERS { d.fill(&circle(x, y, 8.0), 40) },
                |d| {
                    d.pen(4.4);
                    for (y, x) in SLIDERS {
                        d.stroke(&line(10.0, y, x - 8.0, y));
                        d.stroke(&line(x + 8.0, y, 70.0, y));
                        d.stroke(&circle(x, y, 8.0));
                    }
                },
            ],
            Icon::Shortcuts => [
                |d| d.fill(&rounded(14.0, 18.0, 56.0, 52.0, 9.0), 150),
                |d| d.fill(&rounded(18.0, 18.0, 44.0, 36.0, 6.0), 60),
                |d| {
                    d.pen(4.8);
                    d.stroke(&rounded(12.0, 14.0, 56.0, 52.0, 9.0));
                    d.pen(3.6);
                    let (cx, cy, h, r) = (40.0, 37.0, 6.0, 5.0);
                    d.stroke(&line(cx - h, cy - h, cx + h, cy - h));
                    d.stroke(&line(cx - h, cy + h, cx + h, cy + h));
                    d.stroke(&line(cx - h, cy - h, cx - h, cy + h));
                    d.stroke(&line(cx + h, cy - h, cx + h, cy + h));
                    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                        d.stroke(&circle(cx + dx * (h + r), cy + dy * (h + r), r));
                    }
                },
            ],
            Icon::Tones => [
                |d| d.fill(&bubble(), 150),
                |d| d.fill(&bubble(), 75),
                |d| {
                    d.pen(4.8);
                    d.stroke(&rounded(10.0, 14.0, 60.0, 40.0, 12.0));
                    d.stroke(&poly(&[(24.0, 54.0), (17.0, 68.0), (38.0, 54.0)], false));
                    d.pen(4.2);
                    let mut pts = vec![(20.0, 34.0)];
                    let mut x = 20.0f32;
                    while x <= 60.0 {
                        pts.push((x, 34.0 + 6.0 * ((x - 20.0) / 40.0 * 2.0 * std::f32::consts::PI).sin()));
                        x += 1.0;
                    }
                    d.stroke(&poly(&pts, false));
                },
            ],
            Icon::Apps => [
                |d| for (x, y) in TILES { d.fill(&rounded(x + 1.0, y + 1.0, 24.0, 24.0, 6.0), 150) },
                |d| for (i, (x, y)) in TILES.iter().enumerate() { d.fill(&rounded(*x, *y, 24.0, 24.0, 6.0), if i == 1 { 25 } else { 180 }) },
                |d| {
                    d.pen(4.8);
                    for (x, y) in TILES { d.stroke(&rounded(x, y, 24.0, 24.0, 6.0)) }
                },
            ],
            Icon::Model => [
                |d| d.fill(&rounded(21.0, 22.0, 40.0, 40.0, 6.0), 150),
                |d| {
                    d.fill(&rounded(20.0, 20.0, 40.0, 40.0, 6.0), 170);
                    d.fill(&rounded(30.0, 30.0, 20.0, 20.0, 3.0), 20);
                },
                |d| {
                    d.pen(4.8);
                    d.stroke(&rounded(20.0, 20.0, 40.0, 40.0, 6.0));
                    d.pen(3.6);
                    d.stroke(&rounded(30.0, 30.0, 20.0, 20.0, 3.0));
                    d.pen(4.0);
                    for i in 0..3 {
                        let c = 29.0 + i as f32 * 11.0;
                        d.stroke(&line(c, 11.0, c, 19.0));
                        d.stroke(&line(c, 61.0, c, 69.0));
                        d.stroke(&line(11.0, c, 19.0, c));
                        d.stroke(&line(61.0, c, 69.0, c));
                    }
                },
            ],
            Icon::Updates => [
                |d| {
                    d.fill(&ellipse(41.0, 67.5, 54.0, 11.0), 140);
                    d.fill(&sprout_leaf((41.0 + 2.0, 34.0 + 2.0), (66.0 + 2.0, 14.0 + 2.0), 12.0), 150);
                },
                |d| {
                    d.fill(&sprout_leaf((41.0, 34.0), (66.0, 14.0), 12.0), 40);
                    d.fill(&sprout_leaf((39.0, 46.0), (13.0, 34.0), 10.0), 170);
                },
                |d| {
                    d.pen(4.8);
                    let mut b = PathBuilder::new();
                    b.move_to(40.0, 66.0);
                    b.cubic_to(36.0, 54.0, 44.0, 44.0, 41.0, 32.0);
                    d.stroke(&b.finish().unwrap());
                    d.stroke(&sprout_leaf((39.0, 46.0), (13.0, 34.0), 10.0));
                    d.stroke(&sprout_leaf((41.0, 34.0), (66.0, 14.0), 12.0));
                    d.stroke(&line(14.0, 67.0, 66.0, 66.0));
                    d.pen(3.4);
                    d.stroke(&line(41.0, 34.0, 58.0, 21.0));
                },
            ],
            Icon::Tray => [
                |d| d.fill(&rounded(12.0, 24.0, 60.0, 44.0, 4.0), 150),
                |d| {
                    d.fill(&tray_body(), 185);
                    d.fill(&tray_front(), 60);
                },
                |d| {
                    d.pen(4.8);
                    d.stroke(&tray_body());
                    d.stroke(&tray_front());
                },
            ],
            Icon::Wand => [
                |d| d.fill(&star_at(2.0), 150),
                |d| d.fill(&star_at(0.0), 30),
                |d| {
                    d.pen(4.0);
                    d.stroke(&star_at(0.0));
                    d.pen(6.4);
                    d.stroke(&line(14.0, 68.0, 46.0, 34.0));
                    d.pen(3.6);
                    d.stroke(&line(22.0, 22.0, 26.0, 30.0));
                    d.stroke(&line(14.0, 36.0, 22.0, 38.0));
                    d.stroke(&line(62.0, 50.0, 66.0, 58.0));
                },
            ],
            Icon::Words => [
                |d| d.fill(&ellipse(42.0, 67.5, 60.0, 11.0), 140),
                |d| {
                    d.fill(&book_page(false), 60);
                    d.fill(&book_page(true), 175);
                },
                |d| {
                    d.pen(4.8);
                    d.stroke(&book_page(false));
                    d.stroke(&book_page(true));
                    d.pen(3.6);
                    for y in [36.0, 48.0] {
                        d.stroke(&line(16.0, y, 33.0, y + 1.5));
                        d.stroke(&line(64.0, y, 47.0, y + 1.5));
                    }
                },
            ],
            Icon::Privacy => [
                |d| d.fill(&rounded(18.0, 37.0, 48.0, 36.0, 7.0), 150),
                |d| d.fill(&rounded(16.0, 34.0, 48.0, 36.0, 7.0), 60),
                |d| {
                    d.pen(5.4);
                    let mut b = PathBuilder::new();
                    b.move_to(27.0, 34.0);
                    b.line_to(27.0, 25.0);
                    b.cubic_to(27.0, 8.0, 53.0, 8.0, 53.0, 25.0);
                    b.line_to(53.0, 34.0);
                    d.stroke(&b.finish().unwrap());
                    d.pen(4.8);
                    d.stroke(&rounded(16.0, 34.0, 48.0, 36.0, 7.0));
                    d.fill(&circle(40.0, 49.0, 5.0), 0);
                    d.pen(4.4);
                    d.stroke(&line(40.0, 51.0, 40.0, 60.0));
                },
            ],
            Icon::About => [
                |d| d.fill(&nib(true, 2.0), 150),
                |d| {
                    d.fill(&nib(true, 0.0), 175);
                    d.fill(&nib(false, 0.0), 45);
                },
                |d| {
                    d.pen(4.8);
                    d.stroke(&nib(true, 0.0));
                    d.stroke(&line(26.0, 24.0, 54.0, 24.0));
                    d.fill(&circle(40.0, 40.0, 4.5), 0);
                    d.pen(3.4);
                    d.stroke(&line(40.0, 44.0, 40.0, 70.0));
                },
            ],
            Icon::Bug => [
                |d| d.fill(&ellipse(40.0 + 2.0, 48.0 + 3.0, 42.0, 44.0), 150),
                |d| d.fill(&ellipse(40.0, 48.0, 42.0, 44.0), 35),
                |d| {
                    d.pen(3.8);
                    for (x1, y1, x2, y2) in [(22.0, 38.0, 11.0, 33.0), (20.0, 49.0, 9.0, 50.0), (22.0, 60.0, 12.0, 67.0)] {
                        d.stroke(&line(x1, y1, x2, y2));
                        d.stroke(&line(80.0 - x1, y1, 80.0 - x2, y2));
                    }
                    d.stroke(&line(35.0, 17.0, 29.0, 8.0));
                    d.stroke(&line(45.0, 17.0, 51.0, 8.0));
                    d.fill(&circle(40.0, 24.0, 9.0), 25);
                    d.pen(4.8);
                    d.stroke(&ellipse(40.0, 48.0, 42.0, 44.0));
                    d.pen(3.6);
                    d.stroke(&line(40.0, 30.0, 40.0, 69.0));
                    for (x, y, r) in [(30.0, 41.0, 4.6), (50.0, 41.0, 4.6), (29.0, 57.0, 3.8), (51.0, 57.0, 3.8)] {
                        d.fill(&circle(x, y, r), 25);
                    }
                },
            ],
        }
    }

    /// `RisoIcon.image()`: printed at 44 pixels (shown at 22 points, a pixel a dot at 2×).
    /// The icon `points` square, crisp (`RisoIconView`: the 44-pixel print, `.interpolation(.none)`).
    pub fn image_at(self, points: f32, scale: f32) -> Pixels {
        riso::cached_crisp(format!("icon-{}", self.raw()), 44, || {
            let [shade, pink, inked] = self.art();
            print(44, str_seed(self.raw(), 11), &[(ink::CREMA, 2, 2, shade), (ink::PINK, -1, -1, pink), (ink::ESPRESSO, 0, 0, inked)])
        }, points, scale)
    }

    pub fn image(self) -> Pixels {
        let plate = Plate { w: 44.0, h: 44.0, scale: 1.0 };
        riso::cached(format!("icon-{}", self.raw()), &plate, || {
            let [shade, pink, inked] = self.art();
            print(44, str_seed(self.raw(), 11), &[(ink::CREMA, 2, 2, shade), (ink::PINK, -1, -1, pink), (ink::ESPRESSO, 0, 0, inked)])
        })
    }
}

const SLIDERS: [(f32, f32); 3] = [(20.0, 52.0), (40.0, 26.0), (60.0, 44.0)];
const TILES: [(f32, f32); 4] = [(12.0, 12.0), (44.0, 12.0), (12.0, 44.0), (44.0, 44.0)];

fn bubble() -> Path {
    let mut b = PathBuilder::new();
    push_rounded(&mut b, 10.0, 14.0, 60.0, 40.0, 12.0);
    b.move_to(24.0, 52.0);
    b.line_to(17.0, 68.0);
    b.line_to(38.0, 53.0);
    b.finish().unwrap()
}

fn sprout_leaf(base: (f32, f32), tip: (f32, f32), bulge: f32) -> Path {
    let mid = ((base.0 + tip.0) / 2.0, (base.1 + tip.1) / 2.0);
    let (dx, dy) = (tip.0 - base.0, tip.1 - base.1);
    let len = dx.hypot(dy);
    let n = (-dy / len * bulge, dx / len * bulge);
    let mut b = PathBuilder::new();
    b.move_to(base.0, base.1);
    b.quad_to(mid.0 + n.0, mid.1 + n.1, tip.0, tip.1);
    b.quad_to(mid.0 - n.0, mid.1 - n.1, base.0, base.1);
    b.finish().unwrap()
}

fn tray_body() -> Path {
    poly(&[(10.0, 42.0), (22.0, 20.0), (58.0, 20.0), (70.0, 42.0), (70.0, 64.0), (10.0, 64.0)], true)
}
fn tray_front() -> Path {
    poly(&[(10.0, 42.0), (27.0, 42.0), (31.0, 50.0), (49.0, 50.0), (53.0, 42.0), (70.0, 42.0), (70.0, 64.0), (10.0, 64.0)], true)
}

/// The wand's star, nudged by `d` for its shade.
fn star_at(d: f32) -> Path {
    let pts: Vec<(f32, f32)> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::PI / 2.0 + i as f32 * std::f32::consts::PI / 5.0;
            let r = if i % 2 == 0 { 15.0 } else { 6.75 };
            (56.0 + r * a.cos() + d, 22.0 + r * a.sin() + d)
        })
        .collect();
    poly(&pts, true)
}

fn book_page(mirror: bool) -> Path {
    let f = |x: f32| if mirror { 80.0 - x } else { x };
    let mut b = PathBuilder::new();
    b.move_to(40.0, 26.0);
    b.quad_to(f(26.0), 15.0, f(9.0), 22.0);
    b.line_to(f(9.0), 62.0);
    b.quad_to(f(26.0), 55.0, 40.0, 66.0);
    b.close();
    b.finish().unwrap()
}

/// The pen nib's outline, or its left half; nudged by `d`.
fn nib(whole: bool, d: f32) -> Path {
    let mut b = PathBuilder::new();
    b.move_to(40.0 + d, 72.0 + d);
    b.cubic_to(32.0 + d, 58.0 + d, 20.0 + d, 44.0 + d, 22.0 + d, 30.0 + d);
    b.line_to(28.0 + d, 12.0 + d);
    if whole {
        b.line_to(52.0 + d, 12.0 + d);
        b.line_to(58.0 + d, 30.0 + d);
        b.cubic_to(60.0 + d, 44.0 + d, 48.0 + d, 58.0 + d, 40.0 + d, 72.0 + d);
    } else {
        b.line_to(40.0 + d, 12.0 + d);
    }
    b.close();
    b.finish().unwrap()
}

/// Every riso icon Iris draws, by Swift's names (`RisoIcon.allCases` order), at `points` square:
/// the rail's, and the ones single pages draw (the clock, the hand, the paper plane).
pub fn every(points: f32, scale: f32) -> Vec<(&'static str, Pixels)> {
    let mut out: Vec<(&'static str, Pixels)> = Icon::ALL[..9].iter().map(|i| (i.raw(), i.image_at(points, scale))).collect();
    out.push(("clock", crate::pages::model::clock(points, scale)));
    out.push(("hand", crate::pages::privacy::hand(points, scale)));
    out.push(("paperPlane", crate::pages::privacy::paper_plane(points, scale)));
    out.extend(Icon::ALL[9..].iter().map(|i| (i.raw(), i.image_at(points, scale))));
    out
}

/// Swift's icon sheet (`after-icons`, Snapshot.swift): each icon at 72 points with its name
/// under it, 18 apart, then each at 24, 26 apart, on the panel's paper, 24 of padding. Laid out
/// as Swift's so `gpui/scripts/compare-icons.py` can find each icon in both and compare them.
pub fn sheet(scale: f32) -> gpui::Div {
    use gpui::{ParentElement, Styled, div};
    use crate::parts::{printed, t, caption, z};
    let mut big = div().flex().flex_row().gap(z(18.0)).items_start();
    for (name, img) in every(72.0, scale) {
        big = big.child(div().flex().flex_col().items_center().gap(z(8.0)).w(z(72.0))
            .child(printed(72.0, 72.0, &img))
            .child(div().w(z(72.0)).whitespace_normal().child(t(name, caption(), crate::riso::hue::I10))));
    }
    let mut small = div().flex().flex_row().gap(z(26.0)).items_center();
    for (_, img) in every(24.0, scale) {
        small = small.child(div().w(z(46.0)).flex().justify_center().child(printed(24.0, 24.0, &img)));
    }
    div().p(z(24.0)).bg(crate::riso::col(crate::riso::hue::I1)).flex().flex_col().gap(z(20.0)).child(big).child(small)
}
