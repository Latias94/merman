// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "MermanAppleSmoke",
    platforms: [
        .macOS(.v12),
    ],
    dependencies: [
        .package(path: "../../../.."),
    ],
    targets: [
        .executableTarget(
            name: "MermanAppleSmoke",
            dependencies: [
                .product(name: "Merman", package: "Merman"),
            ],
            path: "Sources/MermanAppleSmoke"
        ),
    ]
)
