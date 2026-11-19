/// Arrow keys in a list Iris draws itself (the menu bar slip's commands, a dropdown's menu):
/// which row the highlight moves to.
public enum ListKeys {
    /// The row after (`down`) or before `current` in a list of `count` rows, stopping at the
    /// ends. With nothing highlighted yet, down starts at the top and up at the bottom. Nil for
    /// an empty list.
    public static func step(from current: Int?, count: Int, down: Bool) -> Int? {
        guard count > 0 else { return nil }
        return down ? min((current ?? -1) + 1, count - 1) : max((current ?? count) - 1, 0)
    }
}
