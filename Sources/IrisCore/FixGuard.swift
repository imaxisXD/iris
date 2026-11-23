import Foundation

/// Checks a model "fix" before the user sees it. Small models over-correct:
/// they rewrite sentences that were already fine. The guard measures how much
/// changed and flags rewrites so the UI can say so instead of silently
/// replacing the user's voice.
public enum FixGuard {
    public struct Verdict: Equatable, Sendable {
        public enum Kind: Equatable, Sendable {
            case unchanged  // nothing to fix
            case minor  // looks like a real proofread
            case heavy  // changed a lot: show it, but warn
        }
        public let kind: Kind
        public let text: String
        public let segments: [WordDiff.Segment]
        public var changedWords: Int { FixGuard.changedWords(in: segments) }
    }

    /// Above this share of changed words (on messages long enough to judge), call it a rewrite.
    public static let heavyEditRatio = 0.4
    public static let minWordsToJudge = 6

    /// Judges a corrected text the model's wrappers are already off (`clean`).
    public static func evaluate(original: String, corrected: String) -> Verdict {
        let corrected = preservingOuterWhitespace(of: original, in: corrected)
        let segments = WordDiff.diff(original, corrected)
        let total = max(wordCount(original), 1)
        let kind: Verdict.Kind
        if corrected == original || segments.allSatisfy({ if case .same = $0 { return true } else { return false } }) {
            kind = .unchanged
        } else if total >= minWordsToJudge, Double(changedWords(in: segments)) / Double(total) > heavyEditRatio {
            kind = .heavy
        } else {
            kind = .minor
        }
        return Verdict(kind: kind, text: corrected, segments: segments)
    }

    /// Words changed in a diff. Each run of changes counts the larger of what it removed and
    /// what it added: swapping one word is one change, and so is dropping one or adding one.
    public static func changedWords(in segments: [WordDiff.Segment]) -> Int {
        var changed = 0, removed = 0, added = 0
        for segment in segments {
            switch segment {
            case .removed(let s): removed += wordCount(s)
            case .added(let s): added += wordCount(s)
            case .same:
                changed += max(removed, added)
                removed = 0; added = 0
            }
        }
        return changed + max(removed, added)
    }

    /// Drops emoji the model added that the writer didn't use. Tone prompts ask for no emoji,
    /// but small models still add them; the writer's own emoji (and :shortcodes:) stay.
    public static func removingAddedEmoji(from output: String, keeping input: String) -> String {
        func isEmoji(_ c: Character) -> Bool {
            guard let first = c.unicodeScalars.first, first.properties.isEmoji else { return false }
            // Digits, #, *, © and ® are "emoji" only with a presentation selector or keycap.
            return first.properties.isEmojiPresentation || first.value > 0x238C || c.unicodeScalars.count > 1
        }
        let allowed = Set(input.filter(isEmoji))
        guard output.contains(where: { isEmoji($0) && !allowed.contains($0) }) else { return output }
        // Tidy only the gap each emoji leaves (a doubled space, a space before punctuation):
        // spacing anywhere else is the writer's.
        let characters = Array(output)
        var kept = ""
        var inGap = false
        for (index, c) in characters.enumerated() {
            if isEmoji(c), !allowed.contains(c) { inGap = true; continue }
            if inGap {
                let afterSpace = kept.last.map { $0 == " " || $0 == "\t" } ?? true
                if c == " " || c == "\t", afterSpace { continue }
                let next = index + 1 < characters.count ? characters[index + 1] : nil
                // ":" and ";" only when they end a clause, so ":)" and ":tada:" keep their space.
                let closes = c.isNewline || ".,!?".contains(c) || (";:".contains(c) && (next?.isWhitespace ?? true))
                if closes { while kept.last == " " || kept.last == "\t" { kept.removeLast() } }
                inGap = false
            }
            kept.append(c)
        }
        return kept.trimmingCharacters(in: .whitespaces)
    }

    private static let emDashRegex = try! NSRegularExpression(pattern: #"[ \t]*—[ \t]*"#)
    private static let enDashRegex = try! NSRegularExpression(pattern: #"[ \t]+–(?=\s|$)[ \t]*"#)
    private static let capitalizingIRegex = try! NSRegularExpression(
        pattern: #"(?<![\w.'’{}-])(?!(?<=(?m:^)[ \t]{0,12})i\.)i(?=(?:['’](?:m|ll|ve|d))?(?:[\s,!?;"]|\.(?!\w)|$))"#
    )

    /// Replaces em and en dashes the model added, the most recognizable tic of model-written
    /// text; prompts alone don't stop it. A dash the writer typed stays, and so does a range
    /// like "9–5" or "Mon–Fri" (an en dash with no spaces around it).
    public static func replacingAddedDashes(in output: String, keeping input: String) -> String {
        if input.contains("—") { return output }
        // "word — word", "word—word", "word – word" become "word, word".
        var t = output.contains("—") ? replacingDashes(matching: emDashRegex, in: output) : output
        if !input.contains("–"), t.contains("–") {
            t = replacingDashes(matching: enDashRegex, in: t)
        }
        return t
    }

    /// Puts a comma where each dash was, and tidies there only: the commas, spaces and line
    /// breaks in the rest of the text ("1,000", a link, a list) are never touched.
    private static func replacingDashes(matching regex: NSRegularExpression, in text: String) -> String {
        let ns = text as NSString
        var out = ""
        var cursor = 0
        for match in regex.matches(in: text, range: NSRange(location: 0, length: ns.length)) {
            out += ns.substring(with: NSRange(location: cursor, length: match.range.location - cursor))
            cursor = match.range.location + match.range.length
            let next = cursor < ns.length ? ns.substring(with: ns.rangeOfComposedCharacterSequence(at: cursor)) : nil
            if out.isEmpty || next == nil || next == "\n" || ",.!?;:".contains(next ?? "") {
                continue  // a dash that opens or ends the text or a line, or sits before punctuation
            }
            if out.hasSuffix("\n") {
                out += ns.substring(with: match.range)  // a dash that starts a line is a list's
            } else {
                out += out.hasSuffix(",") ? " " : ", "
            }
        }
        return out + ns.substring(from: cursor)
    }

    /// "i", "i'm", "i'll", "i've", "i'd" standing alone become "I…", which smaller models
    /// often leave lowercase. Not "i.e.", a list's "i)" or "i." at the start of a line, "option i:",
    /// or anything touching a placeholder.
    public static func capitalizingI(_ text: String) -> String {
        guard text.contains("i") else { return text }
        let ns = text as NSString
        return capitalizingIRegex.stringByReplacingMatches(
            in: text, range: NSRange(location: 0, length: ns.length), withTemplate: "I")
    }

    private static let labelRegexLock = NSLock()
    private static var labelRegexCache: [String: NSRegularExpression] = [:]

    private static func labelRegex(for label: String) -> NSRegularExpression? {
        labelRegexLock.lock()
        defer { labelRegexLock.unlock() }
        if let cached = labelRegexCache[label] { return cached }
        let name = NSRegularExpression.escapedPattern(for: label)
        let pattern = #"^\s*(?:\*\*)?"# + name + #"(?:\s+(?:tone|version|rewrite|message))?(?:\*\*)?\s*[:：]\s*(?:\*\*)?\s*"#
        guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else { return nil }
        if labelRegexCache.count >= 64 {
            labelRegexCache.removeAll(keepingCapacity: true)
        }
        labelRegexCache[label] = regex
        return regex
    }

    /// Drops the tone's name the model sometimes puts before its answer ("Professional:",
    /// "Professional tone:", "**Friendly:**"), though the prompt asks for no labels. Kept when
    /// the draft itself starts that way: then it's the writer's word, not a label.
    public static func removingLabel(_ text: String, label: String, keeping original: String) -> String {
        guard text.contains(":") || text.contains("：") else { return text }
        guard let regex = labelRegex(for: label) else { return text }
        func labelled(_ s: String) -> NSRange? {
            regex.firstMatch(in: s, range: NSRange(s.startIndex..., in: s))?.range
        }
        guard labelled(original) == nil, let match = labelled(text), let range = Range(match, in: text) else { return text }
        return String(text[range.upperBound...])
    }

    /// Strips wrappers small models like to add: "Corrected:", Fix's own tags, the spell
    /// checker's line, code fences and surrounding quotes. A lead-in or quotes the writer put
    /// on their own draft stay.
    public static func clean(_ text: String, keeping input: String) -> String {
        var t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let preambles = ["corrected message:", "corrected text:", "corrected:", "here is the corrected message:",
                         "here's the corrected message:", "output:", "rewritten message:"]
        let draft = input.trimmingCharacters(in: .whitespacesAndNewlines)
        // A draft that starts "Output: …" is the writer's word, not the model's wrapper.
        for p in preambles where t.lowercased().hasPrefix(p) && !draft.lowercased().hasPrefix(p) {
            t = String(t.dropFirst(p.count)).trimmingCharacters(in: .whitespacesAndNewlines)
        }
        let tags = Prompts.messageTags
        if t.hasPrefix(tags.open) { t = String(t.dropFirst(tags.open.count)).trimmingCharacters(in: .whitespacesAndNewlines) }
        if !draft.contains(Prompts.hintLead), let hint = t.range(of: "\n" + Prompts.hintLead) {
            t = String(t[..<hint.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
        }
        if t.hasSuffix(tags.close) { t = String(t.dropLast(tags.close.count)).trimmingCharacters(in: .whitespacesAndNewlines) }
        if t.hasPrefix("```"), t.hasSuffix("```"), t.count >= 6 {
            t = String(t.dropFirst(3).dropLast(3)).trimmingCharacters(in: .whitespacesAndNewlines)
        }
        for (open, close) in [("\"", "\""), ("“", "”")] where t.hasPrefix(open) && t.hasSuffix(close) && t.count >= 2
            && !(draft.hasPrefix(open) && draft.hasSuffix(close)) {
            let inner = String(t.dropFirst().dropLast())
            if !inner.contains(open) { t = inner }
        }
        return t
    }

    static func preservingOuterWhitespace(of original: String, in text: String) -> String {
        let leading = original.prefix(while: \.isWhitespace)
        let trailing = String(original.reversed().prefix(while: \.isWhitespace).reversed())
        if original.allSatisfy(\.isWhitespace) { return original }
        return leading + text + trailing
    }

    public static func wordCount(_ s: String) -> Int {
        WordDiff.tokenize(s).filter { token in token.contains(where: { $0.isLetter || $0.isNumber }) }.count
    }
}
