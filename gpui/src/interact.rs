//! What makes a window's pictures into controls, for every window (Settings, setup, the confirm
//! card…): a window's render sets a dispatcher (`Live`) while it builds, and the controls in
//! `controls` ask it how far each is hovered and pressed. A state can also be forced
//! (`forcing`), as Swift's `\.forcedInteraction` does, so a sheet can draw every control at
//! rest, hovered, pressed and focused side by side (`components` scene) through the very code
//! the live window runs.
//!
//! Motion follows the Swift app's numbers (`DS.Motion`): hover, press and colour on
//! `DS.easeOut` (cubic-bezier 0.23, 1, 0.32, 1 over 0.15 s), the switch knob on
//! `spring(duration: 0.45, bounce: 0.2)`, the ink choice's marker on `DS.Motion.pill`
//! (`spring(duration: 0.4, bounce: 0.12)`). Each animated thing is tracked by an id: when the
//! value it's drawn from changes, it moves from where it was drawn to the new value. A thing seen
//! for the first time (a page just turned to) starts at rest, so nothing replays.
//!
//! Event handlers run after the page is built, so they capture what they need (`Live` is cheap
//! to clone); they never read the dispatcher.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use gpui::*;

use crate::parts::{at, sized, with_zoom, z, zoom};

/// What a control does when it's used.
pub type Act = Rc<dyn Fn(&mut Window, &mut App)>;

/// An action from a closure.
pub fn act(f: impl Fn(&mut Window, &mut App) + 'static) -> Act {
    Rc::new(f)
}

/// An action that changes the settings (saved at once, as Swift's `AppSettings` saves).
pub fn change(f: impl Fn(&mut crate::store::Settings) + 'static) -> Act {
    Rc::new(move |_, _| crate::app::change(&f))
}

/// How a tracked value moves to a new one.
#[derive(Clone, Copy, Debug)]
pub enum Curve {
    /// `DS.Motion.fast`'s curve over this many seconds.
    Ease(f32),
    /// SwiftUI's `.spring(duration:bounce:)`.
    Spring(f32, f32),
}

impl Curve {
    /// `DS.easeOut`: hover, press, colour.
    pub const FAST: Curve = Curve::Ease(0.15);
    /// `DS.Motion.knob`.
    pub const KNOB: Curve = Curve::Spring(0.45, 0.2);

    /// Where it is `t` seconds in, 0 → 1 (a spring overshoots a touch).
    fn at(self, t: f32) -> f32 {
        let t = t / crate::parts::slow();
        match self {
            Curve::Ease(d) => ease_out((t / d).clamp(0.0, 1.0)),
            Curve::Spring(d, b) => spring(t, d, b),
        }
    }
    /// How long it takes to settle.
    fn length(self) -> f32 {
        crate::parts::slow() * match self {
            Curve::Ease(d) => d,
            Curve::Spring(d, _) => d * 1.75,
        }
    }
}

/// `DS.Motion`'s curve (`parts::ds_curve`).
pub fn ease_out(x: f32) -> f32 {
    crate::parts::ds_curve(x)
}

/// SwiftUI's `.spring(duration: d, bounce: b)` (`parts::swift_spring`).
pub fn spring(t: f32, d: f32, b: f32) -> f32 {
    crate::parts::swift_spring(t, d, b)
}

#[derive(Clone, Copy)]
struct Track {
    from: f32,
    to: f32,
    since: Instant,
    curve: Curve,
}

/// An analytical damped spring (`DS.Motion.pill` / SwiftUI `.spring(duration:bounce:)`), carrying
/// live position and velocity so mid-flight retargets redirect smoothly instead of restarting.
#[derive(Clone, Copy)]
struct SpringTrack {
    x0: f32,
    v0: f32,
    target: f32,
    since: Instant,
    duration: f32,
    bounce: f32,
}

impl SpringTrack {
    /// Evaluates `(position, velocity)` at `t` scaled seconds into the spring.
    fn eval(&self, t: f32) -> (f32, f32) {
        let e0 = self.x0 - self.target;
        if t <= 0.0 || (e0.abs() <= 1e-6 && self.v0.abs() <= 1e-6) {
            return (self.x0, self.v0);
        }
        let omega = std::f32::consts::TAU / self.duration.max(1e-3);
        let zeta = (1.0 - self.bounce).clamp(0.05, 1.0);
        if zeta >= 0.9999 {
            // Exact critically damped closed form: zero overshoot and C¹ velocity continuity.
            let decay = (-omega * t).exp();
            let c2 = self.v0 + omega * e0;
            let e = decay * (e0 + c2 * t);
            let v = decay * (self.v0 - omega * c2 * t);
            return (self.target + e, v);
        }
        let wd = omega * (1.0 - zeta * zeta).sqrt();
        let a = e0;
        let b = (self.v0 + zeta * omega * e0) / wd;
        let decay = (-zeta * omega * t).exp();
        let (sin, cos) = (wd * t).sin_cos();
        let e = decay * (a * cos + b * sin);
        let v = decay * (self.v0 * cos - ((zeta * omega * self.v0 + omega * omega * e0) / wd) * sin);
        (self.target + e, v)
    }

    fn settled(&self, t: f32, x: f32, v: f32) -> bool {
        let e0 = self.x0 - self.target;
        if e0.abs() <= 1e-6 && self.v0.abs() <= 1e-6 {
            return true;
        }
        if t >= self.duration * 2.2 {
            return true;
        }
        let omega = std::f32::consts::TAU / self.duration.max(1e-3);
        let scale = e0.abs().max(self.v0.abs() / omega).max(1e-3);
        t > 0.0 && (x - self.target).abs() <= 1e-3 * scale && v.abs() <= 1e-3 * scale * omega
    }
}

/// State for a simultaneous in-place panel body cross-fade (`popLayout`).
struct BodySwap {
    key: String,
    current: crate::model::Panel,
    outgoing: Option<crate::model::Panel>,
    since: Instant,
}

/// The window's moving parts: what's under the pointer, what's held down, each tracked value,
/// and where the controls that pop a menu were last drawn.
#[derive(Default)]
pub struct Anim {
    hovered: HashSet<String>,
    pressed: HashSet<String>,
    tracks: HashMap<String, Track>,
    springs: HashMap<String, SpringTrack>,
    pub bounds: HashMap<String, Bounds<Pixels>>,
    /// A marker's flight: the choice it left, the one it's going to, since when.
    flights: HashMap<String, (usize, usize, Instant)>,
    /// Simultaneous in-place panel body cross-fade (`panel.rs`).
    body_swap: Option<BodySwap>,
    /// Measured panel chrome height (`paper_h - body_h`) at full scale, so `opening::scene`
    /// places the paper from `chrome_h + animated_body_h` with zero frame lag.
    pub panel_chrome_h: Option<f32>,
    /// Controls whose menu is open.
    open: HashSet<String>,
    /// The note demo's clock (`NotesSection`).
    pub demo: DemoClock,
    /// Frame counter and start instant for contiguous breathing pulses (`WaitingText`), so each
    /// wait begins at full opacity (`1.0`) rather than a random global phase.
    frame_id: u64,
    pulse_frame: u64,
    pulse_since: Option<Instant>,
    /// Something is still moving: draw another frame.
    pub moving: bool,
}

impl Anim {
    /// Forgets every tracked value (a page turned), so the next page starts at rest.
    pub fn reset(&mut self) {
        self.tracks.clear();
        self.springs.clear();
        self.hovered.clear();
        self.pressed.clear();
        self.bounds.clear();
        self.flights.clear();
        self.body_swap = None;
        self.panel_chrome_h = None;
        self.pulse_since = None;
    }

    fn track(&mut self, id: &str, target: f32, curve: Curve, now: Instant) -> f32 {
        let tr = self.tracks.entry(id.to_string()).or_insert(Track { from: target, to: target, since: now, curve });
        let shown = |tr: &Track| {
            let t = now.duration_since(tr.since).as_secs_f32();
            tr.from + (tr.to - tr.from) * tr.curve.at(t)
        };
        if (tr.to - target).abs() > 1e-6 {
            let at_now = shown(tr);
            *tr = Track { from: at_now, to: target, since: now, curve };
        }
        let t = now.duration_since(tr.since).as_secs_f32();
        if t < tr.curve.length() && (tr.to - tr.from).abs() > 1e-6 {
            self.moving = true;
            shown(tr)
        } else {
            tr.to
        }
    }

    fn spring(&mut self, id: &str, target: f32, duration: f32, bounce: f32, now: Instant) -> (f32, bool) {
        let slow = crate::parts::slow().max(1e-3);
        let tr = self.springs.entry(id.to_string()).or_insert(SpringTrack {
            x0: target,
            v0: 0.0,
            target,
            since: now,
            duration,
            bounce,
        });
        if (tr.target - target).abs() > 1e-6 {
            let elapsed = now.duration_since(tr.since).as_secs_f32() / slow;
            let (cur_x, cur_v) = tr.eval(elapsed);
            *tr = SpringTrack { x0: cur_x, v0: cur_v, target, since: now, duration, bounce };
        }
        let elapsed = now.duration_since(tr.since).as_secs_f32() / slow;
        let (x, v) = tr.eval(elapsed);
        if tr.settled(elapsed, x, v) {
            tr.x0 = tr.target;
            tr.v0 = 0.0;
            (tr.target, false)
        } else {
            self.moving = true;
            (x, true)
        }
    }
}

/// `NotesSection`'s clock: with nothing pointed at, the stage plays every message in turn;
/// pointing at a row holds that row's clip; letting go carries on the run from the one held.
pub struct DemoClock {
    pub start: Instant,
    pub first: usize,
    pub held: Option<usize>,
}

impl Default for DemoClock {
    fn default() -> Self {
        DemoClock { start: Instant::now(), first: 0, held: None }
    }
}

impl DemoClock {
    /// The pointer came onto row `row` (`Some`) or left it (`None`).
    pub fn hold(&mut self, note: Option<usize>, row: usize) {
        let note = match note {
            Some(n) => Some(n),
            None if self.held == Some(row) => None,
            None => return,
        };
        if note == self.held {
            return;
        }
        if note.is_none() && let Some(h) = self.held {
            self.first = h;
        }
        self.held = note;
        self.start = Instant::now();
    }

    /// The clip on the stage and how far into it, `length` seconds a clip, `count` clips.
    pub fn playing(&self, now: Instant, length: f64, count: usize, still: Option<&dyn Fn(usize) -> f64>) -> (usize, f64) {
        if let Some(landed) = still {
            let n = self.held.unwrap_or(self.first);
            return (n, landed(n));
        }
        let elapsed = now.duration_since(self.start).as_secs_f64() / crate::parts::slow() as f64;
        let local = elapsed % length;
        match self.held {
            Some(h) => (h, local),
            None => ((self.first + (elapsed / length) as usize) % count, local),
        }
    }
}

/// Hover, press and keyboard focus for one control (`Interaction`): hover and press run 0 → 1 as
/// they fade in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Interaction {
    pub hover: f32,
    pub pressed: f32,
    pub focused: bool,
}

impl Interaction {
    pub const REST: Interaction = Interaction { hover: 0.0, pressed: 0.0, focused: false };
    pub const HOVER: Interaction = Interaction { hover: 1.0, pressed: 0.0, focused: false };
    /// Pressed is under the pointer too, as Swift's sheet draws it.
    pub const PRESSED: Interaction = Interaction { hover: 1.0, pressed: 1.0, focused: false };
    pub const FOCUSED: Interaction = Interaction { hover: 0.0, pressed: 0.0, focused: true };
}

thread_local! {
    static FORCED: std::cell::Cell<Option<Interaction>> = const { std::cell::Cell::new(None) };
}

/// Builds with every control drawn in `state` (`\.forcedInteraction`).
pub fn forcing<R>(state: Interaction, f: impl FnOnce() -> R) -> R {
    let before = FORCED.replace(Some(state));
    let r = f();
    FORCED.set(before);
    r
}

/// The state being forced, if one is.
pub fn forced() -> Option<Interaction> {
    FORCED.get()
}

/// The dispatcher: set while a live window builds.
#[derive(Clone)]
pub struct Live {
    pub anim: Rc<RefCell<Anim>>,
    /// The view to draw again when something moves (none for a sheet drawn once).
    pub view: Option<AnyWeakEntity>,
    pub now: Instant,
    pub reduce_motion: bool,
    /// Where the pointer is in the window, as this frame is built (none: not known).
    pub mouse: Option<Point<Pixels>>,
}

thread_local! {
    static LIVE: RefCell<Option<Live>> = const { RefCell::new(None) };
}

/// The dispatcher, while the live window builds its page; none in a snapshot.
pub fn get() -> Option<Live> {
    LIVE.with(|l| l.borrow().clone())
}

/// Builds with `live` set, and clears it after (even if building panics).
pub fn with<R>(live: Live, f: impl FnOnce() -> R) -> R {
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            LIVE.with(|l| *l.borrow_mut() = None);
        }
    }
    LIVE.with(|l| *l.borrow_mut() = Some(live));
    let _clear = Clear;
    f()
}

impl Live {
    /// A dispatcher for a picture drawn once (the component sheet): nothing moves.
    pub fn still() -> Live {
        Live { anim: Rc::new(RefCell::new(Anim::default())), view: None, now: Instant::now(), reduce_motion: false, mouse: None }
    }

    /// The state `id` draws in: forced, else as the pointer and keyboard have it. `presses`:
    /// whether it can be pressed at all (a disabled control never is).
    pub fn state(&self, id: &str, presses: bool) -> Interaction {
        if let Some(f) = forced() {
            return Interaction { pressed: if presses { f.pressed } else { 0.0 }, ..f };
        }
        Interaction { hover: self.hover(id), pressed: if presses { self.press(id) } else { 0.0 }, focused: false }
    }

    /// Asks the window to draw again.
    pub fn notify(&self, cx: &mut App) {
        if let Some(e) = self.view.as_ref().and_then(|v| v.upgrade()) {
            cx.notify(e.entity_id());
        }
    }

    /// A value drawn from `target`, moving there on `curve` when it changes. Under Reduce
    /// Motion springs are quick fades instead (`DS.Motion.fast`), as Swift drops their travel.
    pub fn track(&self, id: &str, target: f32, curve: Curve) -> f32 {
        let curve = match curve {
            Curve::Spring(..) if self.reduce_motion => Curve::FAST,
            c => c,
        };
        self.anim.borrow_mut().track(id, target, curve, self.now)
    }

    /// A value driven by an analytical damped spring (`DS.Motion.pill`), preserving live velocity
    /// when retargeted mid-flight. Under Reduce Motion it lands immediately with no travel.
    /// Returns `(current_value, still_moving)`.
    pub fn spring(&self, id: &str, target: f32, duration: f32, bounce: f32) -> (f32, bool) {
        if self.reduce_motion {
            return (target, false);
        }
        self.anim.borrow_mut().spring(id, target, duration, bounce, self.now)
    }

    /// Scaled seconds elapsed since a contiguous breathing wait (`WaitingText`) began, resetting
    /// cleanly to `0.0` (full opacity) whenever a wait starts anew.
    pub fn pulse_elapsed(&self) -> f32 {
        let mut a = self.anim.borrow_mut();
        let frame = a.frame_id;
        let fresh = a.pulse_since.is_none() || (a.pulse_frame != frame && a.pulse_frame + 1 != frame);
        if fresh {
            a.pulse_since = Some(self.now);
        }
        a.pulse_frame = frame;
        let since = a.pulse_since.unwrap_or(self.now);
        self.now.duration_since(since).as_secs_f32() / crate::parts::slow().max(1e-3)
    }

    /// A flight between choices: `(from, to, elapsed_scaled_s)`. While `from != to` and
    /// `elapsed_scaled_s < duration`, marks the window as moving so it draws another frame.
    /// Once settled (`elapsed_scaled_s >= duration`), returns `(to, to, duration)`.
    pub fn flight_elapsed(&self, id: &str, target: usize, duration: f32) -> (usize, usize, f32) {
        let mut a = self.anim.borrow_mut();
        let f = a.flights.entry(id.to_string()).or_insert((target, target, self.now));
        if f.1 != target {
            *f = (f.1, target, self.now);
        }
        let (from, to, since) = *f;
        let slow = crate::parts::slow().max(1e-3);
        let elapsed = self.now.duration_since(since).as_secs_f32() / slow;
        if from != to && elapsed < duration {
            a.moving = true;
            (from, to, elapsed)
        } else {
            (to, to, duration)
        }
    }

    /// Tracks the panel's displayed body keyed by `key`. When `key` changes, keeps the previous
    /// panel as an outgoing layer for `duration` scaled seconds so incoming and outgoing content
    /// cross-fade simultaneously (`popLayout`), marking the window as moving while active.
    pub fn body_swap(&self, panel: &crate::model::Panel, key: String, duration: f32) -> Option<(crate::model::Panel, f32)> {
        let mut a = self.anim.borrow_mut();
        let slow = crate::parts::slow().max(1e-3);
        match &mut a.body_swap {
            None => {
                a.body_swap = Some(BodySwap { key, current: panel.clone(), outgoing: None, since: self.now });
                None
            }
            Some(s) => {
                if s.key != key {
                    let prev = std::mem::replace(&mut s.current, panel.clone());
                    s.key = key;
                    s.outgoing = Some(prev);
                    s.since = self.now;
                } else {
                    s.current = panel.clone();
                }
                let elapsed = self.now.duration_since(s.since).as_secs_f32() / slow;
                if elapsed < duration {
                    if let Some(out) = s.outgoing.clone() {
                        a.moving = true;
                        return Some((out, elapsed));
                    }
                } else {
                    s.outgoing = None;
                }
                None
            }
        }
    }

    /// Whether `id`'s menu is open.
    pub fn is_open(&self, id: &str) -> bool {
        self.anim.borrow().open.contains(id)
    }

    pub fn set_open(&self, id: &str, on: bool) {
        let mut a = self.anim.borrow_mut();
        if on { a.open.insert(id.to_string()); } else { a.open.remove(id); }
    }

    /// The pointer is on `id`.
    pub fn is_hovered(&self, id: &str) -> bool {
        if let Some(f) = forced() {
            return f.hover > 0.5;
        }
        let mut a = self.anim.borrow_mut();
        if !a.hovered.contains(id) {
            return false;
        }
        // The pointer left without a word (GPUI can miss a leave as a page turns or the window
        // goes behind another): it's where the pointer is that counts, as SwiftUI's onHover.
        if let (Some(m), Some(b)) = (self.mouse, a.bounds.get(id))
            && !b.contains(&m)
        {
            a.hovered.remove(id);
            return false;
        }
        true
    }

    /// How far `id`'s hover look has come in (0 → 1, fading on `DS.easeOut` both ways).
    pub fn hover(&self, id: &str) -> f32 {
        if let Some(f) = forced() {
            return f.hover;
        }
        let on = self.is_hovered(id);
        self.track(&format!("hover:{id}"), on as u8 as f32, Curve::FAST)
    }

    /// How far `id` is pressed in (0 → 1).
    pub fn press(&self, id: &str) -> f32 {
        if let Some(f) = forced() {
            return f.pressed;
        }
        let on = self.anim.borrow().pressed.contains(id);
        self.track(&format!("press:{id}"), on as u8 as f32, Curve::FAST)
    }

    /// A handler that keeps `id`'s hover, then calls `also`.
    pub fn hover_handler(&self, id: &str, also: Option<Rc<dyn Fn(bool, &mut App)>>) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
        let (live, id) = (self.clone(), id.to_string());
        move |on: &bool, _, cx| {
            // Under /verify the pointer is never a person's: an accessibility press synthesizes a
            // click there, and Swift's controls take no hover from those presses.
            if crate::driven::on() {
                return;
            }
            {
                let mut a = live.anim.borrow_mut();
                if *on { a.hovered.insert(id.clone()); } else { a.hovered.remove(&id); }
            }
            if let Some(f) = &also {
                f(*on, cx);
            }
            live.notify(cx);
        }
    }

    pub fn set_pressed(&self, id: &str, on: bool, cx: &mut App) {
        let changed = {
            let mut a = self.anim.borrow_mut();
            if on { a.pressed.insert(id.to_string()) } else { a.pressed.remove(id) }
        };
        if changed {
            self.notify(cx);
        }
    }

    /// Where `id` was last drawn, in the window.
    pub fn bounds(&self, id: &str) -> Option<Bounds<Pixels>> {
        self.anim.borrow().bounds.get(id).copied()
    }
}

/// Records where its parent is drawn under `id` (for a menu to pop from it). Fills the parent.
pub fn bounds_marker(live: &Live, id: &str) -> AnyElement {
    let (anim, id) = (live.anim.clone(), id.to_string());
    canvas(move |b, _, _| {
        anim.borrow_mut().bounds.insert(id.clone(), b);
    }, |_, _, _, _| {})
    .absolute().top_0().left_0().size_full().into_any_element()
}

/// Makes `el` (already `w`×`h`) a button live: hover and press tracked under `id`, squeezed to
/// `squeeze` of its size while held (`SqueezeStyle`; none under Reduce Motion), `act` on click.
/// `build` draws the face given how far hover has come in; under a squeeze it's laid out smaller,
/// centred in its box, because GPUI can't scale a box.
pub fn pressable(live: &Live, id: &str, w: f32, h: f32, squeeze: f32, act: Option<Act>, build: impl FnOnce(f32) -> AnyElement) -> Stateful<Div> {
    pressable_hearing(live, id, w, h, squeeze, act, None, build)
}

/// `pressable`, also telling `also` when the pointer comes onto it or leaves (GPUI takes one
/// hover handler an element).
#[allow(clippy::too_many_arguments)]
pub fn pressable_hearing(live: &Live, id: &str, w: f32, h: f32, squeeze: f32, act: Option<Act>, also: Option<Rc<dyn Fn(bool, &mut App)>>, build: impl FnOnce(f32) -> AnyElement) -> Stateful<Div> {
    let hover = live.hover(id);
    let press = if act.is_some() { live.press(id) } else { 0.0 };
    let k = if live.reduce_motion { 1.0 } else { 1.0 - (1.0 - squeeze) * press };
    let face = if (1.0 - k).abs() > 1e-4 {
        let outer = zoom();
        let inner = with_zoom(outer * k, || build(hover));
        at(inner, (w - w * k) / 2.0, (h - h * k) / 2.0).into_any_element()
    } else {
        at(build(hover), 0.0, 0.0).into_any_element()
    };
    let mut el = sized(w, h).child(face).child(bounds_marker(live, id)).id(SharedString::from(format!("live-{id}")));
    // Keyboard focus: a pen ring 4 points outside (`focusRing`).
    if forced().is_some_and(|f| f.focused) {
        el = el.child(crate::controls::focus_ring(w, h));
    }
    // A sheet drawn once has nothing to hear.
    if live.view.is_none() {
        return el;
    }
    el = el.on_hover(live.hover_handler(id, also));
    if let Some(a) = act {
        let (l1, l2, l3, i1, i2, i3) = (live.clone(), live.clone(), live.clone(), id.to_string(), id.to_string(), id.to_string());
        el = el
            .on_mouse_down(MouseButton::Left, move |_, _, cx| l1.set_pressed(&i1, true, cx))
            .on_mouse_up(MouseButton::Left, move |_, _, cx| l2.set_pressed(&i2, false, cx))
            .on_mouse_up_out(MouseButton::Left, {
                let l = live.clone();
                let i = id.to_string();
                move |_, _, cx| l.set_pressed(&i, false, cx)
            })
            .on_click(move |_, window, cx| {
                l3.set_pressed(&i3, false, cx);
                a(window, cx);
                l3.notify(cx);
            });
    }
    el
}

/// `z` for a whole box, for callers that lay out by hand.
pub fn zbox(w: f32, h: f32) -> Size<Pixels> {
    size(z(w), z(h))
}

/// Two colours mixed, `k` of the way from `a` to `b`.
pub fn mix(a: crate::riso::Rgb8, b: crate::riso::Rgb8, k: f32) -> crate::riso::Rgb8 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k.clamp(0.0, 1.0)).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// A colour at `alpha` (washes fade by their ink's alpha).
pub fn alpha(c: Rgba, a: f32) -> Rgba {
    Rgba { a: c.a * a.clamp(0.0, 1.0), ..c }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's own `test` attribute would shadow the standard one.
    use std::time::{Duration, Instant};
    use super::{Anim, ease_out, spring};

    #[test]
    fn curves_run_from_zero_to_one() {
        assert!(ease_out(0.0).abs() < 1e-6 && (ease_out(1.0) - 1.0).abs() < 1e-6);
        // The fast tier is mostly there early on.
        assert!(ease_out(0.3) > 0.8);
        assert!(spring(0.0, 0.45, 0.2).abs() < 1e-4);
        assert!((spring(0.45 * 1.75, 0.45, 0.2) - 1.0).abs() < 0.01);
        // A bounce overshoots.
        assert!((0..100).any(|i| spring(i as f32 * 0.01, 0.45, 0.2) > 1.01));
    }

    #[test]
    fn spring_track_matches_swift_spring_and_retargets_smoothly() {
        let t0 = Instant::now();
        let mut anim = Anim::default();
        // First sight starts at rest.
        assert_eq!(anim.spring("x", 0.0, 0.4, 0.12, t0), (0.0, false));
        // Retarget to 100.0; at t = 0.1 s it must match `swift_spring(0.1, 0.4, 0.12) * 100.0`.
        let (x0, moving0) = anim.spring("x", 100.0, 0.4, 0.12, t0);
        assert!(moving0 && x0.abs() < 1e-4);
        let t1 = t0 + Duration::from_millis(100);
        let (x1, moving1) = anim.spring("x", 100.0, 0.4, 0.12, t1);
        assert!(moving1);
        let expected = spring(0.1, 0.4, 0.12) * 100.0;
        assert!((x1 - expected).abs() < 1e-3, "got {x1}, expected {expected}");
        // Mid-flight retarget to 220.0 preserves position (no teleport) and continues forward smoothly.
        let (x_retarget, moving_r) = anim.spring("x", 220.0, 0.4, 0.12, t1);
        assert!(moving_r && (x_retarget - x1).abs() < 1e-3);
        let t2 = t1 + Duration::from_millis(16);
        let (x2, _) = anim.spring("x", 220.0, 0.4, 0.12, t2);
        assert!(x2 > x_retarget, "velocity should carry marker forward: {x_retarget} -> {x2}");
        // Eventually settles cleanly on target.
        let t_end = t1 + Duration::from_millis(1000);
        assert_eq!(anim.spring("x", 220.0, 0.4, 0.12, t_end), (220.0, false));
    }

    #[test]
    fn critically_damped_spring_never_overshoots_and_retargets_smoothly() {
        let t0 = Instant::now();
        let mut anim = Anim::default();
        assert_eq!(anim.spring("h", 61.0, 0.24, 0.0, t0), (61.0, false));
        // Shrink from 61 → 17 pt: must be strictly monotonic decreasing with zero overshoot below 17.
        let (h0, m0) = anim.spring("h", 17.0, 0.24, 0.0, t0);
        assert!(m0 && (h0 - 61.0).abs() < 1e-4);
        let mut prev = h0;
        for ms in (8..=360).step_by(8) {
            let (h, _) = anim.spring("h", 17.0, 0.24, 0.0, t0 + Duration::from_millis(ms));
            assert!(h <= prev + 1e-4, "must decrease monotonically: {prev} -> {h} at {ms}ms");
            assert!(h >= 17.0 - 1e-4, "critically damped spring overshot target at {ms}ms: {h}");
            prev = h;
        }
        // Settles cleanly on target.
        assert_eq!(anim.spring("h", 17.0, 0.24, 0.0, t0 + Duration::from_millis(600)), (17.0, false));

        // Mid-flight reversal preserves position with zero teleport.
        let t_rev = t0 + Duration::from_millis(700);
        let _ = anim.spring("h", 61.0, 0.24, 0.0, t_rev);
        let t_mid = t_rev + Duration::from_millis(60);
        let (h_mid, m_mid) = anim.spring("h", 61.0, 0.24, 0.0, t_mid);
        assert!(m_mid && h_mid > 25.0 && h_mid < 55.0);
        let (h_after_rev, m_rev) = anim.spring("h", 17.0, 0.24, 0.0, t_mid);
        assert!(m_rev && (h_after_rev - h_mid).abs() < 1e-3);
    }

    #[test]
    fn flight_elapsed_and_body_swap_track_changes_cleanly() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let t0 = Instant::now();
        let anim = Rc::new(RefCell::new(Anim::default()));
        let make = |now| super::Live { anim: anim.clone(), view: None, now, reduce_motion: false, mouse: None };

        // First sight starts at rest.
        assert_eq!(make(t0).flight_elapsed("tabs", 1, 0.21), (1, 1, 0.21));
        // Switching 1 → 3 reports direction and elapsed time, marking moving = true.
        assert_eq!(make(t0).flight_elapsed("tabs", 3, 0.21), (1, 3, 0.0));
        assert!(anim.borrow().moving);
        let (f, to, el) = make(t0 + Duration::from_millis(100)).flight_elapsed("tabs", 3, 0.21);
        assert_eq!((f, to), (1, 3));
        assert!((el - 0.1).abs() < 1e-3);
        assert_eq!(make(t0 + Duration::from_millis(250)).flight_elapsed("tabs", 3, 0.21), (3, 3, 0.21));

        // BodySwap: first sight is at rest; same key (e.g. streamed tokens) never restarts swap;
        // new key exposes previous panel for duration, then clears it.
        let p0 = crate::opening::panel_state();
        let mut p1 = p0.clone();
        p1.selected = 1;
        assert!(make(t0).body_swap(&p0, "0:done:a".into(), 0.16).is_none());
        assert!(make(t0 + Duration::from_millis(20)).body_swap(&p0, "0:done:a".into(), 0.16).is_none());
        let (out, el0) = make(t0 + Duration::from_millis(40)).body_swap(&p1, "1:gen".into(), 0.16).expect("swap starts");
        assert_eq!(out.selected, 0);
        assert!(el0 < 1e-4);
        // Updating partial text under the same key continues the swap without resetting elapsed time.
        let mut p1_stream = p1.clone();
        p1_stream.phase = crate::model::Phase::Generating("Hi".into());
        let (_, el1) = make(t0 + Duration::from_millis(100)).body_swap(&p1_stream, "1:gen".into(), 0.16).expect("still swapping");
        assert!((el1 - 0.06).abs() < 1e-3);
        // Settles after 160 ms.
        assert!(make(t0 + Duration::from_millis(220)).body_swap(&p1_stream, "1:gen".into(), 0.16).is_none());
    }
}

thread_local! {
    /// Every live window's moving parts, so a test can point at a control (`poke`).
    static WINDOWS: RefCell<Vec<(std::rc::Weak<RefCell<Anim>>, AnyWeakEntity)>> = const { RefCell::new(Vec::new()) };
}

/// Builds a live window's content with its dispatcher set, and asks for the next frame while
/// anything on it still moves. Any window with controls builds through this.
pub fn build<V: 'static, R>(anim: &Rc<RefCell<Anim>>, reduce_motion: bool, window: &mut Window, cx: &mut Context<V>, f: impl FnOnce() -> R) -> R {
    let view: AnyWeakEntity = cx.entity().downgrade().into();
    WINDOWS.with(|w| {
        let mut w = w.borrow_mut();
        w.retain(|(a, _)| a.strong_count() > 0);
        if !w.iter().any(|(a, _)| a.upgrade().is_some_and(|a| Rc::ptr_eq(&a, anim))) {
            w.push((Rc::downgrade(anim), view.clone()));
        }
    });
    {
        let mut a = anim.borrow_mut();
        a.frame_id = a.frame_id.wrapping_add(1);
        a.moving = false;
    }
    let live = Live { anim: anim.clone(), view: Some(view), now: Instant::now(), reduce_motion, mouse: Some(window.mouse_position()) };
    let out = with(live, f);
    if anim.borrow().moving {
        window.request_animation_frame();
    }
    out
}

/// For a test, which can't move the pointer: as if the pointer came onto (or left) the control
/// `id`, or pressed (or let go of) it, in every live window. True if a window has a control
/// by that name.
pub fn poke(what: &str, id: &str, on: bool, cx: &mut App) -> bool {
    let windows: Vec<_> = WINDOWS.with(|w| w.borrow().iter().filter_map(|(a, v)| Some((a.upgrade()?, v.clone()))).collect());
    let mut found = false;
    for (anim, view) in windows {
        {
            let mut a = anim.borrow_mut();
            found |= a.tracks.keys().any(|k| k.ends_with(&format!(":{id}")) || k == id);
            let set = if what == "press" { &mut a.pressed } else { &mut a.hovered };
            if on { set.insert(id.to_string()); } else { set.remove(id); }
        }
        if let Some(e) = view.upgrade() {
            cx.notify(e.entity_id());
        }
    }
    found
}

/// The controls drawn in live windows, by the names `poke` takes.
pub fn names() -> Vec<String> {
    let windows: Vec<_> = WINDOWS.with(|w| w.borrow().iter().filter_map(|(a, _)| a.upgrade()).collect());
    let mut out: Vec<String> = windows.iter().flat_map(|a| a.borrow().tracks.keys().filter_map(|k| k.strip_prefix("hover:").map(String::from)).collect::<Vec<_>>()).collect();
    out.sort();
    out.dedup();
    out
}
