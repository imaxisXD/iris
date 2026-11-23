// swift-tools-version: 6.2
import PackageDescription

// The brain of the GPUI app on the Mac: the Swift app's own engine (Gemma on MLX) and its
// rewriter (prompts, protecting names and links, the fix guard, spelling and language
// checks), compiled from the same sources (`Sources/IrisCore` etc. are links to the app's)
// into one helper program the GPUI app talks to over stdin and stdout (IrisBrain/main.swift).
// The dependencies are the app's own: its copy of mlx-swift-lm (Vendor/, with Iris's changes
// to Gemma 4 for less memory) and the same pinned versions of the rest.
let package = Package(
    name: "IrisBrain",
    platforms: [.macOS("15.0")],
    products: [.executable(name: "iris-brain", targets: ["IrisBrain"])],
    dependencies: [
        .package(path: "../../Vendor/mlx-swift-lm"),
        .package(url: "https://github.com/ml-explore/mlx-swift", exact: "0.33.0"),
        .package(url: "https://github.com/huggingface/swift-huggingface", exact: "1.0.0"),
        .package(url: "https://github.com/huggingface/swift-transformers", exact: "1.3.5"),
    ],
    targets: [
        .target(name: "IrisCore"),
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
        .target(name: "IrisSpelling", dependencies: ["IrisCore"]),
        // Windows' engine (llama.cpp's server). Built here too, so a debug brain can try it on a
        // Mac (IRIS_ENGINE=llama); a release brain never runs it (`Engines.swift`).
        .target(name: "IrisLlama", dependencies: ["IrisCore"]),
        .executableTarget(name: "IrisBrain", dependencies: ["IrisCore", "IrisEngine", "IrisSpelling", "IrisLlama"]),
    ],
    swiftLanguageModes: [.v5]
)
