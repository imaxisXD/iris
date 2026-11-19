import IrisSpelling
import Testing

@MainActor
@Suite struct SpellingTests {
    @Test func britishSpellingIsEnglish() {
        // Indian English spells the British way: neither the hints nor the language check
        // treats it as a mistake or as another language.
        #expect(SpellCheck.knows("colour"))
        #expect(SpellCheck.hints(for: "the colour of the favourite button").isEmpty)
        #expect(!LanguageCheck.looksNonEnglish("please summarise the colour changes in the favourite labels"))
    }

    @Test func romanizedHindiIsAnotherLanguage() {
        #expect(LanguageCheck.looksNonEnglish("bhai order kal tak aa jayega, customer ko bol do"))
        #expect(!LanguageCheck.looksNonEnglish("hey i have checked the pr and its look fine"))
    }

    @Test func asksAboutTypos() {
        #expect(SpellCheck.hints(for: "then favourism").map(\.word) == ["favourism"])
    }
}
