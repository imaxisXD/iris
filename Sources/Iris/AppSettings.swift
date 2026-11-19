import AppKit
import Foundation
import IrisCore
import IrisEngine
import ServiceManagement
import SwiftUI
import os

/// User preferences, stored in UserDefaults. Nothing here leaves the Mac. A change that has
/// to reach the system (the shortcuts, the model) goes through one function each:
/// `HotkeyCenter.apply` and `applyToEngine`.
@MainActor
final class AppSettings: ObservableObject {
    static let shared = AppSettings()
    /// A preview on the test display (`OnDisplay`) reads none of this Mac's settings and saves none.
    private let defaults = OnDisplay.sealed ? OnDisplay.sealedDefaults : UserDefaults.standard

    /// Where each setting is stored. Some keep the name they were first saved under.
    private enum Key: String {
        case defaultMode, placement, showPanelWhenClean, showChanges, hiddenNotes
        case openShortcut, quickFixShortcut
        case customTones, toneOrder, hiddenTones, appModes
        case styleRules, glossary = "glossaryList", excludedApps
        case model, idleMinutes, loadAtLaunch, onboarded
    }

    // MARK: General

    /// Where the panel opens. Above or below goes to the other side when there's no room;
    /// `aboveField` keeps its first stored name, from when it was the only "by the text".
    enum PanelPlacement: String, CaseIterable, Identifiable {
        case aboveField, belowField, nearPointer
        var id: String { rawValue }
    }
    @Published var placement: PanelPlacement { didSet { store(placement.rawValue, .placement) } }

    /// Show the "Looks good" panel when Fix finds nothing, instead of a short toast.
    @Published var showPanelWhenClean: Bool { didSet { store(showPanelWhenClean, .showPanelWhenClean) } }

    /// Mark what changed in the panel (added words in pink, removed ones struck through), for
    /// Fix and every tone. Off shows just the new text.
    @Published var showChanges: Bool { didSet { store(showChanges, .showChanges) } }

    /// The notes Iris leaves by your text that people can turn off. Problems ("nothing to
    /// read", "couldn't fix") and one-time notes always show: without them a key press
    /// would seem to do nothing.
    enum Note: String, CaseIterable {
        /// "replaced" / "fixed 4 words" after a replace. Stored as "undo", its first name.
        case replaced = "undo"
        /// "Undo with ⌘Z" on that note.
        case undoKeys
        case checking, clean, excluded
    }
    @Published var hiddenNotes: Set<Note> { didSet { store(hiddenNotes.map(\.rawValue), .hiddenNotes) } }
    func shows(_ note: Note) -> Bool { !hiddenNotes.contains(note) }
    func showsBinding(_ note: Note) -> Binding<Bool> {
        Binding(get: { self.shows(note) },
                set: { show in if show { self.hiddenNotes.remove(note) } else { self.hiddenNotes.insert(note) } })
    }

    @Published private(set) var launchAtLogin: Bool = SMAppService.mainApp.status == .enabled

    // MARK: Shortcuts

    @Published var openShortcut: KeyCombo { didSet { encode(openShortcut, .openShortcut); HotkeyCenter.shared.apply(self) } }
    /// Another app holds the open shortcut, so pressing it does nothing in Iris. Set by
    /// `HotkeyCenter.apply`; the slip and setup say so, with a way to change it.
    @Published var openShortcutTaken = false
    /// Fix and replace in one step, no panel. Optional.
    @Published var quickFixShortcut: KeyCombo? {
        didSet { encode(quickFixShortcut, .quickFixShortcut); HotkeyCenter.shared.apply(self) }
    }

    func resetShortcuts() {
        openShortcut = .openDefault
        quickFixShortcut = .quickFixDefault
    }

    // MARK: Tones

    /// The tones, their order, which are on and which the panel starts on (`ToneList`'s rules).
    @Published var tones: ToneList {
        didSet {
            encode(tones.custom, .customTones)
            store(tones.order, .toneOrder)
            store(Array(tones.hidden), .hiddenTones)
            store(tones.defaultID, .defaultMode)
            encode(tones.perApp, .appModes)
        }
    }

    /// What the panel shows: Fix, then the tones that are on, as many as it has room for.
    var modes: [Mode] { tones.shown }
    func startingMode(for bundleID: String?) -> String { tones.starting(for: bundleID) }

    /// The writer's own style rules, in their words, followed by every tone.
    @Published var styleRules: [String] { didSet { store(styleRules, .styleRules) } }

    // MARK: Words

    /// Words never to change: product names, SKUs, jargon.
    @Published var glossary: [String] { didSet { store(glossary, .glossary) } }

    /// Adds a word to the never-change list because the writer put it back.
    func learn(_ word: String) {
        if !glossary.contains(word) { glossary.append(word) }
    }

    // MARK: Apps

    /// Apps where Iris stays off (password managers, terminals...).
    @Published var excludedApps: [AppRef] { didSet { encode(excludedApps, .excludedApps) } }

    func isExcluded(_ bundleID: String?) -> Bool {
        guard let bundleID else { return false }
        return excludedApps.contains { $0.bundleID == bundleID }
    }

    struct AppRef: Codable, Hashable, Identifiable {
        let bundleID: String
        let name: String
        var id: String { bundleID }
    }

    static let defaultExclusions: [AppRef] = [
        AppRef(bundleID: "com.1password.1password", name: "1Password"),
        AppRef(bundleID: "com.apple.keychainaccess", name: "Keychain Access"),
        AppRef(bundleID: "com.apple.Passwords", name: "Passwords"),
        AppRef(bundleID: "com.bitwarden.desktop", name: "Bitwarden"),
    ]

    // MARK: Model

    @Published var model: ModelChoice { didSet { store(model.rawValue, .model); Task { await applyToEngine() } } }
    /// Free the model's memory after this many minutes unused: one of `idleChoices`.
    @Published var idleMinutes: Int { didSet { store(idleMinutes, .idleMinutes); Task { await applyToEngine() } } }
    static let idleChoices = [5, 15, 30, 60, 240]
    @Published var loadAtLaunch: Bool { didSet { store(loadAtLaunch, .loadAtLaunch) } }

    /// Hands the model settings to the engine: at launch, before anything loads (or a prewarm
    /// would load the size this Mac gets by default), and whenever they change.
    func applyToEngine() async {
        await Engine.shared.setPreferred(model)
        await Engine.shared.setIdleUnload(minutes: idleMinutes)
    }

    @Published var onboarded: Bool { didSet { store(onboarded, .onboarded) } }

    /// The model at launch. A choice made before stands, unless it was one of Google's earlier
    /// models and is no longer on this Mac: those can't be downloaded again, so Iris moves to
    /// its own. Nothing chosen yet: see `ModelChoice.starting`.
    static func startingModel(stored: ModelChoice?, setUpBefore: Bool) -> ModelChoice {
        if let stored, stored.isTrained || stored.isDownloaded { return stored }
        if stored != nil { return .recommended }
        return ModelChoice.starting(setUpBefore: setUpBefore, onDisk: \.isDownloaded, memoryGB: ModelChoice.macMemoryGB)
    }

    private init() {
        placement = defaults.string(forKey: Key.placement.rawValue).flatMap(PanelPlacement.init(rawValue:)) ?? .aboveField
        showPanelWhenClean = defaults.object(forKey: Key.showPanelWhenClean.rawValue) as? Bool ?? true
        showChanges = defaults.object(forKey: Key.showChanges.rawValue) as? Bool ?? true
        hiddenNotes = Set((defaults.stringArray(forKey: Key.hiddenNotes.rawValue) ?? []).compactMap(Note.init(rawValue:)))
        openShortcut = Self.decode(KeyCombo.self, .openShortcut, defaults) ?? .openDefault
        quickFixShortcut = defaults.object(forKey: Key.quickFixShortcut.rawValue) == nil
            ? .quickFixDefault : Self.decode(KeyCombo?.self, .quickFixShortcut, defaults) ?? nil
        let fresh = ToneList.newMac
        tones = ToneList(custom: Self.decode([Mode].self, .customTones, defaults) ?? fresh.custom,
                         order: defaults.stringArray(forKey: Key.toneOrder.rawValue) ?? fresh.order,
                         hidden: defaults.stringArray(forKey: Key.hiddenTones.rawValue).map(Set.init) ?? fresh.hidden,
                         defaultID: defaults.string(forKey: Key.defaultMode.rawValue) ?? fresh.defaultID,
                         perApp: Self.decode([String: String].self, .appModes, defaults) ?? fresh.perApp)
        glossary = defaults.stringArray(forKey: Key.glossary.rawValue) ?? []
        styleRules = defaults.stringArray(forKey: Key.styleRules.rawValue) ?? []
        excludedApps = Self.decode([AppRef].self, .excludedApps, defaults) ?? Self.defaultExclusions
        let onboarded = defaults.bool(forKey: Key.onboarded.rawValue)
        model = Self.startingModel(stored: defaults.string(forKey: Key.model.rawValue).flatMap(ModelChoice.init(rawValue:)),
                                   setUpBefore: onboarded)
        idleMinutes = defaults.object(forKey: Key.idleMinutes.rawValue) as? Int ?? 30
        loadAtLaunch = defaults.object(forKey: Key.loadAtLaunch.rawValue) as? Bool ?? true
        self.onboarded = onboarded
    }

    /// What macOS still needs before Iris opens at login, in Iris's words, for whichever window
    /// shows the switch (setup, Settings). Nil when nothing's needed.
    @Published private(set) var launchAtLoginProblem: String?

    func setLaunchAtLogin(_ enabled: Bool) {
        do {
            if enabled { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            launchAtLoginProblem = enabled && SMAppService.mainApp.status == .requiresApproval
                ? "Approve Iris in System Settings → General → Login Items." : nil
        } catch {
            Self.log.error("login item: \(error.localizedDescription, privacy: .public)")
            launchAtLoginProblem = "macOS didn't let Iris open at login. Add it in System Settings → General → Login Items."
        }
        launchAtLogin = SMAppService.mainApp.status == .enabled
    }

    // MARK: Storing

    private func store(_ value: Any, _ key: Key) {
        guard !OnDisplay.sealed else { return }
        defaults.set(value, forKey: key.rawValue)
    }

    private func encode<T: Encodable>(_ value: T, _ key: Key) {
        guard !OnDisplay.sealed else { return }
        do { defaults.set(try JSONEncoder().encode(value), forKey: key.rawValue) } catch {
            Self.log.error("couldn't save \(key.rawValue, privacy: .public): \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Nil when nothing's stored; what can't be read is logged, and the default used instead.
    private static func decode<T: Decodable>(_ type: T.Type, _ key: Key, _ defaults: UserDefaults) -> T? {
        guard let data = defaults.data(forKey: key.rawValue) else { return nil }
        do { return try JSONDecoder().decode(T.self, from: data) } catch {
            log.error("couldn't read \(key.rawValue, privacy: .public), using the default: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    private static let log = Logger(subsystem: "local.iris.app", category: "settings")
}

/// Apps in the Applications folders, read once per menu open (a few hundred stat calls).
enum InstalledApps {
    struct App: Hashable { let bundleID: String; let name: String }

    @MainActor static var all: [App] {
        let fm = FileManager.default
        let roots = ["/Applications", "/System/Applications", "/System/Applications/Utilities",
                     fm.homeDirectoryForCurrentUser.appendingPathComponent("Applications").path]
        var seen = Set<String>()
        var apps: [App] = []
        for root in roots {
            for entry in (try? fm.contentsOfDirectory(atPath: root)) ?? [] where entry.hasSuffix(".app") {
                let path = root + "/" + entry
                guard let id = Bundle(path: path)?.bundleIdentifier, id != Bundle.main.bundleIdentifier,
                    seen.insert(id).inserted
                else { continue }
                apps.append(App(bundleID: id, name: name(at: path)))
            }
        }
        return apps.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    /// An app's name as Finder shows it, without ".app".
    static func name(at path: String) -> String {
        FileManager.default.displayName(atPath: path).replacingOccurrences(of: ".app", with: "")
    }
}
