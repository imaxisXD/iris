//! Settings (`SettingsView`): one printed page. The rail of pages and the chosen page share the
//! paper with a faint pen line between them; each page has a running head, a stepped headline,
//! its lettering in the corner (a ghost word behind, a hand word across it), numbered sections
//! of rows, and ends on a pink foot with its folio.
//!
//! The Rune copy's Settings in GPUI. What Rune sets in Martian Mono at a weight or width GPUI
//! has no cut for (the headline and folio at width 75, the sign-off at 600, the section folios
//! at 300, the words up the rule at 600 and width 100) is printed from the variable font's
//! outlines (`settings_kit::martian_line`). What Rune turns (the Report a bug arrow, the words up
//! the rule) is printed whole, turned.

use gpui::{App, ClickEvent, Div, InteractiveElement, IntoElement, ParentElement, Role, StatefulInteractiveElement, Styled, Window, WindowControlArea, div};
use tiny_skia::{FillRule, PathBuilder, Rect, Transform};

use crate::icons::Icon;
use crate::letters::{self, Ghost};
use crate::parts::*;
use crate::pen;
use crate::raster::Sheet;
use crate::riso::{self, Plate, Rgb8, col, hue, ink, over};
use crate::settings_kit::*;

pub const WIDTH: f32 = 940.0;
pub const RAIL: f32 = 214.0;
const PAGE_MAX: f32 = 700.0;
const LETTERING_INSET: f32 = 87.0;
const GHOST_SIZE: f32 = 124.0;
const FOOT_H: f32 = 114.0;
const I4: Rgb8 = (0xE1, 0xD9, 0xD2);
/// The live window's height under its title band.
pub const LIVE_H: f32 = 740.0;
/// The paper band the live window drags by.
pub const TITLE_BAR: f32 = 28.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    General,
    Shortcuts,
    Tones,
    Words,
    Apps,
    Model,
    Privacy,
    Updates,
    About,
}

pub const PAGES: [Page; 9] = [Page::General, Page::Shortcuts, Page::Tones, Page::Words, Page::Apps, Page::Model, Page::Privacy, Page::Updates, Page::About];

impl Page {
    pub fn index(self) -> usize {
        PAGES.iter().position(|p| *p == self).unwrap()
    }
    pub fn title(self) -> &'static str {
        ["General", "Shortcuts", "Tones", "Words", "Apps", "Model & memory", "Privacy", "Updates", "About"][self.index()]
    }
    /// The scene name the snapshots know the page by (`settings-general`, …).
    pub fn slug(self) -> &'static str {
        ["general", "shortcuts", "tones", "words", "apps", "model", "privacy", "updates", "about"][self.index()]
    }
    pub fn from_slug(s: &str) -> Option<Page> {
        PAGES.iter().copied().find(|p| p.slug() == s)
    }
    /// The Swift picture's height in points (a snapshot draws the page whole), for a snapshot
    /// that doesn't give its size. These move with the pages (Updates with every changelog entry).
    pub fn snapshot_height(self) -> f32 {
        [1639.0, 936.0, 1111.0, 729.0, 829.0, 1172.5, 627.0, 1180.0, 1118.0][self.index()]
    }
    fn icon(self) -> Icon {
        [Icon::General, Icon::Shortcuts, Icon::Tones, Icon::Words, Icon::Apps, Icon::Model, Icon::Privacy, Icon::Updates, Icon::About][self.index()]
    }
    fn heading(self) -> [&'static str; 2] {
        [["How Iris", "behaves"], ["Keys for", "your hands"], ["Tones based", "on your words"], ["Words to", "never change"],
         ["Where Iris", "works"], ["The model", "on this Mac"], ["Nothing", "leaves here"], ["What's new", "in Iris"], ["Who made", "Iris"]][self.index()]
    }
    fn running_head(self) -> [String; 3] {
        let n = format!("{:02} of {:02}", self.index() + 1, PAGES.len());
        let [a, b] = [["When and where Iris shows up", "Changes save right away"], ["The keys that call Iris", "They work in any app"],
                      ["How each rewrite sounds", "Names and links stay as written"], ["Names and terms to keep", "Iris never changes them"],
                      ["Where Iris reads text", "Turn it off in any app"], ["What runs on this Mac", "Downloaded once"],
                      ["What Iris can see", "Your text stays here"], ["New versions and what changed", "Iris looks only when you ask"],
                      ["Who made Iris and why", "Free and open source"]][self.index()];
        [n, a.into(), b.into()]
    }
    fn lettering(self) -> (&'static str, &'static str, Ghost) {
        [("Habits", "Basics", Ghost::Khaki), ("Touch", "Press", Ghost::Sky), ("Sound", "Tones", Ghost::Pink), ("Words", "Keep", Ghost::Tan),
         ("Places", "Where", Ghost::Sky), ("Local", "Model", Ghost::Khaki), ("Yours", "Private", Ghost::Pink), ("New", "Latest", Ghost::Sky),
         ("Story", "Thanks", Ghost::Tan)][self.index()]
    }
}

/// The settings people can change here. `Default` is what the Swift app's snapshots set (every
/// snapshot draws it); the live window fills it from the settings saved on this Mac (`live`).
#[derive(Clone, Debug)]
pub struct Values {
    pub placement: usize,
    /// The tones the panel shows (id, name): what Starts in offers.
    pub modes: Vec<(String, String)>,
    /// Which of `modes` the panel starts in.
    pub starts_in: usize,
    pub show_changes: bool,
    pub panel_when_clean: bool,
    pub notes: [bool; 5],
    pub open_at_login: bool,
    /// What macOS still needs before Iris opens at login.
    pub login_problem: Option<String>,
    /// The note demo live: the clip on the stage and how far into it (none: held as the
    /// snapshot holds it).
    pub demo: Option<(crate::pages::notes::Note, f64)>,
    /// The keys the demo shows (Quick Fix's, or Open Iris's when Quick Fix is off).
    pub demo_keys: Vec<crate::pages::notes::Cap>,
    /// Shortcuts: Open Iris, Quick Fix (none: off), and the recorder that's listening.
    pub open_shortcut: crate::store::KeyCombo,
    pub quick_fix: Option<crate::store::KeyCombo>,
    pub recording: Option<crate::pages::shortcuts::Recording>,
    /// Every tone in order, hidden ones included, with the key that picks it in the panel.
    pub tones: Vec<ToneRow>,
    pub style_rules: Vec<String>,
    pub glossary: Vec<String>,
    /// The apps Iris is off in (bundle ID, name), and the apps with a tone of their own
    /// (bundle ID, tone ID).
    pub excluded: Vec<(String, String)>,
    pub per_app: Vec<(String, String)>,
    /// The model page (none: as the Swift snapshot finds this Mac).
    pub model: Option<crate::pages::model::Live>,
    pub idle_minutes: u32,
    pub load_at_launch: bool,
    /// The text fields, live (none in a snapshot: an empty well).
    pub fields: Option<Fields>,
    /// What Check for Updates came back with, live.
    pub update_check: Option<crate::pages::updates::Check>,
}

/// A tone's row on Tones.
#[derive(Clone, Debug)]
pub struct ToneRow {
    pub id: String,
    pub label: String,
    pub instruction: String,
    pub shown: bool,
    pub custom: bool,
    pub key: Option<usize>,
}

/// The fields Settings types into.
#[derive(Clone, Debug)]
pub struct Fields {
    /// The two recorders' keyboard (Open Iris, Quick Fix).
    pub keys: [gpui::FocusHandle; 2],
    pub word: crate::text_field::Field,
    pub tone_name: crate::text_field::Field,
    pub tone_how: crate::text_field::Field,
    pub rule: crate::text_field::Field,
}

/// The tone rows a set of settings gives.
pub fn tone_rows(s: &crate::store::Settings) -> Vec<ToneRow> {
    let list = crate::tones::ToneList::from_settings(s);
    list.all().into_iter().map(|m| ToneRow {
        shown: !list.hidden.contains(&m.id),
        custom: m.id.starts_with(crate::tones::CUSTOM),
        key: list.key(&m.id),
        id: m.id, label: m.label, instruction: m.instruction,
    }).collect()
}

impl Default for Values {
    fn default() -> Self {
        let s = crate::store::Settings::default();
        Values {
            placement: 0, modes: vec![("fix".into(), "Fix".into())], starts_in: 0, show_changes: true, panel_when_clean: true,
            notes: [true; 5], open_at_login: false, login_problem: None, demo: None, demo_keys: crate::pages::notes::QUICK_FIX.to_vec(),
            // The snapshots draw the installed Iris's shortcuts.
            open_shortcut: crate::store::KeyCombo::OPEN, quick_fix: Some(crate::store::KeyCombo::QUICK_FIX), recording: None,
            tones: tone_rows(&s), style_rules: vec![], glossary: vec![],
            excluded: s.excluded_apps.iter().map(|a| (a.bundle_id.clone(), a.name.clone())).collect(), per_app: vec![],
            model: None, idle_minutes: 30, load_at_launch: true, fields: None, update_check: None,
        }
    }
}

/// The notes people can turn off (`AppSettings.Note`), in the rows' order, by their stored names.
pub const NOTE_NAMES: [&str; 5] = ["undo", "undoKeys", "checking", "clean", "excluded"];

/// The height a headless snapshot asked for (`--size WxH`), so the page is laid out as tall as
/// the picture it's drawn into.
pub fn asked_height() -> Option<f32> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == "--size")?;
    args.get(i + 1)?.split('x').nth(1)?.parse().ok()
}

pub struct Ctx {
    pub scale: f32,
}

/// What the live window's rail does: a row pressed turns to its page (`slip::Acts`' style).
pub struct Turn<'a> {
    pub press: &'a dyn Fn(Page) -> Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
}

/// The whole window as the snapshots draw it: rail, rule, page, `height` tall (the page whole).
pub fn window(page: Page, v: &Values, height: f32, cx: &Ctx) -> Div {
    let content_w = WIDTH - RAIL - 3.0;
    let body = page_body(page, v, cx);
    let grain = printed(WIDTH, height, &riso::grain(WIDTH, height, cx.scale, 0.22));
    let page_col = column().w(z(content_w)).h(z(height)).child(body).child(grow()).child(page_foot(page, content_w, cx.scale));
    let body = row().w(z(WIDTH)).h(z(height)).items_start()
        .child(rail(page, height, None, cx))
        .child(divider(height, cx))
        .child(page_col);
    sized(WIDTH, height).bg(col(ink::PAPER))
        // The paper's tooth, under everything: wells and washes print over it, flat.
        .child(at(grain, 0.0, 0.0))
        .child(at(body, 0.0, 0.0))
}

/// The Settings window live: a paper title band the window drags by, then the rail and the
/// page, the page scrolling inside a fixed height (`LIVE_H`), its pink foot at its end. The
/// rail's rows turn the page through `turn`. The window is `WIDTH` × `TITLE_BAR + LIVE_H`.
pub fn window_live(page: Page, v: &Values, turn: &Turn, cx: &Ctx) -> Div {
    let height = LIVE_H;
    let content_w = WIDTH - RAIL - 3.0;
    let body = page_body(page, v, cx);
    let page_col = column().w(z(content_w)).min_h(z(height)).child(body).child(grow()).child(page_foot(page, content_w, cx.scale));
    // A scroll view per page, keyed by the page, so a page you turn to opens at its top.
    let scroller = div().id(gpui::SharedString::from(format!("settings-page-{}", page.index())))
        .flex_none().w(z(content_w)).h(z(height)).overflow_y_scroll().child(page_col);
    let grain = printed(WIDTH, height + TITLE_BAR, &riso::grain(WIDTH, height + TITLE_BAR, cx.scale, 0.22));
    let body = row().w(z(WIDTH)).h(z(height)).items_start()
        .child(rail(page, height, Some(turn), cx))
        .child(divider(height, cx))
        .child(scroller);
    let drag = div().id("settings-title-band").w(z(WIDTH)).h(z(TITLE_BAR)).window_control_area(WindowControlArea::Drag)
        .on_mouse_down(gpui::MouseButton::Left, |_, window, _| window.start_window_move());
    sized(WIDTH, height + TITLE_BAR).bg(col(ink::PAPER))
        .child(at(grain, 0.0, 0.0))
        .child(at(drag, 0.0, 0.0))
        .child(at(body, 0.0, TITLE_BAR))
}

// MARK: - The rail

fn rail(current: Page, height: f32, turn: Option<&Turn>, cx: &Ctx) -> Div {
    let (logo, _, _) = letters::iris_logo(26.0, cx.scale);
    let mut r = column().w(z(RAIL)).h(z(height)).px(z(10.0)).gap(z(1.0))
        .child(div().flex_none().pl(z(8.0)).pt(z(40.0)).pb(z(18.0)).child(logo));
    let l = crate::interact::get();
    for p in PAGES {
        let here = p == current;
        let w = RAIL - 20.0;
        // `RailRow`: the current page two steps of wash, the one under the pointer one, fading
        // in place (`penWash`); its words darken; squeezed to 0.99 while held.
        let face = move |hover: f32| {
            let mut item = sized(w, 32.0);
            if here || hover > 0.001 {
                item = item.child(at(crate::controls::wash(p.title().len() as i64 * 5 + 1, w, 32.0, hover, here), 0.0, 0.0));
            }
            item.child(at(row().w(z(w)).h(z(32.0)).px(z(10.0)).gap(z(10.0)).items_center()
                .child(printed(24.0, 24.0, &p.icon().image_at(24.0, cx.scale)))
                .child(t(p.title(), body(), if here { hue::I12 } else { crate::interact::mix(hue::I11, hue::I12, hover) })), 0.0, 0.0))
                .into_any_element()
        };
        let item = match (&l, turn) {
            (Some(l), Some(tn)) if l.view.is_some() => crate::interact::pressable(l, &format!("rail-{}", p.slug()), w, 32.0, 0.99,
                Some(crate::interact::act(|_, _| {})), face).on_click((tn.press)(p)),
            (_, Some(tn)) => div().flex_none().id(gpui::SharedString::from(format!("settings-rail-{}", p.slug()))).child(face(0.0)).on_click((tn.press)(p)),
            _ => div().flex_none().id(gpui::SharedString::from(format!("settings-rail-{}", p.slug()))).child(face(0.0)),
        };
        r = r.child(item.role(Role::Tab).aria_label(p.title()).aria_selected(here));
    }
    r.child(grow().min_h(z(12.0))).child(div().flex_none().pb(z(26.0)).child(report_bug(cx.scale)))
}

/// Report a bug, at the rail's foot under a short pen line (`ReportBugRow`): live, the rail's
/// hover wash comes in under it and its words darken (`DS.easeOut`), squeezed to 0.99 while
/// held; it opens a new GitHub issue with this copy's versions filled in.
fn report_bug(scale: f32) -> Div {
    let l = crate::interact::get();
    let w = 10.0 + 24.0 + 10.0 + mono_w("Report a bug", 12.5, 0.0) + 4.0 + 22.0 * 0.8 + 8.0;
    let face = move |hover: f32| {
        let arrow = arrow_turned(22.0 * 0.8, 12.0 * 0.8, 2.3 * 0.9, 31, -35.0, scale);
        let mut f = sized(w, 32.0);
        if hover > 0.001 {
            f = f.child(at(hand_fill(41, 0.5, w, 32.0, crate::interact::alpha(col(hue::I3), hover)), 0.0, 0.0));
        }
        f.child(at(row().h(z(32.0)).pl(z(10.0)).pr(z(8.0)).gap(z(10.0)).items_center()
            .child(printed(24.0, 24.0, &Icon::Bug.image_at(24.0, scale)))
            .child(row().gap(z(4.0)).items_center().child(t("Report a bug", body(), crate::interact::mix(hue::I11, hue::I12, hover))).child(arrow)), 0.0, 0.0))
            .into_any_element()
    };
    let button = match l {
        Some(l) if l.view.is_some() => crate::interact::pressable(&l, "report-bug", w, 32.0, 0.99,
            Some(crate::interact::act(|_, _| crate::controls::open_url(&crate::pages::about::problem_report_url()))), face)
            .role(Role::Link).aria_label("Report a bug").into_any_element(),
        _ => div().flex_none().id("settings-report-bug").role(Role::Link).aria_label("Report a bug").child(face(0.0)).into_any_element(),
    };
    column().w(z(194.0)).gap(z(10.0))
        .child(div().flex_none().pl(z(10.0)).child(pen_rule(777, 150.0, over(hue::I12, 0.3, ink::PAPER), 1.0)))
        .child(button)
}

/// The faint pen line between rail and page, with the words up it.
fn divider(height: f32, cx: &Ctx) -> Div {
    let ink_ = over(hue::I12, 0.3, ink::PAPER);
    // A length of the rule from `top` to `bottom` (window points).
    let rule = |seed: i64, top: f32, bottom: f32| {
        let h = bottom - top;
        at(drawn(3.0, h, pen::hand_line_vertical(seed, 0.6, 1.5, 0.0, h), Some(1.0), ink_), 0.0, top)
    };
    let (top, bottom) = (24.0, height - 24.0);
    let centre = height - FOOT_H - 24.0 - 101.0;
    if centre - 101.0 < 399.0 {
        return sized(3.0, height).child(rule(3, top, bottom));
    }
    // The words turned up the rule, centred `centre` down the window (measured against the
    // Swift drawing on every page). The rule stops short of them and starts again past them,
    // so the window's own paper shows round the words: no strip of paper of their own.
    let (sw, sh, words) = edge_strip("Iris ✻ grammar and tone", cx.scale);
    let (from, to) = (centre - sh / 2.0, centre + sh / 2.0);
    sized(3.0, height)
        .child(rule(3, top, from))
        .child(rule(4, to, bottom))
        .child(at(printed(sw, sh, &words), 1.5 - sw / 2.0, from))
}

/// `EdgeText` on its strip of paper, turned up the rule, printed whole: (width, height, picture),
/// as it stands (the strip's height across, its length up).
fn edge_strip(s: &str, scale: f32) -> (f32, f32, riso::Img) {
    let size = 8.0;
    let tracking = 2.4;
    let upper = s.to_uppercase();
    // The star stands out from the words: bigger and heavier than the caps, with room round it
    // (setup's ✻, `letters::star_into`).
    let star_r = 5.4;
    let adv = |c: char| if c == '✻' { star_r * 2.0 + 2.0 } else { mono_adv(size, 600.0, 100.0) };
    let w: f32 = upper.chars().map(|c| adv(c) + tracking).sum();
    let h = (size * MONO_LINE).ceil().max((star_r * 2.0 + 2.0f32).ceil());
    let (sw, sh) = (h, w + 12.0);
    let plate = Plate { w: sw, h: sh, scale };
    let img = riso::cached(format!("settings-edge-{s}"), &plate, || {
        // The words, small tracked caps in the sky ink through the screen, a heavy ✻ between.
        // Clear round them: the window's paper and grain show through.
        let words = Plate { w, h, scale };
        let mut mask = words.mask();
        let mut x = 0.0;
        let base = h - (h - size * 1.2) / 2.0 - size * 0.2;
        for c in upper.chars() {
            if c == '✻' {
                letters::star_into(&mut mask, &words, x + star_r + 1.0, h / 2.0, star_r);
            } else if c != ' ' {
                letters::martian_glyph_into(&mut mask, &words, c, size, 600.0, 100.0, x, base);
            }
            x += adv(c) + tracking;
        }
        let rgba = riso::print(&words, (120, 162, 220), s.len() as i64 * 3, &mask, 1.0);
        let (pw, ph) = words.px();
        let mut sheet = Sheet::new(plate, 0.0, 0.0, 0.0);
        let up = Transform::from_translate(sw / 2.0, sh / 2.0).pre_rotate(-90.0).pre_translate(-(w + 12.0) / 2.0, -h / 2.0);
        sheet.picture_at(&rgba, pw, ph, w, h, up.pre_translate(6.0, 0.0), 1.0);
        sheet.rgba()
    });
    (sw, sh, img)
}

// MARK: - The page

fn page_body(page: Page, v: &Values, cx: &Ctx) -> Div {
    let inner = PAGE_MAX - 88.0;
    let head = running_head(&page.running_head(), inner - 150.0);
    let title = page_title(&page.heading(), cx.scale);
    let mut stack = column().w(z(PAGE_MAX)).px(z(44.0)).pt(z(44.0)).pb(z(52.0)).gap(z(30.0))
        .child(column().gap(z(26.0)).child(head).child(title));
    for section in page_content(page, v, inner, cx) {
        stack = stack.child(section);
    }
    // The corner: the ghost word behind, the hand word across its middle, centred on one line.
    let (ghost, word, ink_) = page.lettering();
    let (gpx, gw, gh) = letters::ghost_word(ghost, GHOST_SIZE, ink_, cx.scale);
    let size = if word.len() > 5 { 41.0 } else { 50.0 };
    let (hpx, hw, hh) = letters::hand_word(word, size, page.index() as i64 * 7, &[], 1.0, cx.scale);
    let centre = PAGE_MAX - LETTERING_INSET;
    let ghost_y = if ghost.len() < 4 { 57.0 } else { 14.0 };
    div().flex_none().relative().w(z(PAGE_MAX))
        .child(at(printed(gw, gh, &gpx), centre - gw / 2.0, ghost_y))
        .child(stack)
        .child(at(printed(hw, hh, &hpx), centre - hw / 2.0, 102.0))
}

/// `RunningHead`: small columns over short pen rules.
fn running_head(items: &[String; 3], w: f32) -> Div {
    let cw = (w - 36.0) / 3.0;
    let mut r = row().gap(z(18.0)).items_start();
    for (i, item) in items.iter().enumerate() {
        r = r.child(column().w(z(cw)).gap(z(5.0))
            .child(para_out(item, martian(9.5, 450), hue::I10, cw))
            .child(pen_rule(40 + i as i64 * 7, 64.0, over(hue::I12, 0.35, ink::PAPER), 1.0)));
    }
    r
}

/// `PageTitle`: condensed caps that step right, almost touching (Martian at width 75, printed).
fn page_title(lines_: &[&str; 2], scale: f32) -> gpui::Stateful<Div> {
    let lh = (30.0f32 * MONO_LINE).ceil();
    let mut c = column();
    for (i, line) in lines_.iter().enumerate() {
        let (l, _) = martian_line(&line.to_uppercase(), 30.0, 400.0, 75.0, -0.8, lh, hue::I12, scale);
        c = c.child(div().flex_none().pl(z(i as f32 * 36.0)).mt(z(if i > 0 { -8.0 } else { 0.0 })).child(l));
    }
    c.id("settings-page-title").role(Role::Heading).aria_label(lines_.join(" "))
}

/// `PageFoot`: printed solid pink, a stepped sign-off, the folio big.
fn page_foot(page: Page, w: f32, scale: f32) -> Div {
    let folio = format!("/{:02}", page.index() + 1);
    let plate = Plate { w, h: FOOT_H, scale };
    let pink = riso::cached(format!("settings-foot-{folio}-{w}"), &plate, || {
        let mut m = plate.mask();
        m.fill_path(&PathBuilder::from_rect(Rect::from_xywh(0.0, 0.0, w, FOOT_H).unwrap()), FillRule::Winding, false, plate.transform());
        riso::print_with(&plate, (236, 110, 176), folio.len() as i64 * 5 + 3, 1.0, 1.0, &m, 1.0, |c| c)
    });
    let light = (255, 251, 247);
    let mut sign = column();
    for (i, line) in ["your words", "— stay on", "this Mac"].iter().enumerate() {
        let (l, _) = martian_line(line, 11.5, 600.0, 87.5, 0.0, 11.5 * MONO_LINE, light, scale);
        sign = sign.child(div().flex_none().pl(z(i as f32 * 18.0)).mt(z(if i > 0 { 1.0 } else { 0.0 })).child(l));
    }
    let (big, _) = martian_line(&folio, 58.0, 400.0, 75.0, -1.5, (58.0f32 * MONO_LINE).ceil(), light, scale);
    sized(w, FOOT_H)
        .child(at(printed(w, FOOT_H, &pink), 0.0, 0.0))
        .child(at(row().w(z(w)).h(z(FOOT_H)).px(z(32.0)).pt(z(26.0)).pb(z(18.0)).items_end()
            .child(sign).child(grow()).child(big), 0.0, 0.0))
}

// MARK: - Pages

fn page_content(page: Page, v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    use crate::pages;
    match page {
        Page::General => general(v, w, cx),
        Page::Shortcuts => pages::shortcuts::content(v, w, cx),
        Page::Tones => pages::tones::content(v, w, cx),
        Page::Words => pages::words::content(v, w, cx),
        Page::Apps => pages::apps::content(v, w, cx),
        Page::Model => pages::model::content(v, w, cx),
        Page::Privacy => pages::privacy::content(v, w, cx),
        Page::Updates => pages::updates::content(v, w, cx),
        Page::About => pages::about::content(v, w, cx),
    }
}

fn general(v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    use crate::controls as c;
    use crate::interact::{act, change};
    use crate::store::Placement;
    let s = cx.scale;
    let place = ["Above", "Below", "By the mouse"];
    let clean = ["Panel", "Message"];
    let labels: Vec<String> = v.modes.iter().map(|m| m.1.clone()).collect();
    let ids: Vec<String> = v.modes.iter().map(|m| m.0.clone()).collect();
    let starts = c::menu(labels, v.starts_in, None, 170.0, move |i, _| {
        let id = ids[i].clone();
        crate::app::change(|s| s.default_mode = Some(id.clone()));
        crate::brain::tones(None);
    });
    let panel = column().gap(z(12.0)).w(z(w))
        .child(setting_row("Where it opens", Some("Next to your text box. It moves to the other side when there's no room."),
            c::choice(&place, v.placement, "Where it opens", s, |i| change(move |s| s.placement = [Placement::AboveField, Placement::BelowField, Placement::NearPointer][i])),
            ink_choice_w(&place), w))
        .child(hairline(536, w))
        .child(setting_row("Starts in", Some("Fix keeps your words and only corrects mistakes."), starts, 170.0, w))
        .child(hairline(541, w))
        .child(setting_row("Show what changed", Some("New words in pink, removed ones crossed out."),
            c::toggle(v.show_changes, "Show what changed", s, change(|s| s.show_changes = !s.show_changes)), 50.0, w))
        .child(hairline(545, w))
        .child(setting_row("Nothing to fix", Some("Show the panel, or a small message that fades."),
            c::choice(&clean, if v.panel_when_clean { 0 } else { 1 }, "Nothing to fix", s, |i| change(move |s| s.show_panel_when_clean = i == 0)),
            ink_choice_w(&clean), w));

    let notes = [
        ("After replacing", "Says “fixed 4 words” or “replaced”."),
        ("With the undo keys", "Adds “Undo with ⌘ Z” to that message."),
        ("While Quick Fix works", "Says “checking…” while Iris reads your text."),
        ("When there's nothing to fix", "Says “looks good”."),
        ("Where Iris is off", "Says “turned off here” if you press the shortcut there."),
    ];
    let live = crate::interact::get();
    // The clip on the stage (live), or the first row's, as the snapshot holds it.
    let on_stage = v.demo.map(|(n, _)| crate::pages::notes::NOTES.iter().position(|x| *x == n).unwrap_or(0)).unwrap_or(0);
    let mut rows = column().gap(z(12.0)).w(z(w));
    for (i, (title, detail)) in notes.iter().enumerate() {
        let r = match &live {
            None => {
                let r = column().w(z(w)).py(z(6.0)).child(setting_row(title, Some(detail), switch(v.notes[i], title, s), 50.0, w));
                // The row on the stage (the first, in a snapshot) takes the rail's hover wash.
                if i == 0 {
                    div().flex_none().relative().w(z(w)).child(at(hand_fill(title.len() as i64 * 5 + 1, 0.5, w + 16.0, 44.0, col(hue::I3)), -8.0, 0.0)).child(r)
                } else { r }.into_any_element()
            }
            Some(l) => {
                // The keys ride on the replace message: with that off there's nothing to add them to.
                let enabled = i != 1 || v.notes[0];
                let name = NOTE_NAMES[i];
                let flip = change(move |s| {
                    if s.shows(name) { s.hidden_notes.push(name.into()) } else { s.hidden_notes.retain(|n| n != name) }
                });
                let row_ = column().w(z(w)).py(z(6.0)).child(setting_row(title, Some(detail), c::toggle_if(v.notes[i], title, s, enabled, flip), 50.0, w));
                let row_ = faded(row_, if enabled { 1.0 } else { 0.45 });
                // The row on the stage takes the rail's hover fill, fading in place; pointing at a
                // row holds its clip.
                let current = l.track(&format!("note-row-{i}"), (i == on_stage) as u8 as f32, crate::interact::Curve::FAST);
                let hold = {
                    let anim = l.anim.clone();
                    std::rc::Rc::new(move |on: bool, _: &mut App| anim.borrow_mut().demo.hold(if on { Some(i) } else { None }, i))
                };
                div().flex_none().relative().w(z(w))
                    .child(at(hand_fill(title.len() as i64 * 5 + 1, 0.5, w + 16.0, 44.0, crate::interact::alpha(col(hue::I3), current)), -8.0, 0.0))
                    .child(row_)
                    .id(gpui::SharedString::from(format!("note-row-{i}")))
                    .on_hover(l.hover_handler(&format!("note-row-{i}"), Some(hold)))
                    .into_any_element()
            }
        };
        rows = rows.child(r);
        if i < notes.len() - 1 {
            rows = rows.child(hairline(700 + i as i64, w));
        }
    }
    // The demo (`NoteDemo`): a message in use, held where the Swift snapshot holds it, or live.
    let demo = column().w(z(w)).pb(z(8.0)).child(crate::pages::notes::content(v, w, &Ctx { scale: s }));
    let speaks = column().gap(z(12.0)).w(z(w)).child(demo).child(rows);

    let setup = match c::secondary("Open setup", false, 32.0, true, act(|_, cx| crate::live_settings::open_setup(cx))) {
        Some((b, bw)) => (b, bw),
        None => {
            let (b, bw) = secondary_button("Open setup", None);
            (b.id("settings-open-setup").role(Role::Button).aria_label("Open setup").into_any_element(), bw)
        }
    };
    let login = c::toggle(v.open_at_login, "Open at login", s, act(|_, _| crate::login_item::set(!crate::login_item::enabled())));
    let mut mac = column().gap(z(12.0)).w(z(w))
        .child(setting_row("Open at login", Some("Start Iris in the menu bar when you log in."), login, 50.0, w));
    if let Some(p) = &v.login_problem {
        mac = mac.child(t(p.clone(), caption(), hue::WARN_TEXT));
    }
    let mac = mac
        .child(hairline(584, w))
        .child(setting_row("First-time setup", Some("Permission, model, and a practice box."), setup.0, setup.1, w));

    vec![
        section(Some(1), "The panel", Some("What you see when you press the shortcut."), None, Some(panel), w, s),
        section(Some(2), "When Iris speaks up", Some("After you press the keys, Iris tells you what happened in a small message by your text. It fades on its own. Errors always show."), None, Some(speaks), w, s),
        section(Some(3), "Your Mac", None, None, Some(mac), w, s),
    ]
}
