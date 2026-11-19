import AppKit
import IrisCore

/// The words in a Fix draft that the system spell checker doesn't know, with its guesses,
/// for `Rewriter.spellingHints`. The model decides what to do with them.
@MainActor
public enum SpellCheck {
    public static func hints(for text: String) -> [SpellingHint] {
        let checker = NSSpellChecker.shared
        let ns = text as NSString
        var hints: [SpellingHint] = []
        var seen = Set<String>()
        var start = 0
        while start < ns.length, hints.count < SpellingHint.limit {
            let range = checker.checkSpelling(of: text, startingAt: start, language: american, wrap: false,
                                              inSpellDocumentWithTag: 0, wordCount: nil)
            guard range.location != NSNotFound, range.length > 0 else { break }
            start = range.location + range.length
            let word = ns.substring(with: range)
            guard seen.insert(word).inserted, !knows(word),
                SpellingHint.isWorthAsking(word, atSentenceStart: SpellingHint.startsSentence(after: ns.substring(to: range.location)))
            else { continue }
            let guesses = checker.guesses(forWordRange: NSRange(location: 0, length: (word as NSString).length),
                                          in: word, language: british, inSpellDocumentWithTag: 0) ?? []
            guard !guesses.isEmpty else { continue }
            hints.append(SpellingHint(word, Array(guesses.prefix(3))))
        }
        return hints
    }

    /// Whether either English dictionary knows the word. Indian English spells the British
    /// way ("colour", "summarise"), so either will do.
    public static func knows(_ word: String) -> Bool {
        [american, british].contains { language in
            NSSpellChecker.shared.checkSpelling(of: word, startingAt: 0, language: language, wrap: false,
                                                inSpellDocumentWithTag: 0, wordCount: nil).location == NSNotFound
        }
    }

    private static let american = "en"
    private static let british = "en_GB"
}
