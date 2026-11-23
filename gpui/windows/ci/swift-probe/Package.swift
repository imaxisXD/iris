// swift-tools-version: 6.0
// What the Windows model helper needs from Swift's Foundation on Windows, tried for real on the
// CI runner: streaming an HTTP answer line by line, a large download with progress, a child
// process started, read and stopped, and the machine's memory.
import PackageDescription
let package = Package(
    name: "SwiftProbe",
    targets: [.executableTarget(name: "SwiftProbe")],
    swiftLanguageModes: [.v5]
)
