import Foundation

/// Why getting the model failed, in the reader's terms, so each screen can say what to do.
/// The engine reports it in its state (`EngineState.failed`) and maps its own errors to it.
public enum ModelFailure: Equatable, Sendable {
    /// Not enough disk space: the figures are already formatted ("6.2 GB").
    case noRoom(needed: String, free: String)
    /// No internet. The download resumes where it stopped once there is.
    case offline
    /// The model isn't on this Mac and nothing asked to download it.
    case notDownloaded
    /// Anything else. The system's own words go to the log, never on screen.
    case other

    /// Another attempt can work: being offline passes, and an unknown failure may. A full
    /// disk doesn't (the page retries by itself once there's room), and a missing model
    /// needs setup, not a retry.
    public var canRetry: Bool {
        switch self {
        case .offline, .other: true
        case .noRoom, .notDownloaded: false
        }
    }

    /// The whole story in one plain sentence, standing alone: the slip and Settings.
    public var sentence: String {
        switch self {
        case .noRoom(let needed, let free): "The model needs \(needed) free, and this Mac has \(free). Free up some space."
        case .offline: "No internet. The model downloads once, and picks up where it stopped."
        case .notDownloaded: "The model isn't on this Mac yet. Open the menu bar flower and finish setup to download it."
        case .other: "The model couldn't download. Try again in a moment."
        }
    }

    /// What to do, after a lead that already says what happened ("no internet"): the message
    /// by your text, the panel's VoiceOver line and setup.
    public var nextStep: String {
        switch self {
        case .noRoom(let needed, let free): "The model needs \(needed) free, and this Mac has \(free). Free up some space."
        case .offline: "The model downloads once and needs the internet for that. It picks up where it stopped."
        case .notDownloaded: "Open the menu bar flower and finish setup to download it."
        case .other: "Something got in the way. Try again in a moment; it picks up where it stopped."
        }
    }

    /// True for the network errors that mean "no connection right now".
    public static func isOffline(_ error: Error) -> Bool {
        let codes: Set<Int> = [
            NSURLErrorNotConnectedToInternet, NSURLErrorNetworkConnectionLost, NSURLErrorTimedOut,
            NSURLErrorCannotFindHost, NSURLErrorCannotConnectToHost, NSURLErrorDNSLookupFailed,
            NSURLErrorInternationalRoamingOff, NSURLErrorDataNotAllowed,
        ]
        var current: NSError? = error as NSError
        while let ns = current {
            if ns.domain == NSURLErrorDomain, codes.contains(ns.code) { return true }
            current = ns.userInfo[NSUnderlyingErrorKey] as? NSError
        }
        return false
    }
}
