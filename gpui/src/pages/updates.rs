//! Settings › Updates (`UpdatesPage`, SettingsUpdates.swift): Your copy, one row saying which
//! version you have (or what the update check came back with, the letter from GitHub beside
//! it, `UpdateCheckScene` in its wide layout, drawn settled as the Swift snapshots draw it), and
//! What's new: one version's changes printed beside its zine cover, the other versions as
//! small covers under them. The changelog is the one that ships inside Iris (Changelog.md).
//!
//! Snapshot switches, as the Swift app has them: `IRIS_SAMPLE_CHANGELOG=1` reads the made-up
//! versions, `IRIS_UPDATE_CHECK=<state>` draws the check answered: idle, up-to-date, new,
//! release, failed, cant. `IRIS_UPDATES_START=<v>` opens another version's notes and
//! `IRIS_UPDATES_INSTALLED=<v>` says which one you have.
//!
//! The covers (masthead, version and title set in Martian at widths and weights GPUI has no cut
//! for, the big one turned 3°) and the letter scene (turned too) are printed whole.

use gpui::{Div, InteractiveElement, IntoElement, ParentElement, Role, StatefulInteractiveElement, Styled, div};
use tiny_skia::{FillRule, PathBuilder, Rect, Transform};

use crate::garden;
use crate::parts::*;
use crate::pen;
use crate::raster::{self, Sheet};
use crate::riso::{self, Plate, Rgb8, col, hue, ink};
use crate::settings::{Ctx, Values};
use crate::settings_kit::*;

/// The version this copy is (`Updater.version`: a build without one says "dev").
fn installed() -> String {
    crate::updater::version()
}

/// Where new versions are published (`Updater.repo`'s releases).
const RELEASES: &str = "https://github.com/imaxisXD/iris/releases/latest";

thread_local! {
    /// What the live check came back with, for the words and the letter (none in a snapshot:
    /// the made-up answers the Swift snapshots draw).
    static FACTS: std::cell::RefCell<Option<crate::updater::Status>> = const { std::cell::RefCell::new(None) };
    /// Where the live check stands, with its clocks, for the letter's scene.
    static PHASE: std::cell::RefCell<Option<crate::updater::Phase>> = const { std::cell::RefCell::new(None) };
}

/// Where the live check stands, as this page draws it.
pub fn from_phase(phase: &crate::updater::Phase) -> Check {
    use crate::updater::{Phase, Status};
    let (check, facts) = match phase {
        Phase::Idle => (Check::Idle, None),
        Phase::Looking(_) | Phase::Updating(_) => (Check::Looking, None),
        Phase::Answered(st, _) => (match st {
            Status::UpToDate => Check::UpToDate,
            Status::Behind(..) => Check::New,
            Status::NewRelease(..) => Check::Release,
            Status::Failed(_) | Status::InstallFailed(_) => Check::Failed,
            Status::NoCheckout | Status::NoRemote => Check::Cant,
        }, Some(st.clone())),
    };
    FACTS.with(|f| *f.borrow_mut() = facts);
    PHASE.with(|p| *p.borrow_mut() = Some(phase.clone()));
    check
}

fn facts() -> Option<crate::updater::Status> {
    FACTS.with(|f| f.borrow().clone())
}

/// The page's sections, `w` points wide.
pub fn content(v: &Values, w: f32, cx: &Ctx) -> Vec<Div> {
    let s = cx.scale;
    let check = v.update_check.unwrap_or_else(Check::from_env);
    let sample = std::env::var("IRIS_SAMPLE_CHANGELOG").is_ok_and(|v| v == "1");
    // Read once: the live page draws every frame it scrolls.
    static SHIPPED_ISSUES: std::sync::OnceLock<Vec<Issue>> = std::sync::OnceLock::new();
    static SAMPLE_ISSUES: std::sync::OnceLock<Vec<Issue>> = std::sync::OnceLock::new();
    let changelog = if sample { SAMPLE_ISSUES.get_or_init(|| parse(SAMPLE)) } else { SHIPPED_ISSUES.get_or_init(|| parse(SHIPPED)) };
    let installed = std::env::var("IRIS_UPDATES_INSTALLED").unwrap_or_else(|_| installed());
    // The version picked from the shelf, live; else the one the snapshot asks for.
    let start = crate::live_settings::MOMENT.with(|m| m.borrow().picked_version.clone()).or_else(|| std::env::var("IRIS_UPDATES_START").ok());
    // Both sections in one stack: the page puts 30 between what it's given, Updates has 34.
    vec![column().w(z(w)).gap(z(34.0))
        .child(section(Some(1), "Your copy", None, None, Some(your_copy(&check, w, s)), w, s))
        .child(section(Some(2), "What's new", None, None, Some(version_notes(changelog, &installed, start.as_deref(), w, s)), w, s))]
}

// MARK: - The changelog (IrisCore's `Changelog`)

const SHIPPED: &str = include_str!("../../../Sources/Iris/Resources/Changelog.md");

/// `SampleChangelog.text`: made-up versions for trying the shelf with many.
const SAMPLE: &str = "# 0.5.0 · Night shift
date: 2027-04-02
## Dark paper
Iris prints on dark paper when your Mac is in dark mode.
- Faster first fix after waking.

# 0.4.0 · Many hands
date: 2027-02-14
## Tones you share
Send a tone to a friend as a link.

# 0.3.0 · Small print
date: 2027-01-09
## Quick Fix in more apps
Works in Figma comments and Linear.
## Fewer surprises
Big rewrites always ask first.
- The flower sways less under Reduce Motion.

# 0.2.0 · Second print
date: 2026-12-01
## Faster fixes
Fix comes back sooner on long messages.
- Settings remembers its page.

# 0.1.0 · First print
date: 2026-11-07
## Fix and four tones
Fix corrects grammar and spelling and keeps your words.
## Quick Fix
Fixes the text box you're in and replaces it straight away.
- The flower in the menu bar shows what Iris is doing.
";

#[derive(Clone, Debug)]
struct Issue {
    version: String,
    title: String,
    date: Option<String>,
    chapters: Vec<(String, String)>,
    also: Vec<String>,
    /// 1 for the first version ever: the number on its cover.
    number: usize,
}

/// `ReleaseVersion.parts`: the numbers, without a leading v or anything after a dash.
fn version_parts(v: &str) -> Option<Vec<u64>> {
    let mut v = v.trim();
    if v.starts_with(['v', 'V']) {
        v = &v[1..];
    }
    let v = v.split('-').next().unwrap_or("");
    v.split('.').map(|n| n.parse().ok()).collect()
}

fn same(a: &str, b: &str) -> bool {
    matches!((version_parts(a), version_parts(b)), (Some(x), Some(y)) if x == y)
}

/// `Changelog.parse`, line for line.
fn parse(text: &str) -> Vec<Issue> {
    let mut issues: Vec<Issue> = Vec::new();
    let mut detail: Vec<String> = Vec::new();
    let mut in_chapter = false;
    let mut skipping = false;
    fn close(issues: &mut [Issue], detail: &mut Vec<String>, in_chapter: &mut bool) {
        if *in_chapter
            && let Some(last) = issues.last_mut()
            && let Some(ch) = last.chapters.last_mut()
        {
            ch.1 = detail.join(" ");
        }
        detail.clear();
        *in_chapter = false;
    }
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("## ") {
            close(&mut issues, &mut detail, &mut in_chapter);
            if issues.is_empty() || skipping {
                continue;
            }
            issues.last_mut().unwrap().chapters.push((rest.trim().to_string(), String::new()));
            in_chapter = true;
        } else if let Some(rest) = line.strip_prefix("# ") {
            close(&mut issues, &mut detail, &mut in_chapter);
            let head = rest.replace(" - ", " · ");
            let cut = head.find(['·', '—', '–', ':']).unwrap_or(head.len());
            let mut version = head[..cut].trim().to_string();
            if version.to_lowercase().starts_with('v') {
                version.remove(0);
            }
            let title = if cut < head.len() { head[cut..].chars().skip(1).collect::<String>().trim().to_string() } else { String::new() };
            skipping = version_parts(&version).is_none() || issues.iter().any(|i| same(&i.version, &version));
            if skipping {
                continue;
            }
            issues.push(Issue { version, title, date: None, chapters: vec![], also: vec![], number: 1 });
        } else if line.to_lowercase().starts_with("date:") {
            if issues.is_empty() || skipping {
                continue;
            }
            let d = line[5..].trim();
            issues.last_mut().unwrap().date = if d.is_empty() { None } else { Some(d.to_string()) };
        } else if let Some(rest) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            close(&mut issues, &mut detail, &mut in_chapter);
            if issues.is_empty() || skipping {
                continue;
            }
            issues.last_mut().unwrap().also.push(rest.trim().to_string());
        } else if line.is_empty() || line.starts_with("<!--") {
            continue;
        } else if in_chapter {
            detail.push(line.to_string());
        }
    }
    close(&mut issues, &mut detail, &mut in_chapter);
    let n = issues.len();
    for (i, issue) in issues.iter_mut().enumerate() {
        issue.number = n - i;
    }
    issues
}

/// "2026-11-17" as "17 Nov 2026".
fn day(iso: &str) -> String {
    let p: Vec<&str> = iso.split('-').collect();
    let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    match (p.first(), p.get(1).and_then(|m| m.parse::<usize>().ok()), p.get(2).and_then(|d| d.parse::<u32>().ok())) {
        (Some(y), Some(m), Some(d)) if (1..=12).contains(&m) && p.len() == 3 => format!("{d} {} {y}", months[m - 1]),
        _ => iso.to_string(),
    }
}

// MARK: - What's new (`VersionNotes`)

fn version_notes(issues: &[Issue], installed: &str, start: Option<&str>, w: f32, s: f32) -> Div {
    let Some(newest) = issues.first() else {
        return column().child(t("No notes for this version yet.", caption(), hue::I10));
    };
    let shown = start.and_then(|v| issues.iter().find(|i| same(&i.version, v))).unwrap_or(newest);
    let others: Vec<&Issue> = issues.iter().filter(|i| i.version != shown.version).collect();
    let cover_w = 118.0;
    let notes_w = w - cover_w - 28.0;
    let mut c = column().w(z(w)).gap(z(12.0))
        .child(row().gap(z(28.0)).items_start()
            .child(div().flex_none().pt(z(14.0)).child(issue_cover(shown, cover_w, same(&shown.version, installed), -3.0, s)))
            .child(issue_notes(shown, notes_w)));
    if !others.is_empty() {
        c = c.child(div().flex_none().pt(z(6.0)).child(hairline(901, w))).child(other_versions(&others, installed, s));
    }
    c
}

/// `IssueNotes`: the version's name in brush and its day, each headline change with its line,
/// then the small ones ticked off in pink.
fn issue_notes(issue: &Issue, w: f32) -> Div {
    let brush_box = line_box(26.0, BRUSH_LINE);
    let brush_base = (brush_box - 26.0 * BRUSH_LINE) / 2.0 + 26.0 * BRUSH_ASCENT;
    let cap_box = line_box(11.0, MONO_LINE);
    let cap_base = (cap_box - 11.0 * MONO_LINE) / 2.0 + 11.0;
    let mut head = row().gap(z(10.0)).items_start().child(t(issue.title.clone(), brush(26.0), hue::I12));
    if let Some(d) = &issue.date {
        head = head.child(t(day(d), caption(), hue::I10).mt(z(brush_base - cap_base)));
    }
    let mut c = column().w(z(w)).gap(z(12.0))
        .child(head.id(gpui::SharedString::from(format!("settings-issue-{}", issue.version))).role(Role::Heading).aria_label(issue.title.clone()));
    for (title, detail) in &issue.chapters {
        let mut ch = column().gap(z(3.0)).child(para_out(title, body(), hue::I12, w));
        if !detail.is_empty() {
            ch = ch.child(para_floor(detail, caption(), hue::I10, w));
        }
        c = c.child(ch);
    }
    for line in &issue.also {
        c = c.child(row().gap(z(8.0)).items_start()
            .child(sized(12.0, 9.0).mt(z(cap_base - 9.0)).child(drawn(12.0, 9.0, pen::hand_tick(12.0, 9.0), Some(1.8), col(ink::PINK_TEXT))))
            .child(para_floor(line, caption(), hue::I11, w - 20.0)));
    }
    c
}

/// `OtherVersions`: small covers in rows of eight, each with its number under it.
fn other_versions(issues: &[&Issue], installed: &str, s: f32) -> Div {
    let micro = martian(9.5, 450);
    let label = column().pt(z(4.0))
        .child(t("OTHER", micro.tracked(1.0), hue::I10))
        .child(t("VERSIONS", micro.tracked(1.0), hue::I10));
    let mut rows = column().gap(z(14.0));
    for chunk in issues.chunks(8) {
        let mut r = row().gap(z(14.0)).items_start();
        for issue in chunk {
            let yours = same(&issue.version, installed);
            r = r.child(cover_button(issue, yours, micro, s));
        }
        rows = rows.child(r);
    }
    row().gap(z(18.0)).items_start().child(label).child(rows)
}

/// `CoverButton`: a small cover to pick; it lifts a little under the pointer, like taking it off
/// the pile (3 points up and turned 3°, on `DS.easeOut`); a click shows what changed in it.
fn cover_button(issue: &Issue, yours: bool, micro: Ts, s: f32) -> gpui::AnyElement {
    let l = crate::interact::get();
    let id = format!("cover-{}", issue.version);
    let hover = l.as_ref().map_or(0.0, |l| l.hover(&id));
    // The turn is printed whole, so it comes in at once, halfway through the lift.
    let degrees = if hover > 0.5 { -3.0 } else { 0.0 };
    let el = column().gap(z(6.0))
        .child(div().flex_none().relative().top(z(-3.0 * hover)).child(issue_cover(issue, 44.0, yours, degrees, s)))
        .child(t(issue.version.clone(), micro, hue::I11))
        .id(gpui::SharedString::from(format!("settings-version-{}", issue.version))).role(Role::Button)
        .aria_label(format!("Version {}, {}{}", issue.version, issue.title, if yours { ", the one you have" } else { "" }));
    match l {
        Some(l) if l.view.is_some() => {
            let v = issue.version.clone();
            el.cursor_pointer().on_hover(l.hover_handler(&id, None)).on_click(move |_, window, _| {
                crate::live_settings::MOMENT.with(|m| m.borrow_mut().picked_version = Some(v.clone()));
                window.refresh();
            }).into_any_element()
        }
        _ => el.into_any_element(),
    }
}

/// `IssueCover`: a zine cover printed in one riso ink: the masthead, the issue number, the
/// version big and the title in brush, under a pen edge, with a deeper spine down its left;
/// turned `degrees`, printed whole.
fn issue_cover(issue: &Issue, width: f32, yours: bool, degrees: f32, scale: f32) -> Div {
    let h = width * 330.0 / 244.0;
    // Room round it for the turn and the tape across its top.
    let pad = if yours { 26.0 } else { (width.max(h) * 0.06).ceil() + 2.0 };
    let issue = issue.clone();
    turned(format!("settings-cover-{}-{}-{width}-{yours}-{degrees}", issue.number, issue.title), width, h, pad, degrees, scale, move |sh| {
        cover_into(sh, &issue, width, yours, scale);
    })
}

const COVER_INKS: [Rgb8; 4] = [(236, 110, 176), (120, 162, 220), (196, 157, 122), (226, 200, 185)];

/// A rectangle as a path.
fn rect(x: f32, y: f32, w: f32, h: f32) -> tiny_skia::Path {
    PathBuilder::from_rect(Rect::from_xywh(x, y, w, h).unwrap())
}

/// The cover drawn into `sh`, its top left at the sheet's origin.
fn cover_into(sh: &mut Sheet, issue: &Issue, width: f32, yours: bool, scale: f32) {
    let ink_c = COVER_INKS[(issue.number + 399) % 4];
    let s = width / 98.0;
    let h = width * 330.0 / 244.0;
    let n = issue.number as i64;
    let inner = width - 18.0 * s;
    let id = Transform::identity();

    // The paper, printed in its ink.
    let plate = Plate { w: width, h, scale };
    let mut m = plate.mask();
    m.fill_path(&rect(0.0, 0.0, width, h), FillRule::Winding, false, plate.transform());
    sh.picture(&riso::print(&plate, ink_c, n * 13, &m, 1.0), width, h, 0.0, 0.0);

    // The masthead: "IRIS" tracked wide, the number at the right (Martian 600, printed).
    let ms = 9.0 * s;
    let mline = ms * MONO_LINE;
    let (mx, my) = (9.0 * s, 9.0 * s);
    let base = my + mono_base(ms, mline);
    if let Some(p) = martian_path("IRIS", ms, 600.0, 87.5, 1.6 * s, mx, base) {
        sh.fill_at(&p, id, hue::I12, 1.0);
    }
    let no = format!("No.{:02}", issue.number);
    let no_w = martian_w(&no, ms, 600.0, 87.5, 0.0);
    if let Some(p) = martian_path(&no, ms, 600.0, 87.5, 0.0, mx + inner - no_w, base) {
        sh.fill_at(&p, id, hue::I12, 1.0);
    }
    // The pen line under it.
    let rule_y = my + mline + 2.0 * s;
    let line_c = riso_over(hue::I12, 0.7, ink_c);
    sh.stroke_at(&pen::hand_line(n * 5 + 1, 0.5, 0.0, 1.5, inner).skia().unwrap(), Transform::from_translate(mx, rule_y), 1.2 * s, line_c);

    // From the foot up: the title in brush, then the version big (as big as fits,
    // `minimumScaleFactor(0.6)`), in Martian at width 75.
    let size = 30.0 * s;
    let natural = issue.version.chars().count() as f32 * (mono_adv(size, 400.0, 75.0) - 0.8 * s);
    let fit = (inner / natural).clamp(0.6, 1.0);
    let vsize = size * fit;
    let tsize = 17.0 * s;
    let tnatural = tsize * BRUSH_LINE;
    // `lineSpacing(-4 × s)`: the lines on whole points, the first at its own height.
    let pitch = tnatural;
    // SwiftUI breaks these lines a little short of the frame.
    let title_lines = brush_lines(&issue.title, tsize, inner - 4.4 * s, 2);
    let title_h = tnatural + pitch * (title_lines.len() as f32 - 1.0);
    let title_y = h - 9.0 * s - title_h;
    let vbox = line_box(vsize, MONO_LINE);
    let vy = title_y - vbox;
    if let Some(p) = martian_path(&issue.version, vsize, 400.0, 75.0, -0.8 * s * fit, mx, vy + mono_base(vsize, vsize * MONO_LINE)) {
        sh.fill_at(&p, id, hue::I12, 1.0);
    }
    for (i, l) in title_lines.iter().enumerate() {
        let top = title_y + (tnatural - pitch) / 2.0 + i as f32 * pitch;
        let b = top + (pitch - tsize * BRUSH_LINE) / 2.0 + tsize * BRUSH_ASCENT;
        if let Some(p) = raster::text_path(CAVEAT, tsize, l, mx, b, 0.0) {
            sh.fill_at(&p, id, hue::I12, 1.0);
        }
    }

    // The pen edge, the deeper spine, and the fold line down it.
    sh.stroke_at(&pen::hand_rect(n * 7 + 2, 0.5 * s, 2.5, false, 0.0, 0.0, width, h).skia().unwrap(), id, 1.4 * s, hue::I12);
    let spine = Plate { w: 9.0 * s, h, scale };
    let mut sm = spine.mask();
    sm.fill_path(&rect(0.0, 0.0, 9.0 * s, h), FillRule::Winding, false, spine.transform());
    sh.picture(&riso::print(&spine, ink::INK, n * 3, &sm, 0.16), 9.0 * s, h, 0.0, 0.0);
    sh.stroke_at(&pen::hand_line_vertical(n * 11, 0.4 * s, 0.5, 0.0, h).skia().unwrap(), Transform::from_translate(9.0 * s, 0.0), 1.0 * s, riso_over(hue::I12, 0.55, ink_c));

    // The one you have wears a strip of tape across the top: the big cover's off to the right
    // and higher, a small one's centred.
    if yours {
        let big = width > 60.0;
        let tsz = if big { 9.0 * s } else { 8.0 };
        let pad_x = tsz * 0.62 + 4.0;
        let pad_y = tsz * 0.26;
        let (tw, th) = (mono_w("yours", tsz, 0.0) + 2.0 * pad_x, tsz * MONO_LINE + 2.0 * pad_y);
        let (dx, dy) = if big { (12.0 * s, -15.0 * s) } else { (0.0, -9.0) };
        let at_ = Transform::from_translate((width - tw) / 2.0 + dx, dy)
            .pre_concat(Transform::from_rotate_at(-7.0, tw / 2.0, th / 2.0))
            .pre_concat(Transform::from_rotate_at(-1.5, tw / 2.0, th / 2.0));
        sh.fill_at(&pen::tape(tw, th).skia().unwrap(), at_, riso::multiply(hue::TAPE, ink::PAPER), 1.0);
        let ts = martian(tsz, 430);
        if let Some(p) = raster::text_path(ts.bytes, tsz, "yours", pad_x, pad_y + mono_base(tsz, tsz * MONO_LINE), 0.0) {
            sh.fill_at(&p, at_, hue::I12, 1.0);
        }
    }
}

/// `c` at `alpha` over `under`, as bytes.
fn riso_over(c: Rgb8, alpha: f32, under: Rgb8) -> Rgb8 {
    let m = |a: u8, b: u8| (a as f32 * alpha + b as f32 * (1.0 - alpha)).round() as u8;
    (m(c.0, under.0), m(c.1, under.1), m(c.2, under.2))
}

/// Brush words broken into at most `max` lines as SwiftUI breaks them: a word goes down when
/// its ink would pass `w`.
fn brush_lines(s: &str, size: f32, w: f32, max: usize) -> Vec<String> {
    let mut ls: Vec<String> = Vec::new();
    for word in s.split(' ') {
        let full = ls.len() >= max;
        match ls.last_mut() {
            Some(last) if brush_w(&format!("{last} {word}"), size) <= w || full => {
                last.push(' ');
                last.push_str(word);
            }
            _ => ls.push(word.to_string()),
        }
    }
    // Apple's line breaking pushes a word down rather than leave one alone on the last line,
    // when the line it leaves still holds most of the width.
    let n = ls.len();
    if n >= 2 && !ls[n - 1].contains(' ') && ls[n - 2].contains(' ') {
        let (keep, moved) = ls[n - 2].rsplit_once(' ').map(|(a, b)| (a.to_string(), b.to_string())).unwrap();
        let last = format!("{moved} {}", ls[n - 1]);
        if brush_w(&keep, size) >= w * 0.5 && brush_w(&last, size) <= w {
            ls[n - 2] = keep;
            ls[n - 1] = last;
        }
    }
    ls
}

// MARK: - Your copy (`UpdateStatus`)

/// Where the update check stands, as the Swift snapshots preview it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Check {
    Idle,
    /// Looking on GitHub: the button waits, the words say so.
    Looking,
    UpToDate,
    New,
    Release,
    Failed,
    Cant,
}

impl Check {
    fn from_env() -> Check {
        match std::env::var("IRIS_UPDATE_CHECK").as_deref() {
            Ok("up-to-date") => Check::UpToDate,
            Ok("new") => Check::New,
            Ok("release") => Check::Release,
            Ok("failed") => Check::Failed,
            Ok("cant") => Check::Cant,
            _ => Check::Idle,
        }
    }

    /// (title, lead, warning): `UpdateCheck.Words`.
    fn words(self) -> (String, String, bool) {
        if let (Some(st), Check::UpToDate | Check::New | Check::Release | Check::Failed | Check::Cant) = (facts(), self) {
            return crate::updater::words(&st);
        }
        match self {
            Check::Idle => (format!("Iris {}", installed()), crate::words::w("The version on this Mac.").to_string(), false),
            Check::Looking => ("Looking…".into(), "Looking for a new Iris…".into(), false),
            Check::UpToDate => ("Up to date".into(), "You have the latest Iris.".into(), false),
            Check::New => ("2 new changes".into(), "Ready to install.".into(), false),
            Check::Release => ("Iris 0.2.0 is out".into(), "Download it, then replace Iris in Applications. Your settings stay.".into(), false),
            Check::Failed => ("Couldn't check".into(), "Couldn't reach GitHub. Check your connection and try again.".into(), true),
            Check::Cant => ("Can't update".into(), "This copy of Iris wasn't installed from its project folder, so it can't update itself.".into(), true),
        }
    }

    /// What the butterfly's letter says: Iris's line, the facts, and whether it's ticked.
    fn letter(self) -> Option<(&'static str, Vec<(&'static str, String)>, bool)> {
        let yours = ("yours", installed());
        let (count, tag) = match facts() {
            Some(crate::updater::Status::Behind(n, _)) => (n, "0.2.0".to_string()),
            Some(crate::updater::Status::NewRelease(t, _, _)) => (2, t.trim_start_matches('v').to_string()),
            _ => (2, "0.2.0".to_string()),
        };
        match self {
            Check::UpToDate => Some(("nothing newer yet", vec![("newest", installed()), yours], true)),
            Check::New => Some(("something new for you", vec![yours, ("new", if count == 1 { "1 change".into() } else { format!("{count} changes") })], false)),
            Check::Release => Some(("fresh off the press", vec![("newest", tag), yours], false)),
            _ => None,
        }
    }
}

fn your_copy(check: &Check, w: f32, s: f32) -> Div {
    let live = crate::interact::get().filter(|l| l.view.is_some());
    let moment = live.as_ref().and_then(|l| PHASE.with(|p| p.borrow().as_ref().and_then(|p| crate::page::Moment::of(p, l.reduce_motion))));
    // Live, the answer waits for the letter (`UpdateCheckScene.Beat`): the title is written in
    // 0.9 s after it, the line under it 1.15 s, the button 1.25 s.
    let before = |beat: f64| moment.and_then(|m| m.a).is_some_and(|a| a < beat);
    let (title, lead, warning) = check.words();
    let (looking_title, looking_lead, _) = Check::Looking.words();
    let (title, warning) = if before(0.9) { (looking_title, false) } else { (title, warning) };
    let lead = if before(1.15) { looking_lead } else { lead };
    let button_check = if before(1.25) { Check::Looking } else { *check };
    // The answer's button once the letter has opened; none when there's nothing to press.
    use crate::controls::{self as c, Button};
    let ask = crate::interact::act(|window, cx| {
        crate::updater::start(cx);
        window.refresh();
    });
    let button: Option<(gpui::AnyElement, f32)> = match &button_check {
        Check::Idle => Some(c::button(&Button::primary("Check for updates").id("update-check"), s, ask)),
        // While it looks, the button waits in place (`TactileButton(loading:)`).
        Check::Looking => Some(c::button(&Button::primary("Looking…").loading(true).enabled(false).reserve("Check for updates").id("update-check"), s, ask)),
        // A downloaded copy installs the release itself when it can; else its page.
        Check::Release => match facts() {
            Some(crate::updater::Status::NewRelease(tag, _, Some(zip))) => Some(c::button(&Button::primary("Update & reopen").id("update-install"), s,
                crate::interact::act(move |_, cx| crate::updater::install_release(tag.clone(), zip.clone(), cx)))),
            other => {
                let page = match other {
                    Some(crate::updater::Status::NewRelease(_, page, None)) => page,
                    _ => RELEASES.to_string(),
                };
                Some(c::button(&Button::primary("Download").id("update-download"), s, crate::interact::act(move |_, _| c::open_url(&page))))
            }
        },
        Check::UpToDate => Some(c::button(&Button::secondary("Check again").id("update-again"), s, ask)),
        Check::Failed => Some(c::button(&Button::secondary("Try again").id("update-retry"), s, ask)),
        Check::New | Check::Cant => None,
    };
    let mut actions = row().gap(z(12.0)).items_center();
    let mut actions_w = 0.0;
    // Live, the scene plays from the check's clocks (the bud grows on the click, the
    // butterfly flies off and comes back with the letter); else its settled moment.
    match (&live, moment) {
        (Some(l), Some(m)) if *check != Check::Idle => {
            actions = actions.child(wide_scene(*check, l, &m, s));
            actions_w += SCENE.0;
        }
        _ if !matches!(check, Check::Idle | Check::Looking) => {
            actions = actions.child(sized(SCENE.0, SCENE.1).child(at(scene(check, s), 0.0, 0.0)));
            actions_w += SCENE.0;
        }
        _ => {}
    }
    match button {
        Some((b, bw)) => {
            actions = actions.child(b);
            actions_w += bw + if actions_w > 0.0 { 12.0 } else { 0.0 };
        }
        // No button once the letter has opened: its empty slot still takes the stack's spacing.
        None => {
            actions = actions.child(div().flex_none().w(z(0.0)));
            actions_w += 12.0;
        }
    }
    let title_ink = if warning { hue::RED_TEXT } else { hue::I12 };
    let spoken = format!("{title}. {lead}");
    let words_w = w - actions_w - 16.0;
    let words = column().gap(z(3.0)).w(z(words_w))
        .child(t(title, body(), title_ink))
        .child(para_floor(&lead, caption(), hue::I10, words_w));
    // Heard as Swift's Text is: the answer's title and lead.
    let words = words.id("update-answer").role(Role::Label).aria_label(spoken);
    let the_row = row().w(z(w)).gap(z(16.0)).items_center().child(words).child(actions);
    let mut c = column().w(z(w)).gap(z(12.0)).child(the_row);
    if *check == Check::New && !before(1.15) {
        // The section's content is padded 2 on top, each of its views (the row and this).
        c = c.child(column().w(z(w)).gap(z(12.0)).pt(z(2.0)).child(hairline(311, w))
            .child(update_changes(&match facts() {
                Some(crate::updater::Status::Behind(_, subjects)) => subjects,
                _ => vec!["The menu bar flower in pen".into(), "Names each fix in the margin".into()],
            }, w, s)));
    }
    c
}

/// `UpdateChanges`: each change ticked off in pink, what happens next, and the button.
fn update_changes(lines_: &[String], w: f32, s: f32) -> Div {
    let base = 12.5 + (line_box(12.5, MONO_LINE) - 12.5 * MONO_LINE) / 2.0;
    let mut list = column().gap(z(8.0));
    for line in lines_.iter().take(5) {
        let mut ls = push_out(line, w - 20.0, 12.5);
        ls.truncate(2);
        list = list.child(row().gap(z(8.0)).items_start()
            .child(sized(12.0, 9.0).mt(z(base - 9.0)).child(drawn(12.0, 9.0, pen::hand_tick(12.0, 9.0), Some(1.8), col(ink::PINK_TEXT))))
            .child(lines(&ls, body(), hue::I11, body().line)));
    }
    use crate::controls::{self as c, Button};
    let (button, _) = c::button(&Button::primary("Update & reopen").compact().id("update-install"), s, crate::interact::act(|_, cx| crate::updater::install(cx)));
    column().gap(z(10.0))
        .child(list)
        .child(div().flex_none().py(z(2.0)).child(pen_rule(61, w, crate::riso::over(hue::I12, 0.3, ink::PAPER), 1.0)))
        .child(para_out("Iris installs it, then quits and reopens. About a minute.", caption(), hue::I10, w))
        .child(div().flex_none().pt(z(2.0)).child(button))
}

// MARK: - The letter from GitHub (`UpdateCheckScene`, wide, settled)

/// The settled moment: 2 s after the answer (the Settings snapshots answer 2 s before they draw).
const A: f64 = 2.0;
/// The storyboard's clock (`Date(timeIntervalSince1970: 4.8)` as reference time), for the
/// wings' slow beat.
const NOW_REF: f64 = 4.8 - 978_307_200.0;

const SCENE: (f32, f32) = (222.0, 100.0);
const ROOT: (f32, f32) = (188.0, 100.0);
const BUD: f32 = 50.0;
const LETTER_AT: (f32, f32) = (78.0, 54.0);
const LETTER: (f32, f32) = (156.0, 80.0);
const SKY: (f32, f32, f32, f32) = (116.0, 6.0, 134.0, 100.0);

fn seg(t: f64, start: f64, length: f64) -> f64 {
    if length <= 0.0 { if t >= start { 1.0 } else { 0.0 } } else { ((t - start) / length).clamp(0.0, 1.0) }
}
fn ease_out(x: f64) -> f64 {
    1.0 - (1.0 - x).powi(3)
}
fn ease_in_out(x: f64) -> f64 {
    if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
}

/// The live scene (`UpdateCheckScene`, wide) at moment `m`.
fn wide_scene(check: Check, live: &crate::interact::Live, m: &crate::page::Moment, scale: f32) -> Div {
    let facts = check.letter().map(|(_, rows, _)| rows.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>().join("|")).unwrap_or_default();
    let sky_ = move |clears: f64, sun_up: f64, drift: f64| sky(clears, sun_up, drift, scale);
    let bottom = move |sh: &mut Sheet, at_: Transform| letter_bottom_into(sh, at_, check, scale, false);
    let top = move |sh: &mut Sheet, at_: Transform| letter_top_into(sh, at_, check, scale);
    let tick = check.letter().is_some_and(|l| l.2).then(tick_place);
    let ring = move |sh: &mut Sheet, at_: Transform, drawn: f64, q: f64| ring_drawn(sh, at_.pre_translate(LETTER_AT.0 + 40.0 - 18.0, LETTER_AT.1 - 4.0 - 18.0), 36.0, drawn, q, scale);
    let art = crate::page::Art { key: format!("wide-{check:?}-{facts}"), sky: &sky_, bottom: &bottom, top: &top, tick, ring: &ring };
    crate::page::scene_element(&crate::page::WIDE, m, &art, live, scale)
}

/// The whole scene printed once (the letter and the ring are turned): its picture spills past
/// the scene's frame (the sky reaches past its right edge), so it's `SCENE` plus room round it.
fn scene(check: &Check, scale: f32) -> Div {
    let check = *check;
    // The letter's facts (how many changes, which version) are part of the picture.
    let facts = check.letter().map(|(_, rows, _)| rows.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>().join("|")).unwrap_or_default();
    turned(format!("settings-letter-scene-{check:?}-{facts}"), SCENE.0, SCENE.1, 30.0, 0.0, scale, move |sh| scene_into(sh, check, scale))
}

fn scene_into(sh: &mut Sheet, check: Check, scale: f32) {
    let failed = matches!(check, Check::Failed | Check::Cant);
    let open = if failed { 0.0 } else { ease_out(seg(A, 1.15, 0.45)) };
    let clears = if failed { 0.0 } else { ease_in_out(seg(A, 1.15 - 0.35, 0.9)) };
    let sun_up = if failed { 0.0 } else { ease_out(seg(A, 1.15, 0.8)) };
    let drift = (NOW_REF * 0.7).sin() * 3.0;
    let resting_wings = 0.45 + 0.2 * (NOW_REF * 2.4).sin().max(0.0);
    let perch = (ROOT.0 + 4.0, ROOT.1 - BUD - open as f32 * 4.0);
    let id = Transform::identity();

    let sky_rgba = sky(clears, sun_up, drift, scale);
    sh.picture(&sky_rgba, SKY.2, SKY.3, SKY.0, SKY.1);

    // The flower: the bud, or the bloom it opened into.
    let art = garden::flower_raw(open >= 1.0, scale);
    sh.picture_at(&art.1, art.0, art.0, BUD, BUD, id.pre_translate(ROOT.0 - BUD / 2.0, ROOT.1 - BUD), 1.0);

    if failed {
        ring_into(sh, id.pre_translate(LETTER_AT.0 + 40.0 - 18.0, LETTER_AT.1 - 4.0 - 18.0), scale);
    } else {
        let (lw, lh) = LETTER;
        let lean = id.pre_translate(LETTER_AT.0 - lw / 2.0, LETTER_AT.1 - lh / 2.0).pre_concat(Transform::from_rotate_at(-3.0, lw / 2.0, lh / 2.0));
        letter_into(sh, lean, check, scale);
    }

    let b = 22.0f32;
    let wings = garden::butterfly_raw();
    let at_ = id.pre_translate(perch.0 - b / 2.0 + b * (1.0 - resting_wings as f32) / 2.0, perch.1 - b / 2.0);
    sh.sprite_at(&wings.1, wings.0, wings.0, b * resting_wings as f32, b, at_, 1.0);
}

/// The patch of sky round the flower (`ScreenPlate(.sky, seed: 23)`), the sun up behind the
/// bloom, the cloud gone, thinning into the paper from its middle.
fn sky(clears: f64, sun_up: f64, drift: f64, scale: f32) -> Vec<u8> {
    let (w, h) = (SKY.2, SKY.3);
    let plate = Plate { w, h, scale };
    let (pw, ph) = plate.px();
    let dots = riso::screen(w as usize, h as usize, (120, 162, 220), 23, 1.0, 1.0);
    let mut out = vec![0u8; (pw * ph * 4) as usize];
    for y in 0..ph {
        for x in 0..pw {
            let c = dots[((y as f32 / scale) as usize).min(h as usize - 1) * w as usize + ((x as f32 / scale) as usize).min(w as usize - 1)];
            let o = ((y * pw + x) * 4) as usize;
            out[o..o + 4].copy_from_slice(&[c.0, c.1, c.2, 255]);
        }
    }
    let sun = (w.min(h) * 0.26) as f64;
    stamp(&mut out, pw, ph, scale, &garden::sun_rgba(), 44, (w as f64 * 0.42, h as f64 * 0.66 - sun_up * h as f64 * 0.32), sun, if sun_up > 0.0 { 1.0 } else { 0.0 });
    let cloud = (w.min(h) * 0.5) as f64;
    stamp(&mut out, pw, ph, scale, &garden::mist_cloud_rgba(), 44, (w as f64 * 0.47 + drift + clears * w as f64 * 0.35, h as f64 * 0.32), cloud, 1.0 - clears);
    let stops = [(0.0, 1.0), (0.3, 0.8), (0.62, 0.4), (0.85, 0.12), (1.0, 0.0)];
    for y in 0..ph {
        for x in 0..pw {
            let (px, py) = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
            let d = (((px - 0.54 * w) / (0.5 * w)).powi(2) + ((py - 0.5 * h) / (0.5 * h)).powi(2)).sqrt();
            let mut a = 0.0f32;
            for k in 0..stops.len() - 1 {
                let (t0, a0) = stops[k];
                let (t1, a1) = stops[k + 1];
                if d >= t0 && d <= t1 {
                    a = a0 + (a1 - a0) * (d - t0) / (t1 - t0);
                }
            }
            let o = ((y * pw + x) * 4 + 3) as usize;
            out[o] = (out[o] as f32 * a).round() as u8;
        }
    }
    out
}

/// A sprite of `px` pixels laid at `centre` (points), `size` points square, without smoothing.
#[allow(clippy::too_many_arguments)]
fn stamp(out: &mut [u8], pw: u32, ph: u32, scale: f32, rgba: &[u8], px: u32, centre: (f64, f64), size: f64, opacity: f64) {
    if opacity <= 0.0 {
        return;
    }
    let x0 = ((centre.0 - size / 2.0) * scale as f64) as i64;
    let y0 = ((centre.1 - size / 2.0) * scale as f64) as i64;
    let n = (size * scale as f64) as i64;
    for dy in 0..n {
        for dx in 0..n {
            let (x, y) = (x0 + dx, y0 + dy);
            if x < 0 || y < 0 || x >= pw as i64 || y >= ph as i64 { continue }
            let sx = (dx * px as i64 / n) as usize;
            let sy = (dy * px as i64 / n) as usize;
            let s = &rgba[(sy * px as usize + sx) * 4..][..4];
            let a = s[3] as f64 / 255.0 * opacity;
            if a <= 0.0 { continue }
            let o = ((y as u32 * pw + x as u32) * 4) as usize;
            for k in 0..3 {
                out[o + k] = (s[k] as f64 * a + out[o + k] as f64 * (1.0 - a)).round() as u8;
            }
        }
    }
}

/// Today as "17 NOV" (`d MMM`, en_GB, upper-cased), from the clock.
fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let z_ = secs.div_euclid(86_400) + 719_468;
    let era = z_.div_euclid(146_097);
    let doe = z_ - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let months = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
    format!("{d} {}", months[(m - 1) as usize])
}

/// The opened letter (`LetterView`, wide), drawn through `at`: who it's from and Iris's line on
/// the top half, the facts and the tick on the bottom.
fn letter_into(sh: &mut Sheet, at_: Transform, check: Check, scale: f32) {
    if check.letter().is_none() {
        return;
    }
    let half = LETTER.1 / 2.0;
    letter_bottom_into(sh, at_.pre_translate(0.0, half), check, scale, true);
    letter_top_into(sh, at_.pre_translate(0.0, 0.0), check, scale);
}

/// A half of the letter's paper with its crema shadow printed off register, through `t_`.
fn half_sheet(sh: &mut Sheet, t_: Transform, seed: i64, scale: f32) {
    let (w, h) = LETTER;
    let half = h / 2.0;
    let plate = Plate { w, h: half, scale };
    let mask = plate.fill(&pen::hand_rect(seed, 0.35, 0.0, true, 0.0, 0.0, w, half).skia().unwrap());
    let crema = riso::print(&plate, ink::CREMA, seed, &mask, 1.0);
    let (pw, ph) = plate.px();
    sh.picture_at(&crema, pw, ph, w, half, t_.pre_translate(1.5, 1.5), 1.0);
    sh.fill_at(&pen::hand_rect(seed, 0.35, 0.0, true, 0.0, 0.0, w, half).skia().unwrap(), t_, ink::PAPER, 1.0);
}

fn half_outline(sh: &mut Sheet, t_: Transform, seed: i64) {
    let (w, h) = LETTER;
    sh.stroke_at(&pen::hand_rect(seed + 9, 0.35, 2.5, false, 0.0, 0.0, w, h / 2.0).skia().unwrap(), t_, 1.1, hue::I12);
}

/// Where the tick sits on the bottom half (x, y, w, h).
fn tick_place() -> (f32, f32, f32, f32) {
    (LETTER.0 - 10.0 - 20.0, (LETTER.1 / 2.0 - 15.0) / 2.0, 20.0, 15.0)
}

/// The bottom half through `bottom` (its own top left): the facts, and the tick when
/// `with_tick` (the live scene draws it in itself).
fn letter_bottom_into(sh: &mut Sheet, bottom: Transform, check: Check, scale: f32, with_tick: bool) {
    let half = LETTER.1 / 2.0;
    let Some((_, rows, ticked)) = check.letter() else { return };
    half_sheet(sh, bottom.pre_translate(0.0, 0.0), 61, scale);
    let fs = martian(9.5, 450);
    let fh = rows.len() as f32 * fs.line + (rows.len() as f32 - 1.0) * 2.0;
    let mut y = (half - fh) / 2.0;
    for (name, value) in &rows {
        let b = y + mono_base(9.5, fs.line);
        if let Some(p) = raster::text_path(fs.bytes, 9.5, name, 10.0, b, 0.0) {
            sh.fill_at(&p, bottom, hue::I10, 1.0);
        }
        if let Some(p) = raster::text_path(fs.bytes, 9.5, value, 60.0, b, 0.0) {
            sh.fill_at(&p, bottom, hue::I12, 1.0);
        }
        y += fs.line + 2.0;
    }
    if ticked && with_tick {
        let (tx, ty, tw, th) = tick_place();
        sh.stroke_at(&pen::hand_tick(tw, th).skia().unwrap(), bottom.pre_translate(tx, ty), 2.4, ink::PINK_TEXT);
    }
    half_outline(sh, bottom.pre_translate(0.0, 0.0), 61);
}

/// The top half through `at_`: who it's from, and Iris's line in brush.
fn letter_top_into(sh: &mut Sheet, at_: Transform, check: Check, scale: f32) {
    let (w, h) = LETTER;
    let half = h / 2.0;
    let Some((line, _, _)) = check.letter() else { return };
    half_sheet(sh, at_.pre_translate(0.0, 0.0), 62, scale);
    let from = martian(8.5, 450);
    if let Some(p) = raster::text_path(from.bytes, 8.5, &format!("FROM GITHUB · {}", today()), 10.0, 6.0 + mono_base(8.5, from.line), 0.5) {
        sh.fill_at(&p, at_, hue::I10, 1.0);
    }
    let size = 17.0 * ((w - 20.0) / brush_w(line, 17.0)).clamp(0.8, 1.0);
    let lh = line_box(size, BRUSH_LINE);
    let top = 6.0 + from.line + 1.0;
    if let Some(p) = raster::text_path(CAVEAT, size, line, 10.0, top + (lh - size * BRUSH_LINE) / 2.0 + size * BRUSH_ASCENT, 0.0) {
        sh.fill_at(&p, at_, hue::I12, 1.0);
    }
    let i8: Rgb8 = (0xA3, 0x95, 0x8B);
    sh.stroke_at(&pen::hand_line(64, 0.4, 0.0, 0.5, w - 8.0).skia().unwrap(), at_.pre_translate(4.0, half - 1.0), 0.8, i8);
    half_outline(sh, at_.pre_translate(0.0, 0.0), 62);
}

/// Back empty-handed: a red brush ring round "?" (`BrushRing(rgb: Riso.red, seed: 19)`), turned
/// 6°, drawn through `at` into its 36-point frame.
fn ring_into(sh: &mut Sheet, at_: Transform, scale: f32) {
    ring_drawn(sh, at_, 36.0, 1.0, 1.0, scale);
}

/// The ring `d` points across, drawn `drawn` of the way round, the "?" at `q` opacity.
pub fn ring_drawn(sh: &mut Sheet, at_: Transform, d: f32, drawn: f64, q: f64, scale: f32) {
    let weight = (d * 0.062).max(2.2);
    let b = (d + 2.0 * weight) / 0.9;
    let turn = at_.pre_concat(Transform::from_rotate_at(-6.0, d / 2.0, d / 2.0));
    let plate = Plate { w: b, h: b, scale };
    let mut points = loop_line(19, b, b, weight);
    if drawn < 1.0 {
        points.truncate(((points.len() as f64 * drawn).round() as usize).max(0));
    }
    if points.len() >= 2
        && let Some(path) = ribbon(&points, weight, 0.8, 0.5, true).skia()
    {
        let mask = plate.fill(&path);
        let red = riso::print(&plate, ink::RED, 19 + 3, &mask, 1.0);
        let (pw, ph) = plate.px();
        sh.picture_at(&red, pw, ph, b, b, turn.pre_translate((d - b) / 2.0, (d - b) / 2.0), 1.0);
    }
    if q <= 0.0 {
        return;
    }
    let qs = d * 0.62;
    let (lead, qw) = brush_box("?", qs);
    let lh = line_box(qs, BRUSH_LINE);
    if let Some(p) = raster::text_path(CAVEAT, qs, "?", (d - qw) / 2.0 + lead, (d - lh) / 2.0 + (lh - qs * BRUSH_LINE) / 2.0 + qs * BRUSH_ASCENT, 0.0) {
        sh.fill_at(&p, turn, hue::RED_TEXT, q as f32);
    }
}
