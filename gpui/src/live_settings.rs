//! Settings, live: the window the slip's Settings… opens (`--live-settings [x y] [page]` opens it
//! alone). The rail turns the page; each page scrolls; every control works, saves at once and
//! moves as the Swift one does (`settings_live`, `settings_controls`). Opened without taking the
//! keyboard from the app in front unless `IRIS_TAKE_KEYS` is set.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gpui::*;

use crate::settings::{self, Ctx, Fields, Page, Turn, Values};
use crate::interact::{self, Anim};
use crate::text_field::{Field, TextField};

pub struct SettingsView {
    pub page: Page,
    pub values: Values,
    pub anim: Rc<RefCell<Anim>>,
    shown: Page,
    reduce_motion: bool,
    fields: [Entity<TextField>; 4],
    /// The recorders' keyboard: where keys go while one listens.
    keys: [FocusHandle; 2],
}

/// What the window keeps between frames that isn't saved: the recorder listening, the Delete
/// under the pointer, the update check's answer.
#[derive(Default)]
pub struct Moment {
    pub recording: Option<crate::pages::shortcuts::Recording>,
    pub crossing: Option<String>,
    pub update_check: Option<crate::pages::updates::Check>,
    /// The version whose notes Updates shows (none: the newest).
    pub picked_version: Option<String>,
}

thread_local! {
    pub static MOMENT: RefCell<Moment> = RefCell::new(Moment::default());
    /// The Settings window, for the control socket.
    static WINDOW: RefCell<Option<WindowHandle<SettingsView>>> = const { RefCell::new(None) };
}

impl SettingsView {
    pub fn new(page: Page, window: &mut Window, cx: &mut Context<Self>) -> SettingsView {
        crate::text_field::init(cx);
        let fields: [Entity<TextField>; 4] = std::array::from_fn(|_| TextField::new(cx));
        // What's typed decides what the page shows (Add lights up): draw again as it changes.
        for f in &fields {
            cx.observe(f, |_, _, cx| cx.notify()).detach();
        }
        let keys: [FocusHandle; 2] = std::array::from_fn(|_| cx.focus_handle());
        // Clicking away (or the window going to the back) ends the listening, or the shortcuts
        // would stay let go of until someone came back.
        for (i, k) in keys.iter().enumerate() {
            cx.on_blur(k, window, move |_, _, cx| {
                crate::pages::shortcuts::stop(Some(i as i64 + 1));
                cx.notify();
            }).detach();
        }
        // About starts playing again, its phrases at their first catch.
        crate::pages::about::reset();
        SettingsView { page, values: Values::default(), anim: Rc::default(), shown: page, reduce_motion: crate::live_slip::reduce_motion(), fields, keys }
    }
}

/// The values the page draws, from the settings saved on this Mac and the brain.
fn values(anim: &Anim, reduce_motion: bool, page: Page, cx: &App) -> Values {
    let brain = crate::brain::get(cx).map(|b| b.read(cx));
    use crate::pages::notes::{self, NOTES};
    let s = crate::app::settings();
    let mut v = Values::default();
    v.placement = match s.placement {
        crate::store::Placement::AboveField => 0,
        crate::store::Placement::BelowField => 1,
        crate::store::Placement::NearPointer => 2,
    };
    let tones = crate::tones::ToneList::from_settings(&s);
    v.modes = tones.shown().iter().map(|m| (m.id.clone(), m.label.clone())).collect();
    v.starts_in = v.modes.iter().position(|m| m.0 == tones.default_id).unwrap_or(0);
    v.show_changes = s.show_changes;
    v.panel_when_clean = s.show_panel_when_clean;
    v.notes = std::array::from_fn(|i| s.shows(settings::NOTE_NAMES[i]));
    // Asking macOS whether Iris opens at login takes about 20 ms: only General shows it, and
    // About and Updates draw every frame while they move.
    v.open_at_login = page == Page::General && crate::login_item::enabled();
    v.login_problem = crate::login_item::problem();
    let landed = |i: usize| notes::landed(NOTES[i]);
    let (i, t) = anim.demo.playing(Instant::now(), notes::LENGTH, NOTES.len(), if reduce_motion { Some(&landed) } else { None });
    v.demo = Some((NOTES[i], t));
    v.demo_keys = crate::tones::caps(s.quick_fix_shortcut.unwrap_or(s.open_shortcut));
    v.open_shortcut = s.open_shortcut;
    v.quick_fix = s.quick_fix_shortcut;
    v.tones = settings::tone_rows(&s);
    v.style_rules = s.style_rules.clone();
    v.glossary = s.glossary.clone();
    v.excluded = s.excluded_apps.iter().map(|a| (a.bundle_id.clone(), a.name.clone())).collect();
    v.per_app = tones.per_app.clone();
    v.idle_minutes = s.idle_minutes;
    v.load_at_launch = s.load_at_launch;
    MOMENT.with(|m| {
        let m = m.borrow();
        v.recording = m.recording.clone();
        // Check for Updates, as the app's one check has it (the slip shows the same).
        v.update_check = m.update_check.or_else(|| Some(crate::pages::updates::from_phase(&crate::updater::phase(cx))));
        v.model = brain.map(|b| crate::pages::model::Live {
            engine: b.engine.clone(),
            chosen: b.chosen.clone().or(s.model.clone()).unwrap_or_else(|| b.recommended.clone().unwrap_or_else(|| "best".into())),
            models: b.models.iter().map(|m| (m.id.clone(), m.download_size.clone(), m.downloaded)).collect(),
            recommended: b.recommended.clone().unwrap_or_else(|| "best".into()),
            crossing: m.crossing.clone(),
        });
    });
    v
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let began = Instant::now();
        crate::riso::let_go_of_last_frame(window);
        crate::pages::about::let_go(window);
        // A page turned to starts at rest: nothing on it replays.
        if self.shown != self.page {
            // A recorder on the page left stops listening (Swift's `onDisappear`).
            crate::pages::shortcuts::stop(None);
            self.anim.borrow_mut().reset();
            crate::pages::about::reset();
            self.shown = self.page;
        }
        self.values = values(&self.anim.borrow(), self.reduce_motion, self.page, cx);
        let t_values = began.elapsed().as_secs_f64() * 1000.0;
        let [word, tone_name, tone_how, rule] = &self.fields;
        let field = |e: &Entity<TextField>| Field::of(e, window, cx);
        self.values.fields = Some(Fields { keys: self.keys.clone(), word: field(word), tone_name: field(tone_name), tone_how: field(tone_how), rule: field(rule) });
        let entity = cx.entity();
        let press = move |p: Page| -> Box<dyn Fn(&ClickEvent, &mut Window, &mut App)> {
            let e = entity.clone();
            Box::new(move |_, _, cx| e.update(cx, |v, cx| { v.page = p; cx.notify() }))
        };
        let turn = Turn { press: &press };
        let scale = window.scale_factor();
        let (anim, values, page_) = (self.anim.clone(), &self.values, self.page);
        let page = interact::build(&anim, self.reduce_motion, window, cx, || settings::window_live(page_, values, &turn, &Ctx { scale }));
        let t_build = began.elapsed().as_secs_f64() * 1000.0;
        // The note demo plays: draw the next frame too.
        if self.page == Page::General && !self.reduce_motion {
            window.request_animation_frame();
        }
        // `IRIS_TRACE`: the main thread's time for this frame, building to painting.
        let trace = std::env::var("IRIS_TRACE").is_ok().then(|| canvas(|_, _, _| (), move |_, _, _, _| {
            eprintln!("settings frame cpu_ms={:.3} values={t_values:.2} built={t_build:.2}", began.elapsed().as_secs_f64() * 1000.0);
        }).absolute().size_0());
        div().size_full()
            .child(page).child(crate::confirm::layer(window, cx)).children(trace)
    }
}

/// Opens Settings at `page`, its top left at `origin` (screen points).
pub fn open(origin: Point<Pixels>, page: Page, cx: &mut App) -> Option<WindowHandle<SettingsView>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(settings::WIDTH), px(settings::TITLE_BAR + settings::LIVE_H)) })),
        // A see-through title bar over the page's own paper band, with the window's buttons on it.
        titlebar: Some(TitlebarOptions { title: Some("Iris Settings".into()), appears_transparent: true, traffic_light_position: None }),
        focus: crate::takes_keys(),
        show: true,
        kind: WindowKind::Normal,
        is_resizable: false,
        ..Default::default()
    };
    let h = cx.open_window(options, |window, cx| {
        crate::color_managed(window);
        cx.new(|cx| {
            // Back to Settings: Open at login may have changed in System Settings meanwhile.
            cx.observe_window_activation(window, |_, window, cx| {
                if window.is_window_active() {
                    crate::login_item::forget();
                    cx.notify();
                }
            }).detach();
            // The model's state, as the brain tells it, redraws the page; so does the update check.
            if let Some(b) = crate::brain::get(cx) {
                cx.observe(&b, |_, _, cx| cx.notify()).detach();
            }
            let st = crate::updater::state(cx);
            cx.observe(&st, |_, _, cx| cx.notify()).detach();
            SettingsView::new(page, window, cx)
        })
    }).ok();
    WINDOW.with(|w| *w.borrow_mut() = h);
    h
}

/// For a test, which can't type into a window that isn't in front: presses `keystroke` (`a`,
/// `cmd-a`, `enter`, `escape`…) in Settings through its own key handling, as if typed. True if
/// something took it.
pub fn key(keystroke: &str, cx: &mut App) -> bool {
    let Some(h) = WINDOW.with(|w| *w.borrow()) else { return false };
    // Through the window, not the view: the keys' handlers update the view themselves.
    cx.update_window(h.into(), |_, window, cx| crate::text_field::key_into(window, keystroke, cx)).unwrap_or(false)
}

/// For a test: types `text` into Settings, a key at a time, through its own key handling.
pub fn type_text(text: &str, cx: &mut App) -> bool {
    let Some(h) = WINDOW.with(|w| *w.borrow()) else { return false };
    cx.update_window(h.into(), |_, window, cx| {
        let mut took = true;
        for ch in text.chars() {
            let key = if ch == ' ' { "space".to_string() } else { ch.to_lowercase().to_string() };
            let k = Keystroke { modifiers: Modifiers { shift: ch.is_uppercase(), ..Default::default() }, key, key_char: Some(ch.to_string()) };
            took &= window.dispatch_keystroke(k, cx);
        }
        took
    }).unwrap_or(false)
}

/// A page by the name the scenes use (`general`, `model`…).
pub fn page_named(name: &str) -> Option<Page> {
    Page::from_slug(name)
}

/// Open setup (`irisShowOnboarding`): the setup window, at the step that isn't done.
pub fn open_setup(cx: &mut App) {
    crate::live_setup::show(cx);
}
