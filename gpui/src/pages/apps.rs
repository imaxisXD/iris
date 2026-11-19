//! Settings › Apps (`AppsPage` in SettingsView.swift): the apps Iris is turned off in, drawn
//! like the zine's brewing methods (each app big, its name on an ink label, a brush X over it,
//! the lot set a few degrees off straight, so printed whole), and the apps that start in a tone
//! of their own (none yet: the empty state).

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{Div, InteractiveElement, ParentElement, Role, StatefulInteractiveElement, Styled};

use crate::controls::{self as c, Button};
use crate::interact::act;
use crate::native_menu::{Item, Picture};
use crate::riso::hue;
use crate::store::AppRef;

use crate::icons::{self, Draw, rounded};
use crate::parts::*;
use crate::riso::{self, Plate, ink};
use crate::settings::{Ctx, Values};
use crate::settings_kit::*;

/// The page's sections, `w` points wide.
pub fn content(v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    let s = cx.scale;
    // `LazyVGrid`: four flexible columns 12 apart, rows 22 apart.
    let cell_w = (w - 3.0 * 12.0) / 4.0;
    let mut grid = column().w(z(w)).gap(z(22.0)).pt(z(6.0));
    for (r, chunk) in v.excluded.chunks(4).enumerate() {
        let mut line = row().w(z(w)).gap(z(12.0)).items_start();
        for (k, (bundle, name)) in chunk.iter().enumerate() {
            line = line.child(turned_off(r * 4 + k, bundle, name, cell_w, s));
        }
        grid = grid.child(line);
    }
    let tone = if v.per_app.is_empty() {
        empty_state(&["Every app opens in the same tone.", "Pick an app to give it its own."], w)
    } else {
        let mut table = column().w(z(w));
        let n = v.per_app.len();
        for (i, (bundle, tone)) in v.per_app.iter().enumerate() {
            table = table.child(per_app_row(v, i, n, bundle, tone, w, s));
        }
        table
    };
    let (off, off_w) = c::menu_button(&Button::secondary("Turn off in an app").icon("plus"), s, app_items, |i, _| {
        if let Some((bundle, name)) = picked_app(i) {
            crate::app::change(|s| {
                if !s.excluded_apps.iter().any(|a| a.bundle_id == bundle) {
                    s.excluded_apps.push(AppRef { bundle_id: bundle.clone(), name: name.clone() });
                }
            });
        }
    });
    let (pick, pick_w) = c::menu_button(&Button::secondary("Pick an app").icon("plus"), s, app_items, |i, _| {
        if let Some((bundle, _)) = picked_app(i) {
            crate::tones::change(|l| {
                if !l.per_app.iter().any(|(b, _)| *b == bundle) {
                    l.per_app.push((bundle.clone(), "fix".into()));
                    l.per_app.sort();
                }
            });
        }
    });
    vec![
        section_wrapped(1, "Turned off in", Some("Iris won't read text in these apps. Password managers are on the list from the start."),
            Some((off, off_w)), grid, w, s),
        section_wrapped(2, "Start in a tone", Some("For example Professional in Mail. Other apps open in the tone from General."),
            Some((pick, pick_w)), tone, w, s),
    ]
}

/// The menu both buttons pop: every app in the Applications folders with its own icon, then
/// Choose another app… (`AppsPage.appItems`).
fn app_items() -> Vec<Item> {
    let mut items: Vec<Item> = installed_apps().iter().map(|(_, name, path)| Item { title: name.clone(), picture: Picture::App(path.clone()) }).collect();
    items.push(Item { title: "Choose another app…".into(), picture: Picture::Png(apps_icon_png()) });
    items
}

/// The apps the menu listed, read once a window (the folders don't change while it's open).
fn installed_apps() -> std::rc::Rc<Vec<(String, String, String)>> {
    thread_local! { static APPS: RefCell<Option<(std::time::Instant, std::rc::Rc<Vec<(String, String, String)>>)>> = const { RefCell::new(None) }; }
    APPS.with(|a| {
        let mut a = a.borrow_mut();
        if let Some((at, list)) = a.as_ref() && at.elapsed().as_secs() < 30 {
            return list.clone();
        }
        let list = std::rc::Rc::new(crate::appicon::installed());
        *a = Some((std::time::Instant::now(), list.clone()));
        list
    })
}

/// What the menu's `i`th item names: (bundle ID, name); the last opens the Mac's file panel.
fn picked_app(i: usize) -> Option<(String, String)> {
    let apps = installed_apps();
    match apps.get(i) {
        Some((id, name, _)) => Some((id.clone(), name.clone())),
        None => crate::native_menu::choose_app(),
    }
}

/// `RisoIcon.apps` as a PNG, for the menu's last item.
fn apps_icon_png() -> Vec<u8> {
    let rgba = faded_apps_icon(44);
    let mut out = Vec::new();
    let img = image::RgbaImage::from_raw(44, 44, rgba).unwrap();
    let _ = img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png);
    out
}

/// A row of Start in a tone: the app's icon and name, the tone it opens in, Remove.
fn per_app_row(v: &Values, i: usize, n: usize, bundle: &str, tone: &str, w: f32, s: f32) -> gpui::AnyElement {
    let labels: Vec<String> = v.modes.iter().map(|m| m.1.clone()).collect();
    let ids: Vec<String> = v.modes.iter().map(|m| m.0.clone()).collect();
    let selected = ids.iter().position(|x| x == tone).unwrap_or(0);
    let b = bundle.to_string();
    let menu = c::menu_id(&format!("app-tone-{bundle}"), labels, selected, None, 150.0, move |k, _| {
        let id = ids[k].clone();
        let b = b.clone();
        crate::tones::change(|l| {
            if let Some(e) = l.per_app.iter_mut().find(|(x, _)| *x == b) {
                e.1 = id.clone();
            }
        });
    });
    let name = crate::appicon::name_of(bundle);
    let gone = bundle.to_string();
    let (remove, _) = c::button(&Button::ghost("Remove").compact().label(format!("Remove {name}")).id(format!("remove-app-tone-{bundle}")), s,
        act(move |_, _| crate::tones::change(|l| l.per_app.retain(|(x, _)| *x != gone))));
    let r = row().gap(z(12.0)).items_center()
        .child(small_icon(bundle, s))
        .child(t(name, body(), hue::I12))
        .child(grow())
        .child(menu)
        .child(remove);
    c::table_row(&format!("app-tone-{bundle}"), r, i == n - 1, i as i64, w, 1.0)
}

/// `AppIconView` at 18 points: the app's own icon, or Iris's apps icon faded when it isn't on
/// this Mac.
fn small_icon(bundle: &str, scale: f32) -> gpui::AnyElement {
    let side = (18.0 * scale).round() as u32;
    let plate = Plate { w: 18.0, h: 18.0, scale };
    let img = riso::cached(format!("settings-apps-small-{bundle}-{side}"), &plate, || {
        crate::appicon::path_of(bundle).and_then(|p| crate::appicon::of_app(&p, side)).unwrap_or_else(|| {
            let mut rgba = faded_apps_icon(side);
            for a in rgba.iter_mut().skip(3).step_by(4) {
                *a = (*a as f32 * 0.45).round() as u8;
            }
            rgba
        })
    });
    printed(18.0, 18.0, &img)
}

/// One app Iris is off in: its icon big under a brush X (turned a few degrees, printed whole),
/// its name on an ink label, Remove.
fn turned_off(i: usize, bundle: &str, name: &str, w: f32, scale: f32) -> gpui::Stateful<Div> {
    let seed = i as i64 * 5 + 2;
    let degrees = if i % 2 == 0 { -4.0 } else { 3.0 };
    let side = (46.0 * scale).round() as u32;
    let icon = app_icon(bundle, side);
    let crossed = turned(format!("settings-apps-crossed-{bundle}-{seed}"), 56.0, 56.0, 6.0, degrees, scale, |sh| {
        match &icon {
            Some(rgba) => sh.picture(rgba, 46.0, 46.0, 5.0, 5.0),
            // An app that isn't on this Mac: Iris's own apps icon, faded.
            None => {
                let mut rgba = faded_apps_icon(side);
                for a in rgba.iter_mut().skip(3).step_by(4) {
                    *a = (*a as f32 * 0.45).round() as u8;
                }
                sh.picture(&rgba, 46.0, 46.0, 5.0, 5.0);
            }
        }
        let plate = Plate { w: 56.0, h: 56.0, scale };
        sh.picture(&dry_brush_x_rgba(ink::INK, seed, 0.12, &plate, tiny_skia::Transform::identity()), 56.0, 56.0, 0.0, 0.0);
    });
    let (label, _) = ink_label(name, 10.5, scale);
    column().w(z(w)).gap(z(9.0)).items_center()
        .child(sized(56.0, 60.0).child(at(crossed, 0.0, 2.0)))
        .child(label)
        .child(c::button(&Button::ghost("Remove").compact().label(format!("Remove {name}")).id(format!("remove-app-{bundle}")), scale, {
            let gone = bundle.to_string();
            act(move |_, _| crate::app::change(|s| s.excluded_apps.retain(|a| a.bundle_id != gone)))
        }).0)
        .id(gpui::SharedString::from(format!("settings-app-{bundle}"))).role(Role::Group).aria_label(name.to_string())
}

/// `RisoIcon.apps` (44 pixels) blown up to `side` pixels without smoothing
/// (`.interpolation(.none)`), for an app that isn't on this Mac.
fn faded_apps_icon(side: u32) -> Vec<u8> {
    const TILES: [(f32, f32); 4] = [(12.0, 12.0), (44.0, 12.0), (12.0, 44.0), (44.0, 44.0)];
    let art: [fn(&mut Draw); 3] = [
        |d| for (x, y) in TILES { d.fill(&rounded(x + 1.0, y + 1.0, 24.0, 24.0, 6.0), 150) },
        |d| for (i, (x, y)) in TILES.iter().enumerate() { d.fill(&rounded(*x, *y, 24.0, 24.0, 6.0), if i == 1 { 25 } else { 180 }) },
        |d| {
            d.pen(4.8);
            for (x, y) in TILES { d.stroke(&rounded(x, y, 24.0, 24.0, 6.0)) }
        },
    ];
    let small = icons::print(44, riso::str_seed("apps", 11), &[(ink::CREMA, 2, 2, art[0]), (ink::PINK, -1, -1, art[1]), (ink::ESPRESSO, 0, 0, art[2])]);
    let mut out = vec![0u8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            let (sx, sy) = (x * 44 / side, y * 44 / side);
            let s = ((sy * 44 + sx) * 4) as usize;
            let d = ((y * side + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&small[s..s + 4]);
        }
    }
    out
}

/// The real icon of an app on this Mac (`AppIconView`), `side` pixels square, found once.
fn app_icon(bundle: &str, side: u32) -> Option<Vec<u8>> {
    thread_local! { static ICONS: RefCell<HashMap<String, Option<Vec<u8>>>> = RefCell::new(HashMap::new()); }
    let key = format!("{bundle}@{side}");
    if let Some(hit) = ICONS.with(|m| m.borrow().get(&key).cloned()) {
        return hit;
    }
    // As the Mac draws it (`NSWorkspace.icon(forFile:)`, what Swift shows).
    let found = crate::appicon::path_of(bundle).and_then(|p| crate::appicon::of_app(&p, side));
    ICONS.with(|m| m.borrow_mut().insert(key, found.clone()));
    found
}
