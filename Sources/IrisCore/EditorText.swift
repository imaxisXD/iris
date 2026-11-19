import Foundation

/// Cleans text read from another app's message box before Iris works on it.
public enum EditorText {
    /// Removes characters an editor puts in its text for its own use, which mean nothing once
    /// pasted back as plain text. Slack starts every bullet line with U+E506, a private-use
    /// character from its icon font: pasted back, Slack adds its own marker again and the
    /// copy shows as a stray "≣" glyph. U+FFFC stands in for an embedded image or widget.
    /// U+F8FF (the Apple logo) is a real character people type, so it stays.
    public static func removingEditorMarkers(_ text: String) -> String {
        guard text.unicodeScalars.contains(where: isMarker) else { return text }
        var scalars = String.UnicodeScalarView()
        scalars.append(contentsOf: text.unicodeScalars.filter { !isMarker($0) })
        return String(scalars)
    }

    private static func isMarker(_ scalar: Unicode.Scalar) -> Bool {
        switch scalar.value {
        case 0xF8FF: false
        case 0xE000...0xF8FE, 0xF0000...0x10FFFF, 0xFFFC: true
        default: false
        }
    }
}
