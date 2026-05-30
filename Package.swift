// swift-tools-version: 5.9
//
// Local development:
//   1. Run `bash scripts/build-apple-xcframework.sh` on macOS to produce Merman.xcframework.
//   2. Add this package locally in Xcode or SwiftPM.
//
// Published releases can replace the local binaryTarget path with a remote url + checksum target.

import PackageDescription

let package = Package(
    name: "Merman",
    platforms: [
        .iOS(.v14),
        .macOS(.v12),
    ],
    products: [
        .library(name: "Merman", targets: ["Merman"]),
    ],
    targets: [
        .binaryTarget(
            name: "MermanFFI",
            path: "platforms/apple/Merman.xcframework"
        ),
        .target(
            name: "Merman",
            dependencies: ["MermanFFI"],
            path: "platforms/apple/Sources/Merman"
        ),
    ]
)
