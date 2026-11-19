//! Settings › Words (`WordsPage` in SettingsView.swift): the words Iris never changes, and what
//! it always protects on its own. Live: type a word, Return or Add keeps it (once), Remove
//! lets it go; each is saved at once (`glossary`) and read by the next check.

use gpui::{Div, IntoElement, ParentElement, Styled};

use crate::controls::{self as c, Button};
use crate::interact::act;
use crate::parts::*;
use crate::riso::hue;
use crate::settings::{Ctx, Values};
use crate::settings_kit::*;

/// The page's sections, `w` points wide.
pub fn content(v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    let s = cx.scale;
    // The box and its button; then the words, or the empty state. Each is padded 2 from the
    // rule (`content.padding(.top, 2)` pads each view the section holds).
    let field = v.fields.as_ref().map(|f| f.word.clone());
    let typed = field.as_ref().map(|f| f.trimmed()).unwrap_or_default();
    let add = field.as_ref().map(|f| {
        let e = f.entity.clone();
        act(move |_, cx| {
            let word = e.read(cx).trimmed().to_string();
            if word.is_empty() || crate::app::settings().glossary.contains(&word) {
                return;
            }
            crate::app::change(|s| s.glossary.push(word.clone()));
            crate::text_field::clear(&e, cx);
        })
    });
    let (add_btn, add_w) = c::button(&Button::primary("Add").enabled(!typed.is_empty()), s, add.clone().unwrap_or_else(|| act(|_, _| {})));
    let well = match (&field, add) {
        (Some(f), Some(a)) => crate::text_field::view(f, "Add a word or phrase", w - 8.0 - add_w, a),
        _ => well_field("Add a word or phrase", w - 8.0 - add_w).into_any_element(),
    };
    let field = row().w(z(w)).gap(z(8.0)).items_center().child(well).child(add_btn);
    let list = if v.glossary.is_empty() {
        empty_state(&["No words yet.", "Add the names Iris should never touch."], w)
    } else {
        let mut list = column().w(z(w));
        let n = v.glossary.len();
        for (i, word) in v.glossary.iter().enumerate() {
            let gone = word.clone();
            let remove = act(move |_, _| crate::app::change(|s| s.glossary.retain(|x| *x != gone)));
            let (remove, _) = c::button(&Button::ghost("Remove").compact().label(format!("Remove {word}")).id(format!("remove-word-{word}")), s, remove);
            let r = row().gap(z(12.0)).items_center().child(t(word.clone(), body(), hue::I12)).child(grow()).child(remove);
            list = list.child(c::table_row(&format!("word-{word}"), r, i == n - 1, i as i64, w, 1.0));
        }
        list
    };
    let keep = column().w(z(w)).gap(z(12.0)).child(field).child(column().pt(z(2.0)).child(list));

    vec![
        section(Some(1), "Never change", Some("Iris keeps these exactly as written: product names, SKUs, people's names."), None, Some(keep), w, s),
        section(Some(2), "Always protected", Some("No need to add these: @mentions, #channels, :emoji:, links, email addresses and `code` are never changed."), None, None, w, s),
    ]
}
