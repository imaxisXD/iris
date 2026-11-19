//! The scenes drawn headlessly to set beside the Swift app's design pictures
//! (`--snap <png> --scene <name> --size WxH [--at t]`), named as the Rune copy names them.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

use crate::opening;
use crate::page::{Outcome, Page};
use crate::panel;
use crate::parts::printed;
use crate::riso::{self, Plate};
use crate::slip::{self, Engine, Slip, rows};

/// The scene `name` at moment `at`, or `None` for a name it doesn't know.
pub fn build(name: &str, at: f64, scale: f32, paper_h: &Rc<Cell<f32>>) -> Option<AnyElement> {
    Some(match name {
        "opening-full" | "opening-quick" | "opening-away" => {
            let look = panel::Look { scale, icon: &crate::app_icon, pick: None, flight: None, act: None };
            let script = match name { "opening-quick" => opening::QUICK, "opening-away" => opening::AWAY, _ => opening::FULL };
            let (w, h, rgba) = opening::rig_rgba(at, &script, scale);
            let rig = riso::image(w, h, rgba);
            let shown = opening::panel_state();
            opening::scene(at, &script, &shown, &look, paper_h.get(), paper_h.clone(), Some(&rig)).into_any_element()
        }
        "slips" => slips_sheet(scale).into_any_element(),
        "toasts" => crate::toast::sheet(scale).into_any_element(),
        // One message alone on paper (`toast-<id>`), for comparing with Swift's `--toast-states`.
        _ if name.starts_with("toast-") => {
            let (_, m) = crate::toast::states().into_iter().find(|(id, _)| *id == &name["toast-".len()..])?;
            div().p(px(14.0)).child(crate::toast::toast(&m, scale)).into_any_element()
        }
        "note-demos" => crate::pages::notes::demos_sheet(scale).into_any_element(),
        "note-demo-keys" => crate::pages::notes::keys_sheet(scale).into_any_element(),
        // Every riso icon at 72 and at 24 points (Swift's `after-icons`).
        "icons" => crate::icons::sheet(scale).into_any_element(),
        // One row of every icon at `<pt>` points, 24 in and `pt + 26` apart: templates for
        // compare-icons-live.py to find each icon in a live window.
        _ if name.starts_with("icons-") => {
            let pt: f32 = name["icons-".len()..].parse().ok()?;
            let mut r = div().flex().flex_row().gap(px(26.0)).p(px(24.0)).bg(crate::riso::col(crate::riso::hue::I1));
            for (_, img) in crate::icons::every(pt, scale) {
                r = r.child(crate::parts::printed(pt, pt, &img));
            }
            r.into_any_element()
        }
        // Every control in every state (Swift's `after-buttons`).
        "components" => crate::components::sheet(scale).into_any_element(),
        "menubar-hint" => crate::toast::hint_sheet(scale).into_any_element(),
        "live-slip" => div().pl(px(18.0)).pr(px(18.0)).pb(px(26.0))
            .child(slip::slip(&live_slip_state(), 1000.0 + at, scale)).into_any_element(),
        // The live window (`settings::window_live`) as it opens, its page scrolled to the top:
        // `--size 940x768`.
        "settings-live" => {
            let press = |_: crate::settings::Page| -> Box<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App)> { Box::new(|_, _, _| {}) };
            let turn = crate::settings::Turn { press: &press };
            crate::settings::window_live(crate::settings::Page::General, &crate::settings::Values::default(), &turn, &crate::settings::Ctx { scale }).into_any_element()
        }
        // The README's picture of Words (`ReadmeImages`): the page with sample words in it.
        "readme-words" => {
            let page = crate::settings::Page::Words;
            let height = crate::settings::asked_height().unwrap_or(page.snapshot_height());
            let mut v = crate::settings::Values::default();
            v.glossary = ["Northwind", "OKR", "Kubernetes", "ACME-2041"].map(String::from).to_vec();
            crate::settings::window(page, &v, height, &crate::settings::Ctx { scale }).into_any_element()
        }
        // Each Settings page whole, as tall as `--size` asks (the Swift picture's height when it
        // doesn't say: `settings::Page::snapshot_height`).
        _ if name.strip_prefix("settings-").and_then(crate::settings::Page::from_slug).is_some() => {
            let page = crate::settings::Page::from_slug(&name["settings-".len()..])?;
            let height = crate::settings::asked_height().unwrap_or(page.snapshot_height());
            crate::settings::window(page, &crate::settings::Values::default(), height, &crate::settings::Ctx { scale }).into_any_element()
        }
        // Each first-time setup state whole (`setup-1-allow`, …), as tall as `--size` asks.
        _ if name.starts_with("setup-") => crate::setup::scene(&name["setup-".len()..], scale)?.into_any_element(),
        _ => return None,
    })
}

/// The slip as the live slip opens on: resting, the model not loaded.
pub fn live_slip_state() -> Slip {
    Slip { engine: Engine::Idle, groups: rows(false, true, false), highlighted: None, busy: vec![], setup_line: None, setup_garden: None, problem_line: None, updating: None, page: None }
}

/// A desktop's gradient behind the slips, as the Swift snapshot paints it: three stops from top
/// left to bottom right of the whole sheet (1320 points wide, centred, so its ends overflow),
/// in SwiftUI's unit space. GPUI's gradients take two stops, so it's printed.
fn desktop(w: f32, h: f32, scale: f32) -> riso::Img {
    let plate = Plate { w, h, scale };
    riso::cached("desktop".into(), &plate, || {
        let (pw, ph) = plate.px();
        let stops = [(0x2B, 0x2A, 0x33), (0x6A, 0x4F, 0x8C), (0x1F, 0x6F, 0x78)];
        let (x0, full_w) = (-130.0, 1320.0);
        let mut out = Vec::with_capacity((pw * ph * 4) as usize);
        for y in 0..ph {
            for x in 0..pw {
                let (px_, py) = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
                let t = (((px_ - x0) / full_w + py / h) / 2.0).clamp(0.0, 1.0);
                let (a, b, k) = if t < 0.5 { (stops[0], stops[1], t / 0.5) } else { (stops[1], stops[2], (t - 0.5) / 0.5) };
                let m = |u: u8, v: u8| (u as f32 + (v as f32 - u as f32) * k).round() as u8;
                out.extend_from_slice(&[m(a.0, b.0), m(a.1, b.1), m(a.2, b.2), 255]);
            }
        }
        out
    })
}

/// The menu bar slip in each state the Swift snapshot draws (`after-status-menu`), laid out as
/// it lays them out: two rows over a desktop gradient, 1060 points wide, overflowing at the sides.
fn slips_sheet(scale: f32) -> gpui::Div {
    let base = |engine, groups, highlighted| Slip { engine, groups, highlighted, busy: vec![], setup_line: None, setup_garden: None, problem_line: None, updating: None, page: None };
    let resting = base(Engine::Idle, rows(false, true, false), Some("check"));
    let downloading = Slip { busy: vec!["model"], ..base(Engine::Downloading(0.42), rows(false, true, false), None) };
    let taken = Slip { problem_line: Some(slip::shortcut_taken_line()), ..base(Engine::Ready, rows(true, true, true), Some("check")) };
    let setup = Slip { setup_line: Some("Step 2 of 3: get the model.".into()), ..base(Engine::Idle, rows(false, false, false), Some("check")) };
    let updating = Slip { updating: Some(0.8), ..base(Engine::Ready, rows(false, true, true), None) };
    let t = 1000.0;
    let row = || div().flex().flex_row().gap(px(40.0)).items_start().ml(px(-293.0)).flex_none();
    let mut first = row();
    for st in [resting, downloading, taken, setup] {
        first = first.child(slip::slip(&st, t, scale));
    }
    let up_to_date = Slip { page: Some(Page { title: "Up to date".into(), lead: "You have the latest Iris.".into(), outcome: Outcome::UpToDate, changes: vec![], button: None, warning: false }),
        ..base(Engine::Ready, rows(false, true, true), None) };
    let changes = Slip { page: Some(Page { title: "2 new changes".into(), lead: "Ready to install.".into(), outcome: Outcome::Changes(2),
        changes: vec!["The menu bar flower in pen, with the pink plate".into(), "Names each fix in the margin".into()], button: None, warning: false }),
        ..base(Engine::Ready, rows(false, true, true), None) };
    let second = row()
        .child(slip::slip(&up_to_date, t, scale))
        .child(slip::slip(&changes, t, scale))
        .child(slip::slip(&updating, t, scale));
    div().relative().w(px(1060.0)).h_full().overflow_hidden()
        .child(div().absolute().top_0().left_0().child(printed(1060.0, 958.0, &desktop(1060.0, 958.0, scale))))
        .child(div().flex().flex_col().pt(px(28.0)).px(px(28.0)).gap(px(40.0)).child(first).child(second))
}
