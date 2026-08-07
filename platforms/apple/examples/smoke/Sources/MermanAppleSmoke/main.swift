import Foundation
import Merman

@main
struct MermanAppleSmoke {
    static func main() throws {
        let iconPack = MermanIconPack(
            json: #"{"icons":{"rocket":{"body":"<path data-icon=\"apple-registry\" d=\"M0 0H16V16H0z\"/>"}}}"#,
            registrationName: "smoke"
        )
        let iconRegistry = try MermanIconRegistry.fromPacks(packs: [iconPack])
        let measurer = FallbackTextMeasurer()
        let services = MermanEngineServices()
            .withIconRegistry(iconRegistry: iconRegistry)
            .withTextMeasurer(textMeasurer: measurer)
        let engine = try MermanEngine(optionsJson: nil, services: services)
        let svg = try engine.renderSvg(
            source: "flowchart TD\nA@{ icon: \"smoke:rocket\", label: \"Hello\" } --> B[World]",
            optionsJson: nil
        )
        guard svg.contains("<svg"), svg.contains("Hello"),
              svg.contains("apple-registry"), measurer.callCount > 0 else {
            throw SmokeError.failed("service-backed SVG smoke failed")
        }
        try engine.close()

        print("merman Apple UniFFI smoke passed")
    }
}

private enum SmokeError: Error {
    case failed(String)
}

private final class FallbackTextMeasurer: MermanTextMeasurer, @unchecked Sendable {
    private let lock = NSLock()
    private var calls = 0

    var callCount: Int {
        lock.lock()
        defer { lock.unlock() }
        return calls
    }

    func measure(request _: MermanTextMeasureRequest) throws -> MermanTextMeasureResult? {
        lock.lock()
        calls += 1
        lock.unlock()
        return nil
    }
}
