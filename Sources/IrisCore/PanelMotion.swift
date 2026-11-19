/// How the panel comes and goes, in seconds: the panel draws it (`PanelView`), the window
/// waits for it (`PanelController`) and the design frames replay it (`RigFrames`), all from
/// these numbers, so the window never leaves before the fold has finished on screen.
public enum PanelMotion {
    /// Folded, the paper is a sliver at the flower's head, turned a little.
    public static let folded = (scaleX: 0.22, scaleY: 0.04, degrees: -4.0)

    /// Unrolling: a quick spring with a little give, the paper solid almost at once so it
    /// never shows as a pale ghost box while it grows.
    public static let unroll = (duration: 0.34, bounce: 0.14)
    public static let unrollFade = 0.07

    /// Folding back down into the flower; the fade starts a beat after the shrink.
    public static let foldShrink = 0.18
    public static let foldFade = (delay: 0.08, duration: 0.12)
    /// From the fold starting to the paper gone: when the window can leave.
    public static var foldDuration: Double { max(foldShrink, foldFade.delay + foldFade.duration) }

    /// How long the bud stands alone before the panel may open: its whole arrival when it
    /// plays in full, a moment when it's the quick one.
    public static func budMoment(full: Bool) -> Double { full ? 0.6 : 0.1 }
    /// After the bloom starts, once the flower has opened, the paper follows.
    public static func afterBloom(full: Bool) -> Double { full ? 0.26 : 0.06 }
    /// A slow answer still unfolds (and streams in the panel) after this.
    public static let unfoldAnyway = 1.1

    /// How long to wait before unrolling, `waited` seconds after the bud arrived: never before
    /// its moment, and when the flower has bloomed, a beat after, so you see bud, bloom, panel.
    public static func unrollDelay(waited: Double, bloomed: Bool, full: Bool) -> Double {
        max(0, budMoment(full: full) - waited) + (bloomed ? afterBloom(full: full) : 0)
    }
}
