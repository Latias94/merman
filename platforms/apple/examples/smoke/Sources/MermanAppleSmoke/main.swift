import Foundation
import Merman

@main
struct MermanAppleSmoke {
    static func main() throws {
        let client = Merman()
        guard client.bindingApiVersionV7() == 7 else {
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
        let resourceOptions = try resourceOptionsJson(
            profile: .interactive,
            overrides: [
                MermanResourceLimitOverride(
                    id: .maxPreparedTextRetainedBytes,
                    value: 1_048_576
                ),
                MermanResourceLimitOverride(id: .maxSvgBytes, value: 2_097_152),
            ]
        )
        guard resourceOptions.contains(#""max_prepared_text_retained_bytes":1048576"#),
              resourceOptions.contains(#""max_svg_bytes":2097152"#) else {
            throw SmokeError.failed("generated resource override IDs drifted from Options JSON")
        }
        let svg = try engine.renderSvg(source: source, optionsJson: nil)
        guard svg.contains("<svg"), svg.contains("Hello"),
              svg.contains("apple-registry"), measurer.callCount > 0 else {
            throw SmokeError.failed("service-backed SVG smoke failed")
        }
        guard try engine.renderAscii(source: basicSource, optionsJson: nil).contains("Hello") else {
            throw SmokeError.failed("ASCII smoke failed")
        }
        let asciiResult = try client.renderAsciiResult(
            source: basicSource, optionsJson: #"{"ascii":{"layout_profile":"auto","max_width":80,"overflow":"error"}}"#
        )
        guard let ascii = asciiResult.metadata.outputPlan?.ascii,
              ascii.schemaVersion == 3, ascii.requestedLayoutProfile == "auto",
              ["canonical", "compact"].contains(ascii.layoutProfile) else {
            throw SmokeError.failed("ASCII selection metadata was lost in the generated binding")
        }
        let compactSource = "flowchart LR\nA --> B --> C --> D"
        let compactResult = try client.renderAsciiResult(
            source: compactSource, optionsJson: #"{"ascii":{"layout_profile":"compact"}}"#
        )
        guard let compact = compactResult.metadata.outputPlan?.ascii else {
            throw SmokeError.failed("Compact plan missing")
        }
        let autoOptions = #"{"ascii":{"layout_profile":"auto","max_width":\#(compact.primaryWidth),"overflow":"error"}}"#
        let selected = try client.renderAsciiResult(source: compactSource, optionsJson: autoOptions)
        guard let selectedPlan = selected.metadata.outputPlan?.ascii,
              selectedPlan.compactAttempted, selectedPlan.layoutProfile == "compact",
              selected.data == compactResult.data else {
            throw SmokeError.failed("Auto selection lost its Compact result")
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

        let constrainedSourceOptions = try resourceOptionsJson(
            profile: .constrained,
            overrides: [
                MermanResourceLimitOverride(id: .maxSourceBytes, value: 8),
            ]
        )
        do {
            _ = try engine.renderSvg(
                source: source,
                optionsJson: constrainedSourceOptions
            )
            throw SmokeError.failed("resource failure did not return a binding error")
        } catch let error as MermanError {
            switch error {
            case let .Binding(_, codeName, _, _, resource, _, _, _, _, _):
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

        let invalidThemeRequest = MermanOperationRequestV4(
            operationId: "materialize-theme-json",
            source: #"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"styles":[{"kind":"rule","target":"node","style":{"typography":{"font_stack":[]}}}]}"#,
            uri: nil,
            optionsJson: nil,
            control: nil
        )
        for operation: () throws -> MermanOperationResult in [
            { try client.execute(request: invalidThemeRequest) },
            { try engine.execute(request: invalidThemeRequest) },
        ] {
            do {
                _ = try operation()
                throw SmokeError.failed("invalid theme definition was accepted")
            } catch let error as MermanError {
                switch error {
                case let .Binding(_, codeName, _, _, _, _, _, _, detailsJson, _):
                    guard let data = detailsJson?.data(using: .utf8),
                          let details = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                          let authoring = details["theme_authoring"] as? [String: Any],
                          let diagnostics = authoring["diagnostics"] as? [[String: Any]],
                          let diagnostic = diagnostics.first,
                          let context = diagnostic["details"] as? [String: String],
                          codeName == "MERMAN_INVALID_ARGUMENT",
                          authoring["schema_version"] as? Int == 1,
                          diagnostic["code"] as? String == "theme-authoring.invalid-token-value",
                          diagnostic["path"] as? String == "/styles/0/style/typography/font_stack",
                          context == ["expected_domain_id": "font-stack"] else {
                        throw SmokeError.failed("theme authoring lost its structured details")
                    }
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
            case let .Binding(_, codeName, _, _, _, _, _, cancellation, _, _):
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
        case let .Binding(_, _, kind, actualCapabilityId, _, _, _, _, _, _):
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
