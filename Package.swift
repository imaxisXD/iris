// swift-tools-version: 6.2
import Foundation
import PackageDescription

// The model eval (Sources/IrisEval, evals/, scripts/eval*) stays on the developer's Mac and
// is git-ignored, so its target exists only where the folder does.
let packageDir = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path
let hasEval = FileManager.default.fileExists(atPath: packageDir + "/Sources/IrisEval")

let package = Package(
    name: "Iris",
    platforms: [.macOS("15.0")],
    products: [
        .executable(name: "Iris", targets: ["Iris"]),
    ] + (hasEval ? [.executable(name: "iris-eval", targets: ["IrisEval"])] : []),
    dependencies: [
        // Pinned exactly: these move fast and break APIs between minors. mlx-swift 0.32+ needs
        // Swift tools 6.3 (Xcode 26.4+; built here with Xcode 27.2). mlx-swift-lm is a copy of
        // main at c8dabc63e7018d0b1567b2171f8df5dd8a334a55 (no release yet carries the Gemma 4
        // E-series MTP drafter, #383, and the fix for dropped speculative tokens, #731), with
        // Iris's changes to Gemma 4 for less memory: see Vendor/mlx-swift-lm/IRIS-CHANGES.md.
        .package(path: "Vendor/mlx-swift-lm"),
        // mlx-swift-lm brings it in too; listed here to hold it at this version, and because
        // the engine imports MLX itself (memory limits, arrays).
        .package(url: "https://github.com/ml-explore/mlx-swift", exact: "0.33.0"),
        .package(url: "https://github.com/huggingface/swift-huggingface", exact: "1.0.0"),
        .package(url: "https://github.com/huggingface/swift-transformers", exact: "1.3.5"),
    ],
    targets: [
        // Pure logic: prompts, protection of Slack tokens, word diff, fix guard.
        // No MLX, no AppKit, so it is fast to test.
        .target(name: "IrisCore"),

        // The single model engine (Gemma 4 on MLX).
        .target(
            name: "IrisEngine",
            dependencies: [
                "IrisCore",
                .product(name: "MLX", package: "mlx-swift"),
                .product(name: "MLXLLM", package: "mlx-swift-lm"),
                .product(name: "MLXVLM", package: "mlx-swift-lm"),
                .product(name: "MLXLMCommon", package: "mlx-swift-lm"),
                .product(name: "MLXHuggingFace", package: "mlx-swift-lm"),
                .product(name: "HuggingFace", package: "swift-huggingface"),
                .product(name: "Tokenizers", package: "swift-transformers"),
            ]
        ),

        // The Mac's own dictionaries: spelling hints for Fix, and whether a draft is English.
        // No MLX, so tests reach it.
        .target(name: "IrisSpelling", dependencies: ["IrisCore"]),

        .executableTarget(
            name: "Iris",
            dependencies: ["IrisCore", "IrisEngine", "IrisSpelling"],
            resources: [.copy("Resources/Fonts"), .copy("Resources/Changelog.md")],
            linkerSettings: [.linkedFramework("Carbon")]
        ),

        .testTarget(name: "IrisCoreTests", dependencies: ["IrisCore", "IrisSpelling", "IrisEngine"]),
    ] + (hasEval ? [.executableTarget(name: "IrisEval", dependencies: ["IrisCore", "IrisEngine", "IrisSpelling"])] : []),
    swiftLanguageModes: [.v5]
)
