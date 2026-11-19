import Foundation

/// Compares release versions ("0.2.0", "v0.10.1") for a downloaded copy of Iris, which checks
/// GitHub's latest release when you choose Check for Updates.
public enum ReleaseVersion {
    /// The numbers in a version, without a leading "v" or anything after a "-" ("1.2.0-beta").
    public static func parts(_ version: String) -> [Int]? {
        var v = version.trimmingCharacters(in: .whitespaces)
        if v.lowercased().hasPrefix("v") { v.removeFirst() }
        if let dash = v.firstIndex(of: "-") { v = String(v[..<dash]) }
        let numbers = v.split(separator: ".").map { Int($0) }
        guard !numbers.isEmpty, numbers.allSatisfy({ $0 != nil }) else { return nil }
        return numbers.compactMap { $0 }
    }

    /// True when `latest` is a newer release than `installed`. Unreadable versions never count
    /// as newer, so a bad tag can't nag anyone.
    public static func isNewer(_ latest: String, than installed: String) -> Bool {
        guard let a = parts(latest), let b = parts(installed) else { return false }
        for i in 0..<max(a.count, b.count) {
            let x = i < a.count ? a[i] : 0, y = i < b.count ? b[i] : 0
            if x != y { return x > y }
        }
        return false
    }
}
