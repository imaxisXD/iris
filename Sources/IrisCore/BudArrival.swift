/// Whether the bud plays its full arrival (the drop and its splash) or the quick one: the
/// first five times ever, the first time each day, and now and then after that, so it stays a
/// treat instead of a wait.
public enum BudArrival {
    /// How many arrivals are always full.
    public static let firstFull = 5
    /// After those, about one in this many is full anyway.
    public static let nowAndThen = 15

    /// `arrivals`: how many there have been before this one. `firstToday`: none yet today.
    /// `roll`: a random number in 0..<`nowAndThen`.
    public static func isFull(arrivals: Int, firstToday: Bool, roll: Int) -> Bool {
        arrivals < firstFull || firstToday || roll == 0
    }
}
