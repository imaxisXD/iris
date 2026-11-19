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
    /// `DS.Motion.pill`.
    pub const PILL: Curve = Curve::Spring(0.4, 0.12);

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

/// The window's moving parts: what's under the pointer, what's held down, each tracked value,
/// and where the controls that pop a menu were last drawn.
#[derive(Default)]
pub struct Anim {
    hovered: HashSet<String>,
    pressed: HashSet<String>,
    tracks: HashMap<String, Track>,
    pub bounds: HashMap<String, Bounds<Pixels>>,
    /// A marker's flight: the choice it left, the one it's going to, since when.
    flights: HashMap<String, (usize, usize, Instant)>,
    /// Controls whose menu is open.
    open: HashSet<String>,
    /// The note demo's clock (`NotesSection`).
    pub demo: DemoClock,
    /// Something is still moving: draw another frame.
    pub moving: bool,
}

impl Anim {
    /// Forgets every tracked value (a page turned), so the next page starts at rest.
    pub fn reset(&mut self) {
        self.tracks.clear();
        self.hovered.clear();
        self.pressed.clear();
        self.bounds.clear();
        self.flights.clear();
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

    /// A flight between choices: (from, to, how far along, 0 → 1 with the spring's overshoot).
    pub fn flight(&self, id: &str, target: usize, curve: Curve) -> (usize, usize, f32) {
        let curve = match curve {
            Curve::Spring(..) if self.reduce_motion => Curve::FAST,
            c => c,
        };
        let mut a = self.anim.borrow_mut();
        let f = a.flights.entry(id.to_string()).or_insert((target, target, self.now));
        if f.1 != target {
            *f = (f.1, target, self.now);
        }
        let (from, to, since) = *f;
        let t = self.now.duration_since(since).as_secs_f32();
        if from != to && t < curve.length() {
            a.moving = true;
            (from, to, curve.at(t))
        } else {
            (to, to, 1.0)
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
    use super::{ease_out, spring};

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
    let live = Live { anim: anim.clone(), view: Some(view), now: Instant::now(), reduce_motion, mouse: Some(window.mouse_position()) };
    anim.borrow_mut().moving = false;
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
