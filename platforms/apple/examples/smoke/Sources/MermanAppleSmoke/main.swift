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
        let engine = try MermanEngine(
            optionsJson: #"{"resources":{"profile":"constrained"}}"#,
            services: services
        )
        let source = "flowchart TD\nA@{ icon: \"smoke:rocket\", label: \"Hello\" } --> B[World]"
        let svg = try engine.renderSvg(source: source, optionsJson: nil)
        guard svg.contains("<svg"), svg.contains("Hello"),
              svg.contains("apple-registry"), measurer.callCount > 0 else {
            throw SmokeError.failed("service-backed SVG smoke failed")
        }

        do {
            _ = try engine.renderSvg(
                source: source,
                optionsJson: #"{"version":2,"resources":{"profile":"constrained","limits":{"max_source_bytes":8}}}"#
            )
            throw SmokeError.failed("resource failure did not return a binding error")
        } catch let error as MermanError {
            switch error {
            case let .Binding(_, codeName, _, _, resource, _, _):
                guard codeName == "MERMAN_RESOURCE_LIMIT_EXCEEDED",
                      resource?.cause == "ceiling",
                      resource?.limitId == "max_source_bytes",
                      resource?.phase == "source",
                      (resource?.actual ?? 0) > (resource?.max ?? UInt64.max),
                      resource?.profile == "constrained" else {
                    throw SmokeError.failed("resource failure lost its structured details")
                }
            }
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
