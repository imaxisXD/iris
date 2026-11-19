import Foundation
import NaturalLanguage

/// Is this draft mostly not English? Then Fix translates instead of correcting (the app passes
/// this to `Rewriter`), and the panel says so rather than showing a diff across two languages.
///
/// Two signals, because neither is enough alone:
/// - a non-Latin script (Devanagari, Arabic…), which the language recognizer reads well;
/// - romanized Hindi ("kal tak aa jayega"), which it reads only as "not English". Paired
///   with the share of words the English dictionaries don't know, that's reliable, while a
///   typo-ridden English draft still scores as English.
@MainActor
public enum LanguageCheck {
    public static func looksNonEnglish(_ text: String) -> Bool {
        let letters = text.unicodeScalars.filter { CharacterSet.letters.contains($0) }
        guard letters.count >= 8 else { return false }
        let latin = letters.filter { $0.value < 0x250 }.count
        if Double(latin) / Double(letters.count) < 0.6 { return true }

        // Lowercase words only: a capitalized one is as likely a name as anything.
        let words = text.split(whereSeparator: { !$0.isLetter && $0 != "'" })
            .map(String.init)
            .filter { $0.count > 1 && $0.first?.isUppercase != true }
        guard words.count >= 4 else { return false }
        let recognizer = NLLanguageRecognizer()
        recognizer.processString(text)
        let english = recognizer.languageHypotheses(withMaximum: 3)[.english] ?? 0
        // Tuned on drafts like "bhai order kal tak aa jayega" (English 0.00, half the words
        // unknown) against typo-heavy English ("statsu", "sayed": English 0.96).
        // The recognizer goes first: it's quick, and the dictionaries take a while on a long draft.
        guard english < 0.3 else { return false }
        let unknown = words.filter { !SpellCheck.knows($0) }
        return Double(unknown.count) / Double(words.count) >= 0.2
    }
}
