//! The ⌃G moment (`RigFrames` in the Swift app): the iris grows beside the text box, opens,
//! and the panel unrolls out of its bloom; on Replace the flower folds and sinks. Drawn for
//! one moment `t` over a stand-in chat, 900×460, placed as `PanelLayout` places it.
//!
//! The flower is `rig.rs`, the Rune copy's port of `IrisRig`, drawn on the CPU (tiny-skia) as
//! one picture a frame. GPUI can't scale a box once it's laid out, so while the panel unrolls it
//! is laid out smaller (`parts::with_zoom`), evenly, and cut to a strip that opens downwards, as
//! the Rune copy does. GPUI can't turn a box either, so the sliver doesn't lean the 4 degrees
//! Swift's does.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{Div, ParentElement, Styled, canvas, div, px};

use crate::model::{Answer, Panel, Phase, Verdict};
use crate::panel::{self, MARGIN};
use crate::parts::{at, printed, with_zoom};
use crate::rig::{self, RigClock, curve};
use crate::riso::{Img, col};

pub const SCENE: (f32, f32) = (900.0, 460.0);
const BOX: (f32, f32, f32, f32) = (200.0, 330.0, 560.0, 58.0);
/// Where the text box's corner is in the scene: the live panel puts it at the pointer.
pub const BOX_AT: (f32, f32) = (BOX.0, BOX.1);
const SAMPLE: &str = "hey, i have checked the PR and its look fine but one thing need to change before merge";
const FIXED: &str = "Hey, I have checked the PR and it looks fine, but one thing needs to change before merge";

/// The flower beside the panel (`BudView`): 108 points, and how far it reaches.
pub const SIZE: f32 = 108.0;
const K: f32 = SIZE / 92.0;
const REACH_LEFT: f32 = 37.0 * K;
const REACH_RIGHT: f32 = 42.0 * K;
const HEADROOM: f32 = 70.0 * K + 2.0;

/// `PanelMotion`: folded, the paper is a sliver at the flower's head, turned a little.
const FOLDED: (f32, f32, f32) = (0.22, 0.04, -4.0);

#[derive(Clone, Copy)]
pub struct Script {
    /// The full arrival (the drop and its splash) or the quick one (`arrival_is_full`).
    pub full: bool,
    /// When the answer came, so the flower opened (`f64::INFINITY`: not yet).
    pub bloom: f64,
    /// When there was something else to read with the flower still shut: a failure, or the
    /// model waking up (`f64::INFINITY`: not yet). The panel unrolls for it without a bloom.
    pub readable: f64,
    /// When the panel went: Replace pressed (`replaced`), or put away (Escape, a click away).
    pub replace: f64,
    pub replaced: bool,
    /// The stand-in chat behind it, as the design frames draw; live, the desktop shows through.
    pub backdrop: bool,
}

pub const FULL: Script = Script { full: true, bloom: 0.9, readable: f64::INFINITY, replace: 3.0, replaced: true, backdrop: true };
/// Put away (Escape) instead of replaced: the panel folds back into the flower.
pub const AWAY: Script = Script { full: true, bloom: 0.9, readable: f64::INFINITY, replace: 3.0, replaced: false, backdrop: true };
pub const QUICK: Script = Script { full: false, bloom: 0.5, readable: f64::INFINITY, replace: 2.2, replaced: true, backdrop: true };

/// The panel the moment opens on: the stand-in draft, fixed.
pub fn panel_state() -> Panel {
    Panel {
        original: SAMPLE.into(),
        selected: 0,
        phase: Phase::Done(Answer::fix(SAMPLE, FIXED, Some(Verdict::Minor), false)),
        app_name: Some("Slack".into()),
        app_bundle: Some("com.tinyspeck.slackmacgap".into()),
        can_replace: true,
        copied: false,
        retry_note: None,
    }
}

/// What each tone makes of the stand-in draft (made-up answers, as `MockAnswers` gives).
pub fn answer(tone: usize) -> Answer {
    match tone {
        0 => Answer::fix(SAMPLE, FIXED, Some(Verdict::Minor), false),
        1 => Answer::tone(SAMPLE, "Hi, I've reviewed the PR. It looks good overall, but one change is needed before we merge.", false),
        2 => Answer::tone(SAMPLE, "Hey! I checked the PR and it looks good. Just one small thing to change before we merge it.", false),
        3 => Answer::tone(SAMPLE, "PR looks good. One change needed before merge.", false),
        _ => Answer::tone(SAMPLE, "I checked the PR and it looks fine. One thing needs to change before we merge.", false),
    }
}

/// The flower's clock at `t` seconds into `script`.
pub fn rig_clock(t: f64, script: &Script) -> RigClock {
    RigClock {
        now: 1000.0 + t,
        arrived: (t >= 0.0).then_some(1000.0),
        bloomed: (t >= script.bloom).then_some(1000.0 + script.bloom),
        wilted: (t >= script.replace).then_some(1000.0 + script.replace),
        full: script.full,
        replaced: script.replaced,
    }
}

/// Whether the flower is still moving at `t`: after its bloom has settled (2.1 s on, when its
/// line stops boiling and its idle sway has faded) it holds still until the panel goes.
pub fn rig_moving(t: f64, script: &Script) -> bool {
    t < script.bloom + 2.15 || t >= script.replace
}

/// Where the rig's picture goes in the scene, in points: (left, top, width, height).
pub fn rig_box(paper_h: f32) -> (f32, f32, f32, f32) {
    let foot = place(paper_h).2;
    (foot.0 - 90.0 * K, foot.1 - 166.0 * K, 180.0 * K, 200.0 * K)
}

/// The rig drawn for `t`, `scale` pixels a point, as straight RGBA.
pub fn rig_rgba(t: f64, script: &Script, scale: f32) -> (u32, u32, Vec<u8>) {
    let (pw, ph) = ((180.0 * K * scale).ceil() as u32, (200.0 * K * scale).ceil() as u32);
    (pw, ph, rig::draw_rig_at(&rig_clock(t, script), SIZE, scale, pw, ph, 0.0, 0.0))
}

/// `PanelLayout.place` in the scene: the panel's left `x`, the paper's top, the flower's foot
/// and the hinge the panel unrolls from. Above the box, the flower stands on the box's top edge
/// beside the paper's foot; below it (`set_below`), it stands beside the paper, its bloom level
/// with the paper's top.
fn place(paper_h: f32) -> (f32, f32, (f32, f32), (f32, f32)) {
    let room = REACH_RIGHT + 10.0;
    let lowest = 8.0 + REACH_LEFT + room;
    let highest = SCENE.0 - 8.0 - panel::WIDTH;
    let x = 8.0f32.max(BOX.0.max(lowest).min(highest));
    let (paper_top, foot) = if BELOW.get() {
        (BELOW_TOP, (x - room, BELOW_TOP + FLOWER_H))
    } else {
        (BOX.1 - 10.0 - paper_h, (x - room, BOX.1 - 10.0 + 10.0 - 2.0))
    };
    let hinge = (x, foot.1 - HEADROOM);
    (x, paper_top, foot, hinge)
}

thread_local! {
    /// The panel opens below its box (`PanelLayout.Side.below`).
    static BELOW: Cell<bool> = const { Cell::new(false) };
}

/// Below the box: where the paper's top is in the scene (room above for the bloom).
const BELOW_TOP: f32 = 90.0;
/// From the flower's foot to the top of its bloom (`BudView.reach.height`).
const FLOWER_H: f32 = 100.0 * K;
/// `PanelLayout.gap`, `edge`, `flowerGap`.
const GAP: f64 = 10.0;
const EDGE: f64 = 8.0;

/// Where the panel opens, chosen as `PanelLayout.place` chooses, in screen points from the top
/// left: the window's origin, and whether the paper goes below the box.
/// - `field`: the text box (x, y, w, h), or none (by the pointer);
/// - `prefer_below` / `near_pointer`: the setting;
/// - `pointer`: where the pointer is; `visible`: the screen's room (x, y, w, h);
/// - `paper_h`: how tall the paper will likely be.
pub fn screen_place(field: Option<[f64; 4]>, prefer_below: bool, near_pointer: bool, pointer: (f64, f64), visible: [f64; 4], paper_h: f64) -> ((f64, f64), bool) {
    let [vx, vy, vw, vh] = visible;
    // A box taller than 40% of the screen is a whole page, one wider than it nonsense: by the
    // pointer instead (`canStand`).
    let field = field.filter(|[x, y, w, h]| *h <= vh * 0.4 && *w <= vw && x + w > vx && *x < vx + vw && y + h > vy && *y < vy + vh);
    let (anchor, prefer_below) = match (field, near_pointer) {
        (Some(f), false) => (f, prefer_below),
        // By the pointer: the flower grows where the pointer is, the paper above it.
        _ => ([pointer.0 + REACH_RIGHT as f64 + GAP, pointer.1, 0.0, 0.0], false),
    };
    let [ax, ay, _, ah] = anchor;
    let fits_above = ay - GAP - paper_h >= vy + EDGE;
    let fits_below = ay + ah + GAP + paper_h <= vy + vh - EDGE;
    let below = if prefer_below {
        fits_below || (!fits_above && vy + vh - (ay + ah) > ay - vy)
    } else {
        !fits_above && (fits_below || vy + vh - (ay + ah) > ay - vy)
    };
    // In line with the box, never so far left the flower leaves the screen, nor so far right
    // the paper does.
    let room = (REACH_RIGHT + 10.0) as f64;
    let lowest = vx + EDGE + REACH_LEFT as f64 + room;
    let highest = vx + vw - EDGE - panel::WIDTH as f64;
    let paper_x = (vx + EDGE).max(ax.max(lowest).min(highest));
    let scene_x = {
        BELOW.set(below);
        place(paper_h as f32).0 as f64
    };
    let x = paper_x - scene_x;
    let y = if below { ay + ah + GAP - BELOW_TOP as f64 } else { ay - BOX.1 as f64 };
    ((x, y), below)
}

/// Opens the next panel below its box (or above).
pub fn set_below(below: bool) {
    BELOW.set(below);
}

/// How long the flower stands alone before the panel may open (`PanelMotion.budMoment`): its
/// whole arrival when it plays in full, a moment when it's the quick one.
pub fn bud_moment(full: bool) -> f64 {
    if full { 0.6 } else { 0.1 }
}

/// After the flower opens, the paper follows this much later (`PanelMotion.afterBloom`).
pub fn after_bloom(full: bool) -> f64 {
    if full { 0.26 } else { 0.06 }
}

/// A slow answer still unfolds the panel (and streams into it) this long after the press
/// (`PanelMotion.unfoldAnyway`).
pub const UNFOLD_ANYWAY: f64 = 1.1;

/// When the panel starts to unroll (`PanelController.unfold` with `PanelMotion.unrollDelay`): as
/// soon as there's something to read, never before the flower has had its moment, and a beat
/// after it blooms, so you see bud, bloom, panel; a slow answer at `UNFOLD_ANYWAY`. Each thing to
/// read before the panel has opened puts the opening off or brings it closer, as Swift's
/// `unfold()` cancels the one it had planned; one that comes after it has opened changes nothing,
/// so the answer can be worked out at any moment from what has happened so far.
pub fn unfold_at(script: &Script) -> f64 {
    let mut at = UNFOLD_ANYWAY;
    let mut events = [(script.readable, false), (script.bloom, true)];
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (when, blooms) in events {
        if when >= at {
            break;
        }
        at = when.max(bud_moment(script.full)) + if blooms { after_bloom(script.full) } else { 0.0 };
    }
    at
}

/// Whether the flower plays its full arrival (the drop and its splash) or the quick one
/// (`BudArrival.isFull`): the first five times ever, the first time each day, and now and then
/// after that (`roll` is 0 about one time in `ARRIVAL_NOW_AND_THEN`), so it stays a treat
/// instead of a wait. `arrivals`: how many there have been before this one.
pub fn arrival_is_full(arrivals: u32, first_today: bool, roll: u32) -> bool {
    arrivals < ARRIVAL_FIRST_FULL || first_today || roll == 0
}

/// How many arrivals are always full (`BudArrival.firstFull`).
pub const ARRIVAL_FIRST_FULL: u32 = 5;
/// After those, about one in this many is full anyway (`BudArrival.nowAndThen`).
pub const ARRIVAL_NOW_AND_THEN: u32 = 15;

/// How far the panel has unrolled at `t` (0 folded … 1 open, overshooting a little), and how
/// solid it is. Replace closes it at once; put away, it folds back down into the flower.
pub fn unroll(t: f64, script: &Script) -> (f64, f64) {
    let unroll_at = unfold_at(script);
    if t < script.replace {
        (curve::spring(t - unroll_at, 0.34, 0.14), curve::ease_out((t - unroll_at) / 0.07))
    } else if script.replaced {
        (-1.0, 0.0)
    } else {
        let k = (t - script.replace).max(0.0);
        let open = curve::spring(script.replace - unroll_at, 0.34, 0.14);
        (open * (1.0 - curve::ease_in(k / 0.18)), 1.0 - curve::ease_out((k - 0.08) / 0.12))
    }
}

/// The scene at `t` seconds into `script`, the panel showing `shown`. `paper_h` is the paper's
/// height at full size (from the last layout); `measured` gets this layout's. `rig` is the
/// flower's picture for this moment (drawn elsewhere, so a live window can draw it off the main
/// thread).
#[allow(clippy::too_many_arguments)]
pub fn scene(t: f64, script: &Script, shown: &Panel, look: &panel::Look, paper_h: f32, measured: Rc<Cell<f32>>, rig: Option<&Img>) -> Div {
    let (x, paper_top, _foot, hinge) = place(paper_h);
    let (u, fade) = unroll(t, script);

    let mut s = div().relative().w(px(SCENE.0)).h(px(SCENE.1)).overflow_hidden();
    if script.backdrop {
        let sans = |size: f32, bold: bool| {
            let d = div().font_family(".SystemUIFont").text_size(px(size)).line_height(px(size * 1.2)).text_color(col((0, 0, 0)));
            if bold { d.font_weight(gpui::FontWeight::BOLD) } else { d }
        };
        let chat = div().flex().flex_row().gap(px(10.0)).items_start()
            .child(div().flex_none().w(px(30.0)).h(px(30.0)).rounded(px(6.0)).bg(col((201, 184, 217))))
            .child(div().flex().flex_col().gap(px(2.0)).child(sans(13.0, true).child("lead")).child(sans(13.0, false).child("Did you get to the PR?")));
        let after = match &shown.phase { Phase::Done(a) => a.text.as_str(), _ => FIXED };
        let words = if t >= script.replace + 0.05 && script.replaced { after } else { SAMPLE };
        let field = div().relative().w(px(BOX.2)).h(px(BOX.3)).rounded(px(8.0)).bg(col((255, 255, 255)))
            .border(px(1.0)).border_color(gpui::Rgba { a: 0.5, ..col((128, 128, 128)) })
            .child(at(sans(13.0, false).w(px(BOX.2 - 24.0)).child(words.to_string()), 12.0, 10.0));
        s = s.bg(col((246, 245, 247))).child(at(chat, BOX.0 + 40.0, BOX.1 - 62.0)).child(at(field, BOX.0, BOX.1));
    }

    if u > 0.0 {
        // Unrolling sideways out of the bloom: the panel laid out `sx` times its size, pinned at
        // the hinge, and cut to a strip that opens downwards.
        let u = u as f32;
        let sx = FOLDED.0 + (1.0 - FOLDED.0) * u;
        let sy = FOLDED.1 + (1.0 - FOLDED.1) * u;
        let origin = (x - MARGIN.0, paper_top - MARGIN.1);
        let full_w = panel::WIDTH + 2.0 * MARGIN.0;
        let full_h = paper_h + MARGIN.1 + MARGIN.2;
        let anchor = ((hinge.0 - origin.0) / full_w, (hinge.1 - origin.1) / full_h);
        let shown_h = (full_h * sy / sx).min(full_h);
        let m = measured.clone();
        let p = with_zoom(sx, || {
            let k = crate::parts::zoom();
            let fit = canvas(|_, _, _| (), move |b, _, window, _| {
                // The paper's height at full size; when it changes (another tone's answer), place
                // the panel again.
                let h = f32::from(b.size.height) / k;
                if (m.get() - h).abs() > 0.25 {
                    m.set(h);
                    window.refresh();
                }
            }).absolute().top_0().left_0().size_full();
            div().pl(crate::parts::z(MARGIN.0)).pt(crate::parts::z(MARGIN.1)).child(panel::panel(shown, look).child(fit))
        });
        // Scaled about the anchor: the panel's corner moves toward it.
        let corner = (anchor.0 * full_w * (1.0 - sx), anchor.1 * full_h * (1.0 - sx));
        let strip = div().relative().w(px(full_w)).h(px(full_h)).child(at(p, corner.0, corner.1)).opacity(fade.clamp(0.0, 1.0) as f32);
        let cut = div().relative().w(px(full_w)).h(px(shown_h)).overflow_hidden()
            .child(at(strip, 0.0, -(full_h - shown_h) * anchor.1));
        s = s.child(at(cut, origin.0, origin.1 + (full_h - shown_h) * anchor.1));
    } else {
        // Folded away: nothing on the paper, but its height is still known.
        let _ = &measured;
    }

    if let Some(img) = rig {
        // One picture pixel a device pixel (the picture is whole pixels, a touch over the canvas).
        let (left, top, w, h) = rig_box(paper_h);
        let (w, h) = ((w * look.scale).ceil() / look.scale, (h * look.scale).ceil() / look.scale);
        s = s.child(at(printed(w, h, img), left, top));
    }
    s
}

/// The folded rotation Swift gives the sliver, which GPUI can't (kept to say what's missing).
pub const FOLDED_DEGREES: f32 = FOLDED.2;

#[cfg(test)]
mod tests {
    use super::{BOX, screen_place, set_below};

    const SCREEN: [f64; 4] = [0.0, 25.0, 1728.0, 1092.0];

    #[test]
    fn above_by_default_below_when_asked() {
        let field = Some([400.0, 600.0, 500.0, 60.0]);
        let ((x, y), below) = screen_place(field, false, false, (0.0, 0.0), SCREEN, 200.0);
        assert!(!below);
        // The box's corner where the scene's is.
        assert_eq!((x, y), (400.0 - BOX.0 as f64, 600.0 - BOX.1 as f64));
        let (_, below) = screen_place(field, true, false, (0.0, 0.0), SCREEN, 200.0);
        assert!(below);
        set_below(false);
    }

    #[test]
    fn flips_when_there_is_no_room() {
        // A box at the top of the screen: no room above, so below.
        let (_, below) = screen_place(Some([400.0, 60.0, 500.0, 40.0]), false, false, (0.0, 0.0), SCREEN, 200.0);
        assert!(below);
        // A box at the bottom: no room below, so above even when below is asked.
        let (_, below) = screen_place(Some([400.0, 1050.0, 500.0, 40.0]), true, false, (0.0, 0.0), SCREEN, 200.0);
        assert!(!below);
        set_below(false);
    }

    #[test]
    fn stays_on_screen_sideways() {
        // A box at the right edge: the paper moves left so it fits.
        let ((x, _), _) = screen_place(Some([1600.0, 600.0, 100.0, 30.0]), false, false, (0.0, 0.0), SCREEN, 200.0);
        let paper_x = x + BOX.0 as f64;
        assert!(paper_x + super::panel::WIDTH as f64 <= 1728.0 - 8.0 + 0.01);
        // A whole page isn't a box: by the pointer.
        let ((x, _), _) = screen_place(Some([0.0, 25.0, 1728.0, 1000.0]), false, false, (900.0, 500.0), SCREEN, 200.0);
        assert!(x + BOX.0 as f64 > 900.0);
        set_below(false);
    }

    /// The moment so far, as the live panel builds it: arrival, answer, something else to read.
    fn so_far(full: bool, bloom: Option<f64>, readable: Option<f64>) -> super::Script {
        super::Script { full, bloom: bloom.unwrap_or(f64::INFINITY), readable: readable.unwrap_or(f64::INFINITY),
                        replace: f64::INFINITY, replaced: false, backdrop: false }
    }

    #[test]
    fn the_panel_unrolls_when_there_is_something_to_read() {
        use super::{UNFOLD_ANYWAY, after_bloom, bud_moment, unfold_at};
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        // The quick arrival with an answer in 0.35 s: a beat after the bloom, not a second later.
        assert!(close(unfold_at(&so_far(false, Some(0.35), None)), 0.35 + after_bloom(false)));
        // An answer at once still lets the flower have its moment: the quick one, and the full one.
        assert!(close(unfold_at(&so_far(false, Some(0.0), None)), bud_moment(false) + after_bloom(false)));
        assert!(close(unfold_at(&so_far(true, Some(0.0), None)), 0.6 + 0.26));
        // The design frames (bloom at 0.9) unroll where they always have.
        assert!(close(unfold_at(&super::FULL), 1.16) && close(unfold_at(&super::QUICK), 0.56));
        // The model waking up: the panel opens after the flower's moment to say so, no bloom.
        assert!(close(unfold_at(&so_far(false, None, Some(0.0))), bud_moment(false)));
        assert!(close(unfold_at(&so_far(true, Some(3.0), Some(0.0))), bud_moment(true)));
        // A failure, the flower shut.
        assert!(close(unfold_at(&so_far(false, None, Some(0.4))), 0.4));
        // Nothing yet: a slow answer unfolds anyway, and one that comes after changes nothing.
        assert!(close(unfold_at(&so_far(false, None, None)), UNFOLD_ANYWAY));
        assert!(close(unfold_at(&so_far(true, Some(1.5), None)), UNFOLD_ANYWAY));
        // An answer just before then puts it off to a beat after the bloom (Swift's unfold()
        // cancels the slow-answer one it had planned).
        assert!(close(unfold_at(&so_far(true, Some(1.0), None)), 1.0 + after_bloom(true)));
    }

    #[test]
    fn the_full_arrival_stays_a_treat() {
        use super::arrival_is_full;
        // The first five ever are full, whatever the day or the roll.
        assert!((0..5).all(|n| arrival_is_full(n, false, 7)));
        // After that: the first each day, or the roll's one in fifteen.
        assert!(!arrival_is_full(5, false, 7));
        assert!(arrival_is_full(101, true, 7));
        assert!(arrival_is_full(101, false, 0));
    }
}
