//! What the shortcuts do (`AppDelegate.openPanel`, `quickFix`, `Paste`): read the text box
//! you're in, then show the panel by it, or fix it in place; and say what happened by your text.

use std::cell::Cell;
use std::time::Instant;

use gpui::*;

use crate::brain::{self, Event, Mode};
use crate::live::{self, OpeningView, news};
use crate::model::Verdict;
use crate::notes_live;
use crate::textbridge::{self, CaptureError, Captured};
use crate::toast::Message;

thread_local! {
    /// One read at a time; one Quick Fix at a time.
    static CAPTURING: Cell<bool> = const { Cell::new(false) };
    static QUICK_FIXING: Cell<bool> = const { Cell::new(false) };
}

/// The open shortcut: the panel for the text box you're in (`target`, under test: the app a
/// test named). Pressed again while the panel is open, it puts the panel away.
pub fn open_panel(target: Option<i32>, cx: &mut App) {
    crate::live::pressed();
    if let Some(h) = live::open_panel()
        && h.update(cx, |v, window, cx| v.key("escape", window, cx)).unwrap_or(false)
    {
        live::set_open_panel(None);
        return;
    }
    // Not set up yet: setup at its current step, never a panel that downloads.
    if !crate::live_setup::can_check(cx) {
        return crate::live_setup::show(cx);
    }
    prewarm(target, false, cx);
    capture(target, cx, |captured, cx| show(captured, None, cx));
}

/// The Quick Fix shortcut: Fix, pasted in place when that's certainly what you meant
/// (`QuickFixLanding`); the panel when a person should look; a message otherwise.
pub fn quick_fix(target: Option<i32>, cx: &mut App) {
    crate::live::pressed();
    if !crate::live_setup::can_check(cx) {
        return crate::live_setup::show(cx);
    }
    prewarm(target, true, cx);
    if QUICK_FIXING.with(|q| q.replace(true)) {
        return;
    }
    // A read for the panel is in flight: `capture` would refuse this one and Quick Fix would
    // stay taken until Iris restarts.
    if CAPTURING.with(|c| c.get()) {
        QUICK_FIXING.with(|q| q.set(false));
        return;
    }
    let pressed = Instant::now();
    let panel_before = live::open_panel().map(|h| h.window_id());
    capture(target, cx, move |captured, cx| {
        notes_live::show(Message::checking(), Some("checking"), captured.field_frame, cx);
        let c = captured.clone();
        brain::check(&captured.text, &Mode::fix(), false, move |event, cx| match event {
            Event::Partial(_) => {}
            Event::Finished(answer, changed) => {
                QUICK_FIXING.with(|q| q.set(false));
                let c = c.clone();
                let unchanged = answer.verdict == Some(Verdict::Unchanged);
                let heavy = answer.verdict == Some(Verdict::Heavy);
                if unchanged {
                    news("quick fix landing clean");
                    notes_live::show(Message::looks_good(), Some("clean"), c.field_frame, cx);
                } else if heavy || !c.can_replace || answer.translated {
                    news("quick fix landing panel");
                    notes_live::dismiss(cx);
                    let frame = c.field_frame;
                    show_with(c, frame, move |captured, cx| OpeningView::with_answer(captured, answer, cx), cx);
                } else if c.element_known() {
                    // Check `still_holds` off the main thread so Accessibility / UIA IPCs never
                    // block the UI thread (or self-deadlock Windows AccessKit if Iris has focus).
                    // Still Quick Fixing until it lands, so another press can't cut into the paste.
                    QUICK_FIXING.with(|q| q.set(true));
                    let text = answer.text.clone();
                    let keep = answer.keep.clone();
                    cx.spawn(async move |cx| {
                        let c_check = c.clone();
                        let still_there = cx.background_executor().spawn(async move { textbridge::still_holds(&c_check) }).await;
                        let _ = cx.update(|cx| {
                            QUICK_FIXING.with(|q| q.set(false));
                            if panel_took_over(panel_before) {
                                news("quick fix landing left alone (panel)");
                            } else if !still_there {
                                news("quick fix landing left alone");
                                notes_live::show(Message::left_alone(), None, c.field_frame, cx);
                            } else {
                                news("quick fix landing paste");
                                replace_and_say(c, text, keep, Said::Fixed(changed), cx);
                            }
                        });
                    })
                    .detach();
                } else if pressed.elapsed().as_secs_f64() >= 4.0 {
                    news("quick fix landing left alone");
                    notes_live::show(Message::left_alone(), None, c.field_frame, cx);
                } else if panel_took_over(panel_before) {
                    news("quick fix landing left alone (panel)");
                } else {
                    news("quick fix landing paste");
                    replace_and_say(c, answer.text.clone(), answer.keep.clone(), Said::Fixed(changed), cx);
                }
            }
            Event::Failed(problem) => {
                QUICK_FIXING.with(|q| q.set(false));
                let c = c.clone();
                // Another try can work when the check itself failed: the panel, with Try again.
                if problem.check_failed && problem.can_retry {
                    notes_live::dismiss(cx);
                    show(c, None, cx);
                } else {
                    notes_live::show(Message::problem(&problem.lead, &problem.detail), None, c.field_frame, cx);
                }
            }
        });
    });
}

/// What a paste says once it lands.
pub enum Said {
    Replaced,
    Fixed(usize),
}

/// Pastes `text` over what `captured` read and says so by the text ("replaced", "fixed 3
/// words", with ⌘Z unless that's turned off), or "copied" when it couldn't land.
pub fn replace_and_say(captured: Captured, text: String, keep: Vec<(String, String)>, said: Said, cx: &mut App) {
    let frame = captured.field_frame;
    let watched = captured.clone();
    crate::paste::replace_then(captured, text, cx, move |landed, cx| {
        let undo = crate::app::settings().shows("undoKeys");
        if landed {
            // Watch for a word put back (`LearnWords`).
            crate::learn_words::watch(watched, keep, cx);
            let m = match said {
                Said::Replaced => Message::replaced(undo),
                Said::Fixed(n) => Message::fixed(n, undo),
            };
            notes_live::show(m, Some("undo"), frame, cx);
        } else {
            notes_live::show(Message::copied(), None, None, cx);
        }
    });
}

/// The model woken (never downloaded) and the tone the panel starts on made ready while the
/// box is read (`AppDelegate.prewarm`), so the answer comes sooner; not for an app Iris is
/// turned off in, where nothing will be checked.
fn prewarm(target: Option<i32>, quick: bool, cx: &mut App) {
    let pid = target.or_else(|| if textbridge::under_test() { None } else { textbridge::focused_app() });
    #[cfg(target_os = "macos")]
    let bundle = pid.and_then(|p| textbridge::app_info(p).1);
    #[cfg(not(target_os = "macos"))]
    let bundle: Option<String> = { let _ = pid; None };
    if crate::app::settings().is_excluded(bundle.as_deref()) {
        return;
    }
    let mode = if quick {
        Mode::fix()
    } else {
        let list = crate::tones::ToneList::from_settings(&crate::app::settings());
        let start_id = list.starting_for(bundle.as_deref());
        live::current_modes(cx).into_iter().find(|m| m.id == start_id).unwrap_or_else(Mode::fix)
    };
    brain::prewarm(&mode);
}

/// The open shortcut was pressed while Quick Fix was checking: a panel opened since the press
/// (`before`: the one open then, if any), or one is reading the box, has it now, and Quick Fix
/// doesn't paste under it.
fn panel_took_over(before: Option<gpui::WindowId>) -> bool {
    let now = live::open_panel().map(|h| h.window_id());
    (now.is_some() && now != before) || CAPTURING.with(|c| c.get())
}

/// Reads the text box off the main thread (Accessibility calls block on the other app), then
/// hands what it read to `then` on the main thread; a problem is said by the text instead.
fn capture(target: Option<i32>, cx: &mut App, then: impl FnOnce(Captured, &mut App) + 'static) {
    if CAPTURING.with(|c| c.replace(true)) {
        return;
    }
    // Setup's practice box, in Iris's own window: read directly.
    if let Some(captured) = crate::live_setup::own_capture(target, cx) {
        CAPTURING.with(|c| c.set(false));
        return then(captured, cx);
    }
    // Under test: only the app the test named, never whatever has focus on someone's screen.
    let pid = match target {
        Some(p) => Some(p),
        None if textbridge::under_test() => None,
        None => textbridge::focused_app(),
    };
    let Some(pid) = pid else {
        CAPTURING.with(|c| c.set(false));
        QUICK_FIXING.with(|q| q.set(false));
        explain(CaptureError::NothingToCheck, cx);
        return;
    };
    // Refuse excluded apps (password managers, etc.) before touching their AX tree or clipboard,
    // matching Swift's `AppDelegate.captureOrExplain`. Keep the post-capture check below as a
    // second guard in case `read.owner` switches the pid mid-capture. Windows: the program's
    // file name, as its "off" list keeps them.
    #[cfg(any(target_os = "macos", windows))]
    {
        let (app_name, bundle_id) = textbridge::app_info(pid);
        if crate::app::settings().is_excluded(bundle_id.as_deref()) {
            CAPTURING.with(|c| c.set(false));
            QUICK_FIXING.with(|q| q.set(false));
            crate::host_level::set_host(pid);
            let icon = excluded_icon(bundle_id.as_deref());
            notes_live::show(Message::turned_off(app_name.as_deref(), icon), Some("excluded"), None, cx);
            return;
        }
    }
    cx.spawn(async move |cx| {
        let read = cx.background_executor().spawn(async move { textbridge::capture(pid) }).await;
        let _ = cx.update(|cx| {
            CAPTURING.with(|c| c.set(false));
            match read {
                Ok(mut captured) => {
                    // What Iris shows by this box stands over its window (`host_level`).
                    if !captured.own {
                        crate::host_level::set_host(captured.pid);
                    }
                    if crate::live_setup::pretend_read_only() {
                        captured.can_replace = false;
                    }
                    let s = crate::app::settings();
                    if s.is_excluded(captured.bundle_id.as_deref()) {
                        QUICK_FIXING.with(|q| q.set(false));
                        let icon = excluded_icon(captured.bundle_id.as_deref());
                        notes_live::show(Message::turned_off(captured.app_name.as_deref(), icon), Some("excluded"), captured.field_frame, cx);
                        return;
                    }
                    then(captured, cx)
                }
                Err(e) => {
                    QUICK_FIXING.with(|q| q.set(false));
                    explain(e, cx)
                }
            }
        });
    })
    .detach();
}

/// Cached excluded-app toast icon (reused through `riso::PRINTS` instead of allocating a fresh
/// uncached `RenderImage` on every excluded-app trigger).
fn excluded_icon(bundle: Option<&str>) -> Option<crate::riso::Img> {
    let bundle = bundle?;
    let (rgba, n) = crate::app_icon(bundle)?;
    let plate = crate::riso::Plate { w: n as f32, h: n as f32, scale: 1.0 };
    Some(crate::riso::cached(format!("excluded-icon-{bundle}"), &plate, move || rgba))
}

/// A read that found nothing to check, said by the text (`captureOrExplain`).
fn explain(e: CaptureError, cx: &mut App) {
    match e {
        // Setup asks for the permission.
        CaptureError::NotTrusted => {
            news("needs Accessibility");
            crate::live_setup::show(cx);
        }
        CaptureError::SecureField => notes_live::show(Message::password(), None, None, cx),
        CaptureError::NothingToCheck => {
            let keys = crate::app::shortcut_caps(crate::app::settings().open_shortcut);
            let caps: Vec<&str> = keys.iter().map(String::as_str).collect();
            notes_live::show(Message::nothing_to_read(&caps), None, None, cx)
        }
    }
}

/// The panel for `captured`, on the tone it starts on (or with `answer` already in).
fn show(captured: Captured, _answer: Option<()>, cx: &mut App) {
    let modes = live::current_modes(cx);
    let start_id = if crate::mock::on() {
        brain::get(cx).map(|b| b.read(cx).starting.clone()).unwrap_or_else(|| "fix".into())
    } else {
        crate::tones::ToneList::from_settings(&crate::app::settings()).starting_for(captured.bundle_id.as_deref())
    };
    let starting = modes.iter().position(|m| m.id == start_id).unwrap_or(0);
    let frame = captured.field_frame;
    show_with(captured, frame, move |captured, cx| OpeningView::for_captured(captured, starting, cx), cx);
}

/// Opens the panel window with its text box's corner where the scene's box is.
fn show_with(captured: Captured, frame: Option<[f64; 4]>, make: impl FnOnce(Captured, &mut Context<OpeningView>) -> OpeningView + 'static, cx: &mut App) {
    // Under test nothing opens on someone's screen: a box off the test display (or none, which
    // would mean the pointer) becomes the test display's own spot for the panel.
    let frame = match crate::test_display() {
        Some((dx, dy)) if textbridge::under_test() && !frame.is_some_and(|[x, y, _, _]| crate::on_test_display(x, y)) => {
            news(&format!("panel: {frame:?} is off the test display; opening on it"));
            Some([dx + 220.0, dy + 360.0, 560.0, 58.0])
        }
        _ => frame,
    };
    // The pointer is on someone's screen: under test, a spot on the test display stands in.
    let pointer = match crate::test_display() {
        Some((dx, dy)) if textbridge::under_test() => (dx + 600.0, dy + 600.0),
        _ => crate::menubar::pointer(),
    };
    let at = frame.map_or(pointer, |[x, y, _, _]| (x, y));
    let s = crate::app::settings();
    // `PanelController.show`: above or below the box as set (the other side when there's no
    // room), or by the pointer; kept on the screen.
    let visible = crate::visible_frame_at(at.0, at.1);
    let ((x, y), below) = crate::opening::screen_place(frame, s.placement == crate::store::Placement::BelowField,
        s.placement == crate::store::Placement::NearPointer, pointer, visible, crate::panel::paper_guess(&captured.text, visible[3] as f32) as f64);
    if below {
        news("panel below the box");
    }
    // The paper stands where Swift's would; the window round it (the flower's room and the
    // shadow's, 900×460) is kept on the screen, its drawing shifted back by as much, so a box near
    // the screen's edge never hangs the window off it.
    let [vx, vy, vw, vh] = crate::visible_frame_at(at.0, at.1);
    let (sw, sh) = (crate::opening::SCENE.0 as f64, crate::opening::SCENE.1 as f64);
    let nx = if vw >= sw { x.clamp(vx, vx + vw - sw) } else { x };
    let ny = if vh >= sh { y.clamp(vy, vy + vh - sh) } else { y };
    let shift = ((x - nx) as f32, (y - ny) as f32);
    let origin = point(px(nx as f32), px(ny as f32));
    news(&format!("panel open: {} chars from {}", captured.text.chars().count(), captured.app_name.clone().unwrap_or_else(|| "an app".into())));
    // Over the window that holds the box, when that stands above Iris's own level.
    let host = (!captured.own).then_some(captured.pid);
    let h = crate::open_opening_with(origin, cx, move |cx| OpeningView { shift, ..make(captured, cx) });
    if let (Some(h), Some(pid)) = (&h, host) {
        let _ = h.update(cx, |_, window, _| crate::host_level::raise(window, Some(pid), frame));
    }
    live::set_open_panel(h);
}
