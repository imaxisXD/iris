//! What the panel shows, without any drawing: the word diff (`WordDiff`), the footer's rules
//! (`PanelFooter`) and the made-up states the Swift app draws for its design check
//! (`PanelStates`), ported from IrisCore so both apps draw the same states from the same rules.

/// A piece of the answer against the draft.
#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Same(String),
    Removed(String),
    Added(String),
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `WordDiff.tokenize`: whitespace runs, words (keeping apostrophes: "it's", "don’t"),
/// `{{n}}` placeholders, and single punctuation marks.
pub fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
        } else if is_word(c) {
            loop {
                while i < chars.len() && is_word(chars[i]) {
                    i += 1;
                }
                if i + 1 < chars.len() && (chars[i] == '\'' || chars[i] == '’') && is_word(chars[i + 1]) {
                    i += 1;
                    continue;
                }
                break;
            }
        } else if c == '{' && placeholder_len(&chars[i..]).is_some() {
            i += placeholder_len(&chars[i..]).unwrap();
        } else {
            i += 1;
        }
        out.push(chars[start..i].iter().collect());
    }
    out
}

fn placeholder_len(c: &[char]) -> Option<usize> {
    if c.len() < 5 || c[0] != '{' || c[1] != '{' {
        return None;
    }
    let mut i = 2;
    while i < c.len() && c[i].is_ascii_digit() {
        i += 1;
    }
    (i > 2 && i + 1 < c.len() && c[i] == '}' && c[i + 1] == '}').then_some(i + 2)
}

/// `WordDiff.diff`: the longest common run of words, a lone space between two changes folded
/// into the change, removals before additions, neighbours merged.
pub fn diff(old: &str, new: &str) -> Vec<Seg> {
    let a = tokenize(old);
    let b = tokenize(new);
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut raw = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            raw.push(Seg::Same(a[i].clone()));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            raw.push(Seg::Removed(a[i].clone()));
            i += 1;
        } else {
            raw.push(Seg::Added(b[j].clone()));
            j += 1;
        }
    }
    raw.extend(a[i..].iter().map(|s| Seg::Removed(s.clone())));
    raw.extend(b[j..].iter().map(|s| Seg::Added(s.clone())));
    merge(absorbing_spaces(raw))
}

fn is_change(s: &Seg) -> bool {
    !matches!(s, Seg::Same(_))
}

fn absorbing_spaces(segments: Vec<Seg>) -> Vec<Seg> {
    let mut out = Vec::new();
    for (index, seg) in segments.iter().enumerate() {
        match seg {
            Seg::Same(s) if s.chars().all(char::is_whitespace) && index > 0 && index < segments.len() - 1
                && is_change(&segments[index - 1]) && is_change(&segments[index + 1]) => {
                out.push(Seg::Removed(s.clone()));
                out.push(Seg::Added(s.clone()));
            }
            _ => out.push(seg.clone()),
        }
    }
    let mut grouped = Vec::new();
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for seg in out {
        match seg {
            Seg::Removed(_) => removed.push(seg),
            Seg::Added(_) => added.push(seg),
            Seg::Same(_) => {
                grouped.append(&mut removed);
                grouped.append(&mut added);
                grouped.push(seg);
            }
        }
    }
    grouped.append(&mut removed);
    grouped.append(&mut added);
    grouped
}

fn merge(segments: Vec<Seg>) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    for seg in segments {
        match (out.last_mut(), &seg) {
            (Some(Seg::Same(x)), Seg::Same(y)) | (Some(Seg::Removed(x)), Seg::Removed(y)) | (Some(Seg::Added(x)), Seg::Added(y)) => {
                x.push_str(y)
            }
            _ => out.push(seg),
        }
    }
    out
}

// MARK: - The answer

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Minor,
    Heavy,
    Unchanged,
}

#[derive(Clone, Debug)]
pub struct Answer {
    pub fix: bool,
    pub text: String,
    pub segments: Vec<Seg>,
    /// Fix's verdict on how much changed; none for a tone.
    pub verdict: Option<Verdict>,
    pub translated: bool,
    /// Words Iris changed that the writer might put back (`WordWatch.candidates`): (from, to).
    pub keep: Vec<(String, String)>,
}

impl Answer {
    pub fn fix(from: &str, to: &str, verdict: Option<Verdict>, translated: bool) -> Answer {
        Answer { fix: true, text: to.into(), segments: diff(from, to), verdict, translated, keep: vec![] }
    }
    pub fn tone(from: &str, to: &str, translated: bool) -> Answer {
        Answer { fix: false, text: to.into(), segments: diff(from, to), verdict: None, translated, keep: vec![] }
    }
}

/// `PanelModel.Failure`, from `ProblemNote`.
#[derive(Clone, Debug)]
pub struct Failure {
    pub note: std::borrow::Cow<'static, str>,
    pub can_retry: bool,
}

pub const TIMED_OUT: Failure = Failure { note: std::borrow::Cow::Borrowed("ran out of time. one more go?"), can_retry: true };
pub const TOO_LONG: Failure = Failure { note: std::borrow::Cow::Borrowed("that's a lot. select a part and call me again"), can_retry: false };
pub const NOT_DOWNLOADED: Failure = Failure { note: std::borrow::Cow::Borrowed("i need my model first. open setup from the menu bar"), can_retry: false };

#[derive(Clone, Debug)]
pub enum Phase {
    /// The model isn't ready: waking up (`loading`) or not on this Mac.
    Preparing,
    Generating(String),
    Done(Answer),
    Failed(Failure),
}

pub const TONES: [&str; 5] = ["Fix", "Professional", "Friendly", "Shorter", "Clearer"];

/// What the panel is showing: `PanelModel` after `preview(...)`.
#[derive(Clone, Debug)]
pub struct Panel {
    pub original: String,
    pub selected: usize,
    pub phase: Phase,
    pub app_name: Option<String>,
    pub app_bundle: Option<String>,
    pub can_replace: bool,
    pub copied: bool,
    pub retry_note: Option<String>,
}

// MARK: - The footer's rules (`PanelFooter`)

#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    Copied,
    Retry(String),
    LooksGood,
    /// A failed check, in the red pen where "looks good" sits (`ProblemNote.note`).
    Problem(String),
    Translated,
    BigRewrite,
    Keys { next_tone: bool, try_again: bool },
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Checking,
    Writing,
    Replace,
    Copy,
    Done,
    TryAgain,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppLabel {
    PastesInto(String),
    NothingToChange(String),
}

pub struct Footer {
    pub note: Note,
    pub app: Option<AppLabel>,
    pub action: Action,
    /// Tab's hint at the right: a failed check's note has the left.
    pub next_tone: bool,
}

impl Panel {
    pub fn fix_selected(&self) -> bool {
        self.selected == 0
    }

    pub fn answer(&self) -> Option<&Answer> {
        if let Phase::Done(a) = &self.phase { Some(a) } else { None }
    }

    pub fn footer(&self) -> Footer {
        let fix = self.fix_selected();
        let (action, clean, done) = match &self.phase {
            Phase::Preparing | Phase::Generating(_) => (if fix { Action::Checking } else { Action::Writing }, false, None),
            Phase::Failed(f) => (if f.can_retry { Action::TryAgain } else { Action::None }, false, None),
            Phase::Done(a) => {
                let action = if a.verdict == Some(Verdict::Unchanged) {
                    Action::Done
                } else if self.can_replace {
                    Action::Replace
                } else {
                    Action::Copy
                };
                (action, a.fix && a.verdict == Some(Verdict::Unchanged), Some(a))
            }
        };
        let app = match (action, &self.app_name) {
            (Action::Replace, Some(n)) => Some(AppLabel::PastesInto(n.clone())),
            (Action::Done, Some(n)) => Some(AppLabel::NothingToChange(n.clone())),
            _ => None,
        };
        let tones = TONES.len();
        let note = if self.copied {
            Note::Copied
        } else if let Some(r) = &self.retry_note {
            Note::Retry(r.clone())
        } else if clean {
            Note::LooksGood
        } else if let Some(a) = done {
            if a.translated {
                Note::Translated
            } else if a.fix && a.verdict == Some(Verdict::Heavy) {
                Note::BigRewrite
            } else {
                Note::Keys { next_tone: tones > 1, try_again: true }
            }
        } else if let Phase::Failed(f) = &self.phase {
            Note::Problem(f.note.to_string())
        } else {
            Note::None
        };
        let next_tone = matches!(note, Note::Problem(_)) && tones > 1;
        Footer { note, app, action, next_tone }
    }
}

// MARK: - The design check's states (`PanelStates.run`)

pub const SLACK: (&str, &str) = ("Slack", "com.tinyspeck.slackmacgap");
pub const LONG: (&str, &str) = ("T3 Code (Nightly) Preview Edition", "com.t3tools.t3code");

pub struct State {
    pub caption: String,
    pub panel: Panel,
}

pub fn states() -> Vec<State> {
    let original = "hey, i have checked the PR and its look fine but one thing need to change before merge @lead";
    let fixed = "hey, I have checked the PR and it looks fine, but one thing needs to change before merge @lead";
    let rewrite = "Hi @lead, I've reviewed the PR. It looks good overall, but one change is needed before we merge.";
    let friendly = "Hey @lead! I checked the PR and it looks good. Just one small thing to change before we merge it.";
    let clean = "Can you review this when you get a chance? @lead";
    let hinglish = "bhai order kal tak aa jayega, customer ko bol do";
    let english = "The order will arrive by tomorrow, tell the customer.";
    let tone = TONES[1];

    let fix = |from: &str, to: &str, v: Option<Verdict>| Phase::Done(Answer::fix(from, to, v, false));
    let base = |original: &str, phase: Phase| Panel {
        original: original.into(),
        selected: 0,
        phase,
        app_name: Some(SLACK.0.into()),
        app_bundle: Some(SLACK.1.into()),
        can_replace: true,
        copied: false,
        retry_note: None,
    };
    let s = |caption: &str, panel: Panel| State { caption: caption.into(), panel };
    let long = |mut p: Panel| {
        p.app_name = Some(LONG.0.into());
        p.app_bundle = Some(LONG.1.into());
        p
    };
    let toned = |p: Panel| Panel { selected: 1, ..p };
    let minor = || fix(original, fixed, Some(Verdict::Minor));
    let friendly_done = || Phase::Done(Answer::tone(original, friendly, false));

    vec![
        s("Fix · checking", base(original, Phase::Generating("hey, I".into()))),
        s("Fix · fixed · Slack", base(original, minor())),
        s("Fix · fixed · long app name", long(base(original, minor()))),
        s("Fix · fixed · app unknown", Panel { app_name: None, app_bundle: None, ..base(original, minor()) }),
        s("Fix · fixed · can only copy", Panel { can_replace: false, ..base(original, minor()) }),
        s("Fix · big rewrite · Slack", base(original, fix(original, rewrite, Some(Verdict::Heavy)))),
        s("Fix · big rewrite · long app name", long(base(original, fix(original, rewrite, Some(Verdict::Heavy))))),
        s("Fix · translated · long app name",
          long(base(hinglish, Phase::Done(Answer::fix(hinglish, english, None, true))))),
        s("Fix · nothing to fix", base(clean, fix(clean, clean, Some(Verdict::Unchanged)))),
        s("Fix · after ⌘R, came out the same", Panel { retry_note: Some("Came out the same".into()), ..base(original, minor()) }),
        s("Fix · can only copy · copied", Panel { can_replace: false, copied: true, ..base(original, minor()) }),
        s("Fix · failed, can try again", base(original, Phase::Failed(TIMED_OUT))),
        s("Fix · waking the model", base(original, Phase::Preparing)),
        s("Fix · the model isn't on this Mac", base(original, Phase::Failed(NOT_DOWNLOADED))),
        s("Fix · failed, too long", base(original, Phase::Failed(TOO_LONG))),
        s(&format!("{tone} · writing"), toned(base(original, Phase::Generating("Hey @lead!".into())))),
        s(&format!("{tone} · done · Slack"), toned(base(original, friendly_done()))),
        s(&format!("{tone} · done · long app name"), long(toned(base(original, friendly_done())))),
        s(&format!("{tone} · done · can only copy"), Panel { can_replace: false, ..toned(base(original, friendly_done())) }),
        s(&format!("{tone} · translated · Slack"),
          toned(base(hinglish, Phase::Done(Answer::tone(hinglish, english, true))))),
        s(&format!("{tone} · after ⌘R, came out the same"),
          Panel { retry_note: Some("Came out the same".into()), ..toned(base(original, friendly_done())) }),
        s(&format!("{tone} · failed, can try again"), toned(base(original, Phase::Failed(TIMED_OUT)))),
        // Every other way a check fails (`ProblemNote`), each in the red pen at the bottom left.
        s("Fix · too short", base("examples pls", failed("too short. give me a sentence", false))),
        s(&format!("{tone} · too short"), toned(base("examples pls", failed("too short. give me a sentence", false)))),
        s("Fix · one letter", base("k", failed("just one letter? give me a sentence", false))),
        s("Fix · nothing in the box", base("", failed("nothing here to rewrite", false))),
        s("Fix · couldn't make sense of it", base(original, failed("couldn't make sense of this. one more go?", true))),
        s("Fix · came back empty", base(original, failed("came back empty. one more go?", true))),
        s("Fix · dropped a name or link", base(original, failed("dropped a name or link. one more go?", true))),
        s("Fix · no room for the model", base(original, failed("no room for my model on this mac", false))),
        s("Fix · no internet for the model", base(original, failed("no internet for my model. one more go?", true))),
        s("Fix · couldn't finish", base(original, failed("couldn't finish that. one more go?", true))),
    ]
}

fn failed(note: &'static str, can_retry: bool) -> Phase {
    Phase::Failed(Failure { note: std::borrow::Cow::Borrowed(note), can_retry })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fix_reads_as_the_swift_diff_does() {
        let d = diff("hey, i have checked the PR and its look fine but",
                     "hey, I have checked the PR and it looks fine, but");
        assert_eq!(d, vec![
            Seg::Same("hey, ".into()),
            Seg::Removed("i".into()),
            Seg::Added("I".into()),
            Seg::Same(" have checked the PR and ".into()),
            Seg::Removed("its look".into()),
            Seg::Added("it looks".into()),
            Seg::Same(" fine".into()),
            Seg::Added(",".into()),
            Seg::Same(" but".into()),
        ]);
    }

    #[test]
    fn every_state_shows_what_the_swift_check_expects() {
        let s = states();
        assert_eq!(s.len(), 32);
        let f = s[8].panel.footer();
        assert_eq!((f.note, f.action), (Note::LooksGood, Action::Done));
        assert_eq!(f.app, Some(AppLabel::NothingToChange("Slack".into())));
        let f = s[11].panel.footer();
        assert_eq!((f.note, f.action, f.next_tone), (Note::Problem("ran out of time. one more go?".into()), Action::TryAgain, true));
        let f = s[22].panel.footer();
        assert_eq!((f.note, f.action, f.app), (Note::Problem("too short. give me a sentence".into()), Action::None, None));
        assert_eq!(s[5].panel.footer().note, Note::BigRewrite);
        assert_eq!(s[4].panel.footer().action, Action::Copy);
    }
}
