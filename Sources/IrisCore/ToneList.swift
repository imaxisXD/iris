import Foundation

/// The tones people have, in their order, which the panel shows, and which it starts on.
/// Settings stores the parts; these rules decide what they mean, for the panel and the Tones
/// page alike.
public struct ToneList: Equatable, Sendable {
    public var custom: [Mode]
    /// Tone ids in the order people put them; tones missing from it follow in their own order.
    public var order: [String]
    public var hidden: Set<String>
    /// The tone the panel opens on, unless an app has its own.
    public var defaultID: String
    /// Bundle ID → the tone the panel opens on in that app.
    public var perApp: [String: String]

    public init(custom: [Mode] = [], order: [String] = [], hidden: Set<String> = [], defaultID: String = Mode.fix.id,
                perApp: [String: String] = [:]) {
        self.custom = custom
        self.order = order
        self.hidden = hidden
        self.defaultID = defaultID
        self.perApp = perApp
    }

    /// What a new Mac starts with: the built-in tones in their order, Funny hidden (fun to
    /// have, but a wrong press in a work chat costs more than it's worth).
    public static let newMac = ToneList(order: Mode.builtInTones.map(\.id), hidden: [Mode.funny.id])

    /// How many the panel's tabs hold, Fix included. Tab and the number keys reach exactly
    /// these, so no tone can be picked without being seen.
    public static let panelRoom = 6

    /// Every tone, in order, hidden ones included (the Tones page lists them all).
    public var all: [Mode] {
        let tones = Mode.builtInTones + custom
        var ordered = order.compactMap { id in tones.first { $0.id == id } }
        ordered += tones.filter { tone in !ordered.contains { $0.id == tone.id } }
        return ordered
    }

    /// What the panel shows: Fix, then the tones that are on, as many as fit.
    public var shown: [Mode] { Array(([.fix] + all.filter { !hidden.contains($0.id) }).prefix(Self.panelRoom)) }

    /// The number key that picks this tone in the panel, or nil when the panel doesn't show it.
    public func key(of id: String) -> Int? { shown.firstIndex { $0.id == id }.map { $0 + 1 } }

    /// The tone the panel opens on in the app with `bundleID`: that app's own, else the
    /// default, else Fix, whichever the panel shows.
    public func starting(for bundleID: String?) -> String {
        if let bundleID, let mode = perApp[bundleID], key(of: mode) != nil { return mode }
        return key(of: defaultID) != nil ? defaultID : Mode.fix.id
    }

    public mutating func move(_ id: String, by offset: Int) {
        var ids = all.map(\.id)
        guard let index = ids.firstIndex(of: id), ids.indices.contains(index + offset) else { return }
        ids.swapAt(index, index + offset)
        order = ids
    }

    /// Adds a tone of the writer's own, with an id no built-in tone can have.
    public mutating func add(name: String, instruction: String, id: String = UUID().uuidString) {
        custom.append(Mode(id: Mode.customPrefix + id.prefix(8), label: name, instruction: instruction))
        order = all.map(\.id)
    }

    /// Removes a tone of the writer's own, and everything that pointed at it.
    public mutating func remove(_ id: String) {
        custom.removeAll { $0.id == id }
        order.removeAll { $0 == id }
        hidden.remove(id)
        if defaultID == id { defaultID = Mode.fix.id }
        perApp = perApp.filter { $0.value != id }
    }
}
