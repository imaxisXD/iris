//! Settings › Privacy (`PrivacyPage` in SettingsView.swift): one section, four rows, each a
//! riso icon beside what Iris promises.

use gpui::{Div, ParentElement, Styled};
use tiny_skia::{Path, PathBuilder, Transform};

use crate::icons::{self, Draw, Icon, line, rounded};
use crate::parts::*;
use crate::riso::{self, Img, hue, ink, str_seed};
use crate::settings::{Ctx, Values};
use crate::settings_kit::*;

/// The page's sections, `w` points wide.
pub fn content(_v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    let sc = cx.scale;
    let rows = column().w(z(w)).gap(z(12.0))
        .child(item(Icon::Model.image_at(26.0, sc), "Runs on your Mac", "The model runs on this Mac. Your text is never sent to a server.", w))
        // `Hairline()` takes the line it's called on as its seed.
        .child(hairline(955, w))
        .child(item(hand(26.0, sc), "Reads only when you ask", "Only after a shortcut, only the box you're in, never password fields.", w))
        .child(hairline(957, w))
        .child(item(paper_plane(26.0, sc), "Never sends for you", "Iris replaces the text; you still press send. Read it first, especially for customers.", w))
        .child(hairline(959, w))
        .child(item(Icon::Tray.image_at(26.0, sc), "Keeps no history", "Your text isn't saved. Settings stay on this Mac.", w));
    vec![section_wrapped(1, "Privacy", None, None, rows, w, cx.scale)]
}

/// A row: the icon at 26 points, lifted 3, then the title over its detail.
fn item(icon: Img, title: &str, detail: &str, w: f32) -> Div {
    let words_w = w - 26.0 - 12.0;
    row().w(z(w)).gap(z(12.0)).items_start()
        .child(sized(26.0, 23.0).child(at(printed(26.0, 26.0, &icon), 0.0, -3.0)))
        .child(column().w(z(words_w)).gap(z(3.0))
            .child(t(title, body(), hue::I12))
            .child(mono_paragraph(detail, caption(), hue::I10, words_w, 11.0 * MONO_LINE)))
}

// MARK: - Icons this page needs that the rail doesn't

/// Printed at 44 pixels like every riso icon (`RisoIcon.image()`), seeded by its name.
fn icon(name: &str, art: [fn(&mut Draw); 3], points: f32, scale: f32) -> Img {
    riso::cached_crisp(format!("settings-privacy-icon-{name}"), 44, || {
        icons::print(44, str_seed(name, 11), &[(ink::CREMA, 2, 2, art[0]), (ink::PINK, -1, -1, art[1]), (ink::ESPRESSO, 0, 0, art[2])])
    }, points, scale)
}

/// `RisoIcon.hand`: a raised hand, its palm and fingers one outline.
pub fn hand(points: f32, scale: f32) -> Img {
    icon("hand", [
        |d| for p in palm(0.0) { d.fill(&p.transform(Transform::from_translate(1.0, 2.0)).unwrap(), 150) },
        |d| for p in palm(0.0) { d.fill(&p, 85) },
        |d| {
            // Swift strokes the union of the pieces: stroke each, then paper over the inside.
            d.pen(4.2);
            for p in palm(0.0) { d.stroke(&p) }
            for p in palm(2.1) { d.fill(&p, 255) }
        },
    ], points, scale)
}

/// The palm, four fingers and the thumb, each pulled in by `inset`.
pub fn palm(inset: f32) -> Vec<Path> {
    let r = |x: f32, y: f32, w: f32, h: f32, rad: f32| rounded(x + inset, y + inset, w - 2.0 * inset, h - 2.0 * inset, (rad - inset).max(0.1));
    let mut parts = vec![r(22.0, 38.0, 36.0, 30.0, 11.0)];
    for (x, top) in [(23.0, 22.0), (32.0, 15.0), (41.0, 17.0), (50.0, 23.0)] {
        parts.push(r(x, top, 8.5, 50.0 - top, 4.25));
    }
    // `CGAffineTransform(translationX: 16, y: 40).rotated(by: -0.6)`: turned, then moved.
    let turn = Transform::from_translate(16.0, 40.0).pre_concat(Transform::from_rotate((-0.6f32).to_degrees()));
    parts.push(r(0.0, 0.0, 9.0, 22.0, 4.5).transform(turn).unwrap());
    parts
}

/// `RisoIcon.paperPlane`: a paper plane, its near wing pink.
pub fn paper_plane(points: f32, scale: f32) -> Img {
    icon("paperPlane", [
        |d| d.fill(&icons::ellipse(34.0, 70.0, 40.0, 8.0), 140),
        |d| {
            d.fill(&plane_outline(), 170);
            d.fill(&near_wing(), 30);
        },
        |d| {
            d.pen(4.8);
            d.stroke(&plane_outline());
            d.pen(3.6);
            d.stroke(&line(68.0, 14.0, 32.0, 45.0));
            d.stroke(&line(8.0, 56.0, 18.0, 52.0));
            d.stroke(&line(14.0, 64.0, 24.0, 60.0));
        },
    ], points, scale)
}

fn closed(points: &[(f32, f32)]) -> Path {
    let mut b = PathBuilder::new();
    for (i, p) in points.iter().enumerate() {
        if i == 0 { b.move_to(p.0, p.1) } else { b.line_to(p.0, p.1) }
    }
    b.close();
    b.finish().unwrap()
}

fn plane_outline() -> Path {
    closed(&[(68.0, 14.0), (10.0, 38.0), (32.0, 45.0), (40.0, 64.0), (68.0, 14.0)])
}

fn near_wing() -> Path {
    closed(&[(68.0, 14.0), (32.0, 45.0), (40.0, 64.0)])
}
