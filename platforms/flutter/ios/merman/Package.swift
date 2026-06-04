// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "merman",
    platforms: [
        .iOS(.v13),
    ],
    products: [
        .library(name: "merman", targets: ["merman"]),
    ],
    dependencies: [
        .package(name: "FlutterFramework", path: "../FlutterFramework"),
    ],
    targets: [
        .binaryTarget(
            name: "MermanFFI",
            path: "../MermanFFI.xcframework"
        ),
        .target(
            name: "merman",
            dependencies: [
                "MermanFFI",
                .product(name: "FlutterFramework", package: "FlutterFramework"),
            ],
            path: "Sources/merman"
        ),
    ]
)
