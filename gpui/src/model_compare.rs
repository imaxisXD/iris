//! What each model is good at, measured (`ModelCompare.swift`): blind-judged answers on drafts
//! no lesson was written from (the share a person would send), answer time and memory. Shown as
//! printed dots under each model in setup and Settings › Model & memory.

use gpui::{Div, FontWeight, HighlightStyle, InteractiveElement, ParentElement, Role, StatefulInteractiveElement, Styled, StyledText, div};

use crate::parts::*;
use crate::pen;
use crate::riso::{self, Plate, col, hue, ink, over};

/// `ModelStrengths`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strengths {
    /// Share of English answers good enough to send, out of 100.
    pub english: u32,
    /// The same for Hindi and Hinglish drafts.
    pub hindi: u32,
    /// Seconds for a typical answer.
    pub seconds: f64,
    /// Memory it holds while loaded, in GB.
    pub memory_gb: f64,
}

impl Strengths {
    /// By the model's id (`ModelChoice`'s raw value).
    pub fn of(id: &str) -> Option<Strengths> {
        match id {
            "best" => Some(Strengths { english: 91, hindi: 85, seconds: 0.3, memory_gb: 3.7 }),
            "lighter" => Some(Strengths { english: 87, hindi: 59, seconds: 0.2, memory_gb: 2.9 }),
            // Judged in the same rounds as Iris's own; memory as measured with Iris before.
            "e4b" => Some(Strengths { english: 71, hindi: 81, seconds: 0.45, memory_gb: 5.2 }),
            _ => None,
        }
    }
}

/// Dots out of five, to the half: a share out of 100, or how quick an answer is.
pub fn dots_share(share: u32) -> f32 {
    (share as f32 / 20.0 * 2.0).round() / 2.0
}
pub fn dots_seconds(s: f64) -> f32 {
    if s <= 0.2 { 5.0 } else if s <= 0.3 { 4.0 } else if s <= 0.5 { 3.0 } else { 2.0 }
}

/// This Mac's memory, in GB (`ModelChoice.macMemoryGB`).
pub fn mac_memory_gb() -> u32 {
    #[cfg(target_os = "macos")]
    {
        let mut size: u64 = 0;
        let mut len = std::mem::size_of::<u64>();
        let name = c"hw.memsize";
        unsafe { libc_sysctlbyname(name.as_ptr(), &mut size as *mut u64 as *mut _, &mut len, std::ptr::null_mut(), 0) };
        (size as f64 / 1_073_741_824.0).round() as u32
    }
    #[cfg(not(target_os = "macos"))]
    16
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    #[link_name = "sysctlbyname"]
    fn libc_sysctlbyname(name: *const std::ffi::c_char, oldp: *mut std::ffi::c_void, oldlenp: *mut usize, newp: *mut std::ffi::c_void, newlen: usize) -> i32;
}

/// `ModelChoice.recommended(memoryGB:)`: best quality with 16 GB or more (`roomyMemoryGB`).
pub fn recommended(memory_gb: u32) -> &'static str {
    if memory_gb >= 16 { "best" } else { "lighter" }
}

/// Why the recommended model suits this Mac (`recommendationLine`).
pub fn recommendation_line(memory_gb: u32) -> String {
    if recommended(memory_gb) == "best" {
        format!("This Mac has {memory_gb} GB of memory, room for best quality.")
    } else {
        format!("This Mac has {memory_gb} GB of memory. Lighter leaves room for your other apps.")
    }
}

/// One dot (`Pip`): a pen circle, filled with the pink plate to `fill` (0, half or whole).
fn pip(fill: f32, seed: i64, scale: f32) -> Div {
    let s = 12.0;
    let mut e = sized(s, s);
    if fill > 0.0 {
        let plate = Plate { w: s, h: s, scale };
        let img = riso::cached(format!("pip-{seed}-{fill}"), &plate, || {
            let circle = tiny_skia::PathBuilder::from_circle(s / 2.0, s / 2.0, s / 2.0).unwrap();
            let mut mask = plate.fill(&circle);
            // Masked to the filled share, from the left.
            let (pw, ph) = plate.px();
            let keep = (s * fill * scale).round() as u32;
            let data = mask.data_mut();
            for y in 0..ph {
                for x in keep..pw {
                    data[(y * pw + x) as usize] = 0;
                }
            }
            riso::print(&plate, ink::PINK, seed, &mask, 1.0)
        });
        e = e.child(at(printed(s, s, &img), 0.0, 0.0));
    }
    // `Circle().strokeBorder(…, lineWidth: 1.2)`: the stroke inside the circle.
    e.child(at(drawn(s, s, pen::hand_ellipse(seed, 0.0, s - 1.2, s - 1.2).translated(0.6, 0.6), Some(1.2), over(hue::I12, 0.75, ink::PAPER)), 0.0, 0.0))
}

/// `ModelPips`: English, Hindi and speed as dots out of five, then the memory as a figure.
pub fn pips(st: &Strengths, seed: i64, scale: f32) -> gpui::Stateful<Div> {
    let row = |label: &str, score: f32, seed: i64| {
        let mut dots = div().flex().flex_row().gap(z(4.0));
        for k in 0..5i64 {
            let fill = (score - k as f32).clamp(0.0, 1.0);
            let dy = ((k + seed) % 3) as f32 * 0.4 - 0.4;
            dots = dots.child(div().relative().left(z((k % 2) as f32 * 0.4)).top(z(dy)).child(pip(fill, seed * 7 + k, scale)));
        }
        div().flex().flex_row().gap(z(6.0)).items_center()
            .child(div().w(z(64.0)).child(t(label, caption(), hue::I10)))
            .child(dots)
    };
    let memory = div().flex().flex_row().gap(z(6.0)).items_center()
        .child(div().w(z(64.0)).child(t("Memory", caption(), hue::I10)))
        .child(t(format!("{:.1} GB", st.memory_gb), caption(), hue::I12));
    let spoken = format!("English: {} of 100 answers good enough to send. Hindi: {} of 100. An answer in about {:.1} seconds. Takes {:.1} GB of memory.",
                         st.english, st.hindi, st.seconds, st.memory_gb);
    div().flex().flex_col().gap(z(5.0))
        .child(row("English", dots_share(st.english), seed))
        .child(row("Hindi", dots_share(st.hindi), seed + 5))
        .child(row("Speed", dots_seconds(st.seconds), seed + 9))
        .child(memory)
        .id(gpui::SharedString::from(format!("model-pips-{seed}"))).role(Role::Label).aria_label(spoken)
}

/// `ModelPipsNote`: why one is recommended (in the darker ink), and what the dots measure,
/// wrapped in `w` points as one caption paragraph.
pub fn pips_note(w: f32) -> Div {
    let first = recommendation_line(mac_memory_gb());
    let text = format!("{first} Dots: how often an answer was good enough to send, in blind tests.");
    let split = first.len();
    let quiet = HighlightStyle { color: Some(col(hue::I10).into()), font_weight: Some(FontWeight::NORMAL), ..Default::default() };
    styled(caption(), hue::I11).whitespace_normal().w(z(w))
        .child(StyledText::new(text.clone()).with_highlights(vec![(split..text.len(), quiet)]))
}
