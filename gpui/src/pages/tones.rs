//! Settings › Tones (`TonesPage` in SettingsView.swift): the tone table, adding a tone, and
//! the style rules every tone follows.

use gpui::{Div, IntoElement, ParentElement, Styled, div};

use crate::controls::{self as c, Button};
use crate::interact::act;

use crate::parts::*;
use crate::pen;
use crate::riso::{col, hue, ink};
use crate::settings::{Ctx, Values};
use crate::settings_kit::*;

/// `ToneList.panelRoom`: Fix and five tones fit in the panel.
const PANEL_ROOM: usize = 6;

/// A cell of fixed width.
fn cell(w: f32) -> Div {
    div().flex_none().w(z(w))
}

/// A line of caption cut short with an ellipsis where it runs out of room.
fn cut_short(s: &str, w: f32, a: f32) -> Div {
    faded(styled(caption(), hue::I10).w(z(w)).truncate().child(s.to_string()), a)
}

/// The page's sections, `w` points wide.
pub fn content(v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    let s = cx.scale;
    let fields = v.fields.as_ref();

    // 01: the table.
    let cols: [(&str, Option<f32>); 5] = [("Key", Some(28.0)), ("Tone", Some(118.0)), ("How it sounds", None), ("", Some(72.0)), ("Show", Some(34.0))];
    // Every row is six items 12 apart; what's left after the fixed columns is the words'.
    let words_w = w - 24.0 - (28.0 + 118.0 + 72.0 + 34.0) - 5.0 * 12.0;
    let mut table = column().w(z(w)).child(table_header(&cols, w));
    let fix = row().gap(z(12.0)).items_center()
        .child(cell(28.0).child(t("1", caption(), hue::I10)))
        .child(cell(118.0).child(ink_label_faded("Fix", 12.0, 1.0, s)))
        .child(cut_short("Corrects mistakes, keeps your words", words_w, 1.0))
        .child(grow())
        .child(cell(72.0))
        // Fix is always in the panel: its box can't be unticked.
        .child(cell(34.0).child(c::checkbox(true, 0.6, "Fix is always shown", s, None)));
    table = table.child(c::table_row("tone-fix", fix, false, 0, w, 1.0));
    let n = v.tones.len();
    for (i, tone) in v.tones.iter().enumerate() {
        let key_text = tone.key.map(|k| k.to_string()).unwrap_or_else(|| "–".into());
        // A hidden tone's row is faded to half, each thing in it on its own.
        let a = if tone.shown { 1.0 } else { 0.5 };
        let (id, label) = (tone.id.clone(), tone.label.clone());
        let moving = |by: isize| {
            let id = id.clone();
            act(move |_, _| crate::tones::change(|l| l.move_by(&id, by)))
        };
        let mut tools = row().gap(z(2.0))
            .child(faded(c::icon_button("chevron.up", i > 0, &format!("Move {label} up"), moving(-1)), a))
            .child(faded(c::icon_button("chevron.down", i < n - 1, &format!("Move {label} down"), moving(1)), a));
        if tone.custom {
            let gone = id.clone();
            tools = tools.child(faded(c::icon_button("trash", true, &format!("Delete {label}"), act(move |_, _| crate::tones::change(|l| l.remove(&gone)))), a));
        }
        let shown = tone.shown;
        let flip = {
            let id = id.clone();
            act(move |_, _| crate::tones::change(|l| l.set_hidden(&id, shown)))
        };
        let r = row().gap(z(12.0)).items_center()
            .child(cell(28.0).child(faded(t(key_text, caption(), hue::I10), a)))
            .child(cell(118.0).child(ink_label_faded(&label, 12.0, a, s)))
            .child(cut_short(&tone.instruction, words_w, a))
            .child(grow())
            .child(cell(72.0).child(tools))
            .child(cell(34.0).child(c::checkbox(shown, a, &format!("Show {label}"), s, Some(flip))));
        table = table.child(c::table_row(&format!("tone-{id}"), r, i == n - 1, i as i64 + 1, w, a));
    }

    // 02: a new tone. Return in either box does nothing (Swift's fields here have no submit).
    let (name, how) = (fields.map(|f| f.tone_name.clone()), fields.map(|f| f.tone_how.clone()));
    let ready = name.as_ref().is_some_and(|f| !f.trimmed().is_empty()) && how.as_ref().is_some_and(|f| !f.trimmed().is_empty());
    let add = match (&name, &how) {
        (Some(n), Some(h)) => {
            let (n, h) = (n.entity.clone(), h.entity.clone());
            act(move |_, cx| {
                let (label, instruction) = (n.read(cx).trimmed().to_string(), h.read(cx).trimmed().to_string());
                if label.is_empty() || instruction.is_empty() {
                    return;
                }
                crate::tones::change(|l| l.add(&label, &instruction));
                crate::text_field::clear(&n, cx);
                crate::text_field::clear(&h, cx);
            })
        }
        _ => act(|_, _| {}),
    };
    let (add, add_w) = c::button(&Button::primary("Add tone").icon("plus").enabled(ready), s, add);
    let new_tone = column().w(z(w)).gap(z(12.0))
        .child(well(name.as_ref(), "Name", w, act(|_, _| {})))
        .child(well(how.as_ref(), "How should it sound?", w, act(|_, _| {})))
        .child(row().w(z(w)).child(grow()).child(cell(add_w).child(add)));

    // 03: the style rules, a tick each, then the box to add one.
    let mut style = column().w(z(w)).gap(z(12.0));
    for (i, rule) in v.style_rules.iter().enumerate() {
        if i > 0 {
            style = style.child(hairline(60 + i as i64, w));
        }
        let gone = rule.clone();
        let remove = act(move |_, _| crate::app::change(|s| s.style_rules.retain(|r| *r != gone)));
        let (remove, _) = c::button(&Button::ghost("Remove").compact().label(format!("Remove {rule}")).id(format!("remove-rule-{rule}")), s, remove);
        style = style.child(row().w(z(w)).gap(z(10.0)).items_center()
            .child(drawn(13.0, 10.0, pen::hand_tick(13.0, 10.0), Some(1.9), col(ink::PINK_TEXT)))
            .child(t(rule.clone(), body(), hue::I12)).child(grow()).child(remove));
    }
    let rule_field = fields.map(|f| f.rule.clone());
    let add_rule = match &rule_field {
        Some(f) => {
            let e = f.entity.clone();
            act(move |_, cx| {
                let rule = e.read(cx).trimmed().to_string();
                if rule.is_empty() || crate::app::settings().style_rules.contains(&rule) {
                    return;
                }
                crate::app::change(|s| s.style_rules.push(rule.clone()));
                crate::text_field::clear(&e, cx);
            })
        }
        None => act(|_, _| {}),
    };
    let typed = rule_field.as_ref().is_some_and(|f| !f.trimmed().is_empty());
    let (add_btn, add_rule_w) = c::button(&Button::secondary("Add").enabled(typed).id("add-rule"), s, add_rule.clone());
    style = style.child(row().w(z(w)).gap(z(8.0)).items_center().mt(z(if v.style_rules.is_empty() { 0.0 } else { 4.0 }))
        .child(well(rule_field.as_ref(), "e.g. keep my lowercase, never \"kindly\"", w - 8.0 - add_rule_w, add_rule))
        .child(add_btn));

    vec![
        section(Some(1), "Tones", Some(&format!("The panel has room for Fix and {} tones: the first ones checked here, in this order. Their number keys pick them.", PANEL_ROOM - 1)), None, Some(table), w, s),
        section(Some(2), "Add a tone", Some("Say how it should sound, e.g. \"Customer update: polite, says what happens next\"."), None, Some(new_tone), w, s),
        section(Some(3), "Your style", Some("Say it the way you'd tell a friend. Every tone follows these; Fix only corrects mistakes."), None, Some(style), w, s),
    ]
}

/// A text field live, or the empty well a snapshot draws.
fn well(field: Option<&crate::text_field::Field>, placeholder: &str, w: f32, submit: crate::interact::Act) -> gpui::AnyElement {
    match field {
        Some(f) => crate::text_field::view(f, placeholder, w, submit),
        None => well_field(placeholder, w).into_any_element(),
    }
}
