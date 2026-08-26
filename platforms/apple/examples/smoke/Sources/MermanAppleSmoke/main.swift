import Foundation
import Merman

@main
struct MermanAppleSmoke {
    static func main() throws {
        let client = Merman()
        guard client.bindingApiVersionV6() == 6 else {
            throw SmokeError.failed("unexpected UniFFI binding API version")
        }
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
        let basicSource = "flowchart TD\nA[Hello] --> B[World]"
        let svg = try engine.renderSvg(source: source, optionsJson: nil)
        guard svg.contains("<svg"), svg.contains("Hello"),
              svg.contains("apple-registry"), measurer.callCount > 0 else {
            throw SmokeError.failed("service-backed SVG smoke failed")
        }
        guard try engine.renderAscii(source: basicSource, optionsJson: nil).contains("Hello") else {
            throw SmokeError.failed("ASCII smoke failed")
        }
        guard !(try engine.analyzeJson(source: basicSource, optionsJson: nil)).isEmpty else {
            throw SmokeError.failed("analysis smoke failed")
        }
        try requireMissingCapability("png") {
            _ = try engine.renderPng(source: basicSource, optionsJson: nil)
        }
        try requireMissingCapability("jpeg") {
            _ = try engine.renderJpeg(source: basicSource, optionsJson: nil)
        }
        try requireMissingCapability("pdf") {
            _ = try engine.renderPdf(source: basicSource, optionsJson: nil)
        }
        try requireMissingCapability("math") {
            _ = try engine.renderSvg(
                source: "flowchart TD\nA[\"$$x^2$$\"] --> B",
                optionsJson: nil
            )
        }

        do {
            _ = try engine.renderSvg(
                source: source,
                optionsJson: #"{"version":2,"resources":{"profile":"constrained","limits":{"max_source_bytes":8}}}"#
            )
            throw SmokeError.failed("resource failure did not return a binding error")
        } catch let error as MermanError {
            switch error {
            case let .Binding(_, codeName, _, _, resource, _, _, _, _):
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

        let deadline = MermanOperationControl(timeoutMs: 0)
        let request = MermanOperationRequestV4(
            operationId: "svg",
            source: basicSource,
            uri: nil,
            optionsJson: nil,
            control: deadline
        )
        do {
            _ = try client.execute(request: request)
            throw SmokeError.failed("expired operation deadline did not cancel the request")
        } catch let error as MermanError {
            switch error {
            case let .Binding(_, codeName, _, _, _, _, _, cancellation, _):
                guard codeName == "MERMAN_CANCELLED",
                      cancellation?.reason == "deadline_exceeded",
                      cancellation?.phase == "admission" else {
                    throw SmokeError.failed(
                        "deadline failure lost its structured cancellation details"
                    )
                }
            }
        }

        try engine.close()
        print("merman Apple UniFFI smoke passed")
    }
}

private func requireMissingCapability(
    _ capabilityId: String,
    operation: () throws -> Void
) throws {
    do {
        try operation()
    } catch let error as MermanError {
        switch error {
        case let .Binding(_, _, kind, actualCapabilityId, _, _, _, _, _):
            guard kind == .missingCapability, actualCapabilityId == capabilityId else {
                throw SmokeError.failed(
                    "\(capabilityId) failure lost its missing-capability contract"
                )
            }
            return
        }
    }
    throw SmokeError.failed(
        "default native artifact unexpectedly supports \(capabilityId)"
    )
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
