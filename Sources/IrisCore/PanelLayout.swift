import CoreGraphics

/// Where the panel's paper and the flower go, in screen points with y up (AppKit's).
///
/// The paper sits just off the text box, above or below it, its left edge in line with the
/// box's, so your eye goes straight from what you typed to the fix. The flower stands to the
/// paper's left, beside it and never under it, and the paper unrolls sideways out of its bloom.
///
///   ✿  ┌──── paper ────┐           ✿  ┌── text box ───┐
///   │  └───────────────┘           │  └───────────────┘
///  ─┴─ ┌── text box ───┐    or    ─┴─ ┌──── paper ────┐
///      └───────────────┘              └───────────────┘
public struct PanelLayout: Equatable, Sendable {
    public enum Side: Equatable, Sendable { case above, below }

    /// The flower's reach around its foot, in points.
    public struct Flower: Equatable, Sendable {
        /// How far its petals reach left and right of the foot when open.
        public var left: CGFloat
        public var right: CGFloat
        /// From the foot to the top of the bloom.
        public var height: CGFloat
        /// From the foot to the middle of the head, where the paper unrolls from.
        public var headroom: CGFloat

        public init(left: CGFloat, right: CGFloat, height: CGFloat, headroom: CGFloat) {
            self.left = left; self.right = right; self.height = height; self.headroom = headroom
        }
    }

    /// The paper itself, without the room the window keeps round it for the shadow.
    public var paper: CGRect
    public var side: Side
    /// Where the flower stands.
    public var foot: CGPoint
    /// The bloom's head: the paper unrolls out of it.
    public var hinge: CGPoint

    /// Between the paper and the text box.
    public static let gap: CGFloat = 10
    /// Between the open bloom and the paper's edge.
    public static let flowerGap: CGFloat = 10
    /// The least room left between the paper and the screen's edge.
    public static let edge: CGFloat = 8

    /// - Parameters:
    ///   - anchor: the text box, or a zero-size rect where the paper's corner should go.
    ///   - prefer: the side people chose; the other side is used only when it doesn't fit.
    ///   - keep: a side already chosen, so a panel growing as text streams in never jumps.
    public static func place(paper size: CGSize, by anchor: CGRect, prefer: Side, keep: Side? = nil,
                             in visible: CGRect, flower: Flower) -> PanelLayout {
        let fitsAbove = anchor.maxY + gap + size.height <= visible.maxY - edge
        let fitsBelow = anchor.minY - gap - size.height >= visible.minY + edge
        let other: Side = prefer == .above ? .below : .above
        let fits = { (side: Side) in side == .above ? fitsAbove : fitsBelow }
        // Neither fits (a tall panel on a short screen): the roomier side, clamped on screen.
        let side = keep ?? (fits(prefer) ? prefer : fits(other) ? other : roomier(anchor, visible))

        // In line with the box, but never so far left the flower leaves the screen, nor so far
        // right the paper does.
        let room = flower.right + flowerGap
        let lowest = visible.minX + edge + flower.left + room
        let highest = visible.maxX - edge - size.width
        let x = max(visible.minX + edge, min(max(anchor.minX, lowest), highest))

        let wanted = side == .above ? anchor.maxY + gap : anchor.minY - gap - size.height
        let y = max(visible.minY + edge, min(wanted, visible.maxY - edge - size.height))
        let paper = CGRect(origin: CGPoint(x: x, y: y), size: size)

        // Above, the flower stands on the box's top edge beside the paper's foot; below, it
        // stands beside the paper with its bloom level with the paper's top.
        let footY = side == .above ? paper.minY - gap + 2 : paper.maxY - flower.height
        let foot = CGPoint(x: paper.minX - room, y: footY)
        return PanelLayout(paper: paper, side: side, foot: foot,
                           hinge: CGPoint(x: paper.minX, y: foot.y + flower.headroom))
    }

    /// The side with more screen between the box and the edge.
    private static func roomier(_ anchor: CGRect, _ visible: CGRect) -> Side {
        visible.maxY - anchor.maxY >= anchor.minY - visible.minY ? .above : .below
    }
}

extension PanelLayout {
    /// Whether the bud can stand on `rect`: a box-sized place on the screen. One taller than
    /// 40% of the screen is a whole page (a chat history, a document), with no room above or
    /// below it for the panel; one wider than the screen is nonsense (Chrome sometimes answers
    /// with a selection 30,000 points wide).
    public static func canStand(on rect: CGRect, visible: CGRect) -> Bool {
        rect.intersects(visible) && rect.height <= visible.height * 0.4 && rect.width <= visible.width
    }
}
