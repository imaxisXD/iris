//! Every control in every state, side by side: Swift's `after-buttons` sheet (Snapshot.swift,
//! `stateRow`), drawn through the same code the live windows run, each cell with its state
//! forced (`interact::forcing`). `--snap … --scene components` draws it to compare with Swift's;
//! `--components x y` opens it live on the test display, with a row of controls left to the
//! pointer and keyboard under it.

use gpui::*;

use crate::controls::{self, Button};
use crate::interact::{self as live, Interaction, Live};
use crate::parts::*;
use crate::riso::{col, hue};

/// `DS.Font.micro` in capitals, tracked 0.4 (`caption` in Snapshot.swift).
fn label(s: &str) -> Div {
    // SwiftUI gives the line whole points (11.4 takes 12).
    div().flex_none().h(z(line_box(9.5, MONO_LINE))).child(t(s.to_uppercase(), martian(9.5, 450).tracked(0.4), hue::I10))
}

/// A control over its caption, 8 apart.
fn cell(e: impl IntoElement, words: &str) -> Div {
    div().flex().flex_col().flex_none().gap(z(8.0)).items_start().child(e).child(label(words))
}

fn nothing() -> live::Act {
    live::act(|_, _| {})
}

/// `stateRow`: rest, hover, pressed, keyboard focus, disabled.
fn state_row(title: &str, make: &dyn Fn(bool) -> AnyElement) -> Div {
    let forced = |st: Interaction, words: &str| cell(live::forcing(st, || make(true)), words);
    div().flex().flex_row().gap(z(14.0)).items_center()
        .child(forced(Interaction::REST, &format!("{title} · rest")))
        .child(forced(Interaction::HOVER, "hover"))
        .child(forced(Interaction::PRESSED, "pressed"))
        .child(forced(Interaction::FOCUSED, "keyboard focus"))
        .child(cell(live::forcing(Interaction::REST, || make(false)), "disabled"))
}

/// The sheet, 700 points wide on the panel's paper (`DS.Surface.panel`, 24 of padding).
pub fn sheet(scale: f32) -> Div {
    let l = live::get().unwrap_or_else(Live::still);
    live::with(l, || build(scale))
}

fn build(scale: f32) -> Div {
    let primary = move |on: bool| controls::button(&Button::primary("Replace").key_chip("↵").enabled(on), scale, nothing()).0;
    let secondary = move |on: bool| controls::button(&Button::secondary("Open setup").enabled(on), scale, nothing()).0;
    let rest = |f: &dyn Fn() -> AnyElement| live::forcing(Interaction::REST, f);
    let focused = |f: &dyn Fn() -> AnyElement| live::forcing(Interaction::FOCUSED, f);

    let third = div().flex().flex_row().gap(z(14.0)).items_center()
        .child(cell(rest(&|| controls::button(&Button::ghost("Remove"), scale, nothing()).0), "Ghost"))
        .child(cell(rest(&|| controls::button(&Button::new("Yes, delete", controls::Kind::DestructiveConfirm), scale, nothing()).0), "Final confirm"))
        .child(cell(rest(&|| div().flex().flex_row().gap(z(2.0))
            .child(controls::icon_button("chevron.up", true, "Up", nothing()))
            .child(controls::icon_button("trash", true, "Delete", nothing()))
            .into_any_element()), "Icon"));
    let mut caps = div().flex().flex_row().gap(z(6.0));
    for k in ["↵", "⇥", "⌘C", "esc"] {
        caps = caps.child(keycap(k, scale));
    }
    let keycaps = div().flex().flex_row().child(cell(caps, "Keycaps"));
    let focus_row = div().flex().flex_row().gap(z(26.0)).items_center().py(z(8.0))
        .child(cell(focused(&|| controls::toggle(true, "On", scale, nothing())), "Switch, focused"))
        .child(cell(focused(&|| controls::checkbox(true, 1.0, "On", scale, Some(nothing()))), "Check, focused"))
        .child(cell(focused(&|| controls::choice(&["Panel", "Message"], 0, "x", scale, |_| nothing())), "Choice, focused"))
        .child(cell(focused(&|| controls::menu(vec!["Fix".into()], 0, None, 120.0, |_, _| {})), "Picker, focused"));
    let plain = div().flex().flex_row().gap(z(14.0)).items_center()
        .child(cell(rest(&|| div().flex().flex_row().gap(z(8.0))
            .child(controls::toggle(true, "On", scale, nothing()))
            .child(controls::toggle(false, "Off", scale, nothing()))
            .into_any_element()), "Switch"))
        .child(cell(rest(&|| div().flex().flex_row().gap(z(8.0))
            .child(controls::checkbox(true, 1.0, "On", scale, Some(nothing())))
            .child(controls::checkbox(false, 1.0, "Off", scale, Some(nothing())))
            .into_any_element()), "Check"));
    let field = div().flex().flex_row().gap(z(14.0))
        .child(cell(crate::settings_kit::well_field("Add a word or phrase", 260.0), "Field · rest"))
        .child(cell(crate::text_field::sheet_face("Northwind", 260.0), "Field · typing"));
    use crate::store::KeyCombo;
    let rec = |listening: bool, problem: Option<crate::tones::Problem>| {
        let r = controls::Recorder { keys: Some(KeyCombo::QUICK_FIX), action: 2, listening, allows_clear: true, pending: problem.is_some(), problem };
        rest(&|| controls::recorder(&r, scale, None).0)
    };
    let recorders = div().flex().flex_row().gap(z(26.0)).items_start()
        .child(cell(rec(false, None), "Recorder · rest"))
        .child(cell(rec(true, None), "listening"));
    let warning = div().flex().flex_row().child(cell(rec(false, Some(crate::tones::Problem::CommonConflict("Bold in Slack and most editors"))), "Recorder · warning"));
    // The panel's tone tabs (`ToneTabs`): another tone at rest, under the pointer, held.
    let tab = |st: Interaction| -> AnyElement {
        live::forcing(st, || {
            let l = live::get().unwrap_or_else(Live::still);
            let (w, h) = tone_tab_size("Friendly");
            live::pressable(&l, "sheet-tab", w, h, 0.97, Some(nothing()), |hover| tone_tab("Friendly", false, hover).into_any_element()).into_any_element()
        })
    };
    let tabs = div().flex().flex_row().gap(z(14.0)).items_end()
        .child(cell(tab(Interaction::REST), "Tone · rest"))
        .child(cell(tab(Interaction::HOVER), "hover"))
        .child(cell(tab(Interaction::PRESSED), "pressed"));
    div().w(z(700.0)).p(z(24.0)).bg(col(hue::I1)).flex().flex_col().gap(z(18.0))
        .child(state_row("Primary", &primary))
        .child(state_row("Secondary", &secondary))
        .child(third)
        .child(keycaps)
        .child(focus_row)
        .child(plain)
        .child(field)
        .child(recorders)
        .child(warning)
        .child(tabs)
}

// MARK: - Live

/// The sheet in a window of its own, live: the forced rows as the snapshot has them, and under
/// them a row of controls left to the pointer and keyboard, each saying what it last did.
pub struct Workbench {
    anim: std::rc::Rc<std::cell::RefCell<crate::interact::Anim>>,
    on: bool,
    checked: bool,
    choice: usize,
    picked: usize,
    pressed: usize,
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::riso::let_go_of_last_frame(window);
        let scale = window.scale_factor();
        let anim = self.anim.clone();
        let me = cx.entity().downgrade();
        let set = move |f: fn(&mut Workbench)| -> live::Act {
            let me = me.clone();
            live::act(move |_, cx| {
                let _ = me.update(cx, |v, cx| {
                    f(v);
                    cx.notify();
                });
            })
        };
        let (on, checked, choice, picked, pressed) = (self.on, self.checked, self.choice, self.picked, self.pressed);
        let me2 = cx.entity().downgrade();
        let me3 = cx.entity().downgrade();
        crate::interact::build(&anim, false, window, cx, || {
            let live_row = div().flex().flex_row().gap(z(14.0)).items_center()
                .child(controls::button(&Button::primary("Replace").key_chip("↵").id("bench-primary"), scale, set(|v| v.pressed += 1)).0)
                .child(controls::button(&Button::secondary("Open setup").id("bench-secondary"), scale, set(|v| v.pressed += 1)).0)
                .child(controls::button(&Button::ghost("Remove").id("bench-ghost"), scale, set(|v| v.pressed += 1)).0)
                .child(controls::toggle(on, "bench switch", scale, set(|v| v.on = !v.on)))
                .child(controls::checkbox(checked, 1.0, "bench check", scale, Some(set(|v| v.checked = !v.checked))))
                .child(controls::choice(&["Panel", "Message"], choice, "bench choice", scale, move |i| {
                    let me = me2.clone();
                    live::act(move |_, cx| { let _ = me.update(cx, |v, cx| { v.choice = i; cx.notify() }); })
                }))
                .child(controls::menu(vec!["Fix".into(), "Friendly".into(), "Professional".into()], picked, None, 150.0, move |i, cx| {
                    let _ = me3.update(cx, |v, cx| { v.picked = i; cx.notify() });
                }));
            let said = format!("pressed {pressed} · switch {} · check {} · choice {choice} · picked {picked}", if on { "on" } else { "off" }, if checked { "on" } else { "off" });
            div().bg(col(hue::I1)).size_full()
                .child(build(scale))
                .child(div().px(z(24.0)).flex().flex_col().gap(z(10.0))
                    .child(label("Live: for the pointer and keyboard"))
                    .child(live_row)
                    .child(div().id("bench-said").child(t(said.clone(), martian(11.0, 450), hue::I11)).role(Role::Label).aria_label(said)))
        })
    }
}

/// Opens the workbench with its top left at `origin` (screen points).
pub fn open(origin: Point<Pixels>, cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(760.0), px(640.0)) })),
        titlebar: Some(TitlebarOptions { title: Some("Iris controls".into()), appears_transparent: true, traffic_light_position: None }),
        focus: crate::takes_keys(),
        show: true,
        kind: WindowKind::Normal,
        ..Default::default()
    };
    let _ = cx.open_window(options, |window, cx| {
        crate::color_managed(window);
        cx.new(|_| Workbench { anim: Default::default(), on: true, checked: false, choice: 0, picked: 0, pressed: 0 })
    });
}
