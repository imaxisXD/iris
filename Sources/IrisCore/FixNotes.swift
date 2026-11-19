import Foundation

/// Names each change in a Fix result, the way an editor writes in the margin: "capital I",
/// "verb form", "comma". Worked out from the word diff by simple rules, not by the model,
/// so a label is never a guess dressed up as fact: when no rule fits it says "wording".
public enum FixNotes {
    public struct Change: Equatable, Sendable {
        public let from: String
        public let to: String
        public let reason: Reason
    }

    /// What kind of change, in the margin's words.
    public enum Reason: String, Sendable {
        case capitalI = "capital I", capitalLetter = "capital letter", apostrophe, comma, questionMark = "question mark"
        case punctuation, article, preposition, wordOrder = "word order", agreement, verbForm = "verb form", spelling, wording
    }

    /// One change per run of removed and/or added words, in reading order.
    public static func changes(_ segments: [WordDiff.Segment]) -> [Change] {
        var out: [Change] = []
        var removed = "", added = ""
        func flush() {
            let from = removed.trimmingCharacters(in: .whitespaces), to = added.trimmingCharacters(in: .whitespaces)
            if !from.isEmpty || !to.isEmpty { out.append(Change(from: from, to: to, reason: reason(from: from, to: to))) }
            removed = ""; added = ""
        }
        for segment in segments {
            switch segment {
            case .removed(let s): removed += s
            case .added(let s): added += s
            case .same: flush()
            }
        }
        flush()
        return out
    }

    static let articles: Set<String> = ["a", "an", "the"]
    static let prepositions: Set<String> = ["about", "at", "by", "for", "from", "in", "into", "of", "on", "onto",
                                            "to", "with", "since", "till", "until", "over", "under", "between"]

    public static func reason(from: String, to: String) -> Reason {
        let a = from.lowercased(), b = to.lowercased()
        let aWords = words(a), bWords = words(b)

        if !from.isEmpty, a == b {
            return to == "I" || from == "i" ? .capitalI : .capitalLetter
        }
        if strip(a, "'’") == strip(b, "'’"), a != b { return .apostrophe }
        // Only punctuation changed.
        if aWords == bWords {
            let added = Set(b).subtracting(a)
            if added.contains(",") || a.contains(",") && !b.contains(",") { return .comma }
            if added.contains("?") { return .questionMark }
            return .punctuation
        }
        // A word added or dropped on its own.
        let removedOnly = Set(aWords).subtracting(bWords), addedOnly = Set(bWords).subtracting(aWords)
        let touched = removedOnly.union(addedOnly)
        if !touched.isEmpty, touched.isSubset(of: articles) { return .article }
        if !touched.isEmpty, touched.isSubset(of: prepositions) { return .preposition }
        // Same words, different order.
        if aWords.count > 1, aWords.sorted() == bWords.sorted() { return .wordOrder }
        // One word for one word.
        if aWords.count == 1, bWords.count == 1, let x = aWords.first, let y = bWords.first {
            if sameStem(x, y) { return y.hasSuffix("s") && !x.hasSuffix("s") || x.hasSuffix("s") && !y.hasSuffix("s") ? .agreement : .verbForm }
            if distance(x, y) <= max(1, min(x.count, y.count) / 3) { return .spelling }
        }
        // A few words, one of which is a form of another ("its look" -> "it looks").
        if aWords.count == bWords.count, zip(aWords, bWords).contains(where: { $0 != $1 && sameStem($0, $1) }) {
            return .verbForm
        }
        return .wording
    }

    private static func words(_ s: String) -> [String] {
        s.split(whereSeparator: { !$0.isLetter && !$0.isNumber && $0 != "'" && $0 != "’" }).map(String.init)
    }

    private static func strip(_ s: String, _ chars: String) -> String { s.filter { !chars.contains($0) } }

    /// "look"/"looks"/"looked"/"looking", "need"/"needs", "go"/"goes".
    private static func sameStem(_ x: String, _ y: String) -> Bool {
        guard x != y else { return false }
        let endings = ["", "s", "es", "ed", "d", "ing"]
        for e1 in endings where x.hasSuffix(e1) {
            for e2 in endings where y.hasSuffix(e2) {
                let s1 = x.dropLast(e1.count), s2 = y.dropLast(e2.count)
                if s1.count >= 2, s1 == s2 { return true }
            }
        }
        return false
    }

    private static func distance(_ x: String, _ y: String) -> Int {
        let a = Array(x), b = Array(y)
        var row = Array(0...b.count)
        for i in 1...max(a.count, 1) where !a.isEmpty {
            var prev = row[0]
            row[0] = i
            for j in 1...max(b.count, 1) where !b.isEmpty {
                let cur = row[j]
                row[j] = min(row[j] + 1, row[j - 1] + 1, prev + (a[i - 1] == b[j - 1] ? 0 : 1))
                prev = cur
            }
        }
        return a.isEmpty ? b.count : row[b.count]
    }
}

/// After a Replace: did the writer put one of Iris's changes back? Then Iris can offer to
/// leave that word alone from now on. Only words where that makes sense (a name or a
/// spelling Iris "fixed"), never grammar like "need" -> "needs".
public enum WordWatch {
    /// The single words Iris changed that could be names or deliberate spellings.
    public static func candidates(_ changes: [FixNotes.Change], ignoring kept: [String] = []) -> [FixNotes.Change] {
        let reasons: Set<FixNotes.Reason> = [.spelling, .wording, .capitalLetter]
        return changes.filter { change in
            reasons.contains(change.reason)
                && change.from.count >= 2
                && change.from.allSatisfy { $0.isLetter || $0 == "'" || $0 == "’" || $0 == "-" }
                && !kept.contains(change.from)
        }
    }

    /// Iris's version is in the box: one of its words is there. Watched for first, so a paste
    /// that failed isn't read as the writer putting words back.
    public static func landed(_ candidates: [FixNotes.Change], in text: String) -> Bool {
        candidates.contains { contains(text, word: $0.to) }
    }

    /// The first candidate the writer changed back: its original word is in the text again
    /// and Iris's word isn't.
    public static func reverted(_ candidates: [FixNotes.Change], in text: String) -> FixNotes.Change? {
        candidates.first { contains(text, word: $0.from) && !contains(text, word: $0.to) }
    }

    public static func contains(_ text: String, word: String) -> Bool {
        guard !word.isEmpty else { return false }
        let pattern = "(?<![\\p{L}\\p{N}])" + NSRegularExpression.escapedPattern(for: word) + "(?![\\p{L}\\p{N}])"
        return text.range(of: pattern, options: .regularExpression) != nil
    }
}
