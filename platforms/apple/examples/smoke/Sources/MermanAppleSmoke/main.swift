import Foundation
import CoreFoundation
import Merman

@main
struct MermanAppleSmoke {
    static func main() throws {
        let source = "flowchart TD\nA[Hello] --> B[World]"
        let engine = MermanEngine()

        guard engine.bindingApiVersion() == 3 else {
            throw SmokeError.failed("expected UniFFI binding API 3")
        }
        guard !engine.packageVersion().isEmpty else {
            throw SmokeError.failed("package version was empty")
        }

        try validateRuntimeCatalog(try jsonObject(engine.runtimeCatalogJson()), engine: engine)

        let constrainedOptions = try resourceOptionsJson(profile: .constrained, overrides: [])
        let svg = try engine.renderSvg(source: source, optionsJson: constrainedOptions)
        guard svg.contains("<svg"), svg.contains("Hello"), svg.contains("World") else {
            throw SmokeError.failed("SVG smoke failed")
        }

        let png = try engine.renderPng(source: source, optionsJson: constrainedOptions)
        guard png.starts(with: Data([0x89, 0x50, 0x4E, 0x47])) else {
            throw SmokeError.failed("PNG smoke failed")
        }

        let jpeg = try engine.renderJpeg(source: source, optionsJson: constrainedOptions)
        guard jpeg.starts(with: Data([0xFF, 0xD8, 0xFF])) else {
            throw SmokeError.failed("JPEG smoke failed")
        }

        let pdf = try engine.renderPdf(source: source, optionsJson: constrainedOptions)
        guard pdf.starts(with: Data("%PDF-".utf8)) else {
            throw SmokeError.failed("PDF smoke failed")
        }

        let operation = MermanOperationRequest(
            operationId: "svg",
            source: source,
            uri: nil,
            optionsJson: #"{"runtime_policy":"native","svg":{"diagram_id":"swift-one-shot"}}"#
        )
        let genericResult = try engine.execute(request: operation)
        guard genericResult.operationId == "svg",
              genericResult.mediaType == "image/svg+xml",
              String(data: genericResult.data, encoding: .utf8)?.contains("id=\"swift-one-shot\"") == true,
              genericResult.metadataJson.contains("\"runtime_policy\":\"native\"") else {
            throw SmokeError.failed("generic operation smoke failed")
        }

        do {
            let unknownOperation = MermanOperationRequest(
                operationId: "not-an-operation",
                source: source,
                uri: nil,
                optionsJson: constrainedOptions
            )
            _ = try engine.execute(request: unknownOperation)
            throw SmokeError.failed("unknown operation did not return a binding error")
        } catch let error as MermanError {
            switch error {
            case let .Binding(_, _, kind, capabilityId, _):
                guard kind == .unknownOperation, capabilityId == nil else {
                    throw SmokeError.failed("unknown operation lost its machine-readable error fields")
                }
            }
        }

        let reusable = try engine.reusableEngine(optionsJson: constrainedOptions)
        let reusableOperation = MermanOperationRequest(
            operationId: "svg",
            source: source,
            uri: nil,
            optionsJson: #"{"svg":{"diagram_id":"swift-request"}}"#
        )
        let reusableResult = try reusable.execute(request: reusableOperation)
        guard reusableResult.operationId == "svg",
              String(data: reusableResult.data, encoding: .utf8)?.contains("id=\"swift-request\"") == true else {
            throw SmokeError.failed("reusable generic operation smoke failed")
        }

        let measurer = FallbackTextMeasurer()
        let measuredReusable = try engine.reusableEngineWithTextMeasurer(
            optionsJson: constrainedOptions,
            measurer: measurer
        )
        let measuredSvg = try measuredReusable.renderSvg(source: source, optionsJson: nil)
        guard measuredSvg.contains("<svg"), measurer.callCount > 0 else {
            throw SmokeError.failed("generated text-measurement callback smoke failed")
        }

        let semanticJSON = try engine.parseJson(source: source, optionsJson: constrainedOptions)
        guard semanticJSON.contains("flowchart-v2") else {
            throw SmokeError.failed("semantic JSON smoke failed")
        }

        let validation = try engine.validate(source: source, optionsJson: constrainedOptions)
        guard validation.valid else {
            throw SmokeError.failed("validation smoke failed")
        }

        guard engine.supportedDiagrams().contains("flowchart") else {
            throw SmokeError.failed("supported diagrams smoke failed")
        }

        print("merman Apple UniFFI smoke passed (\(engine.packageVersion()))")
    }
}

private func jsonObject(_ text: String) throws -> [String: Any] {
    let value = try JSONSerialization.jsonObject(with: Data(text.utf8))
    guard let object = value as? [String: Any] else {
        throw SmokeError.failed("expected JSON object")
    }
    return object
}

private func integer(_ value: Any?) -> Int? {
    guard let number = value as? NSNumber else { return nil }
    return CFGetTypeID(number) == CFBooleanGetTypeID() ? nil : number.intValue
}

private func validateRuntimeCatalog(_ catalog: [String: Any], engine: MermanEngine) throws {
    let expectedKeys: Set<String> = [
        "schema_version", "transport_api_version", "package_version", "capabilities", "registry", "resources",
    ]
    guard Set(catalog.keys) == expectedKeys,
          integer(catalog["schema_version"]) == 1,
          (integer(catalog["transport_api_version"]) ?? -1) == engine.bindingApiVersion(),
          catalog["package_version"] as? String == engine.packageVersion(),
          let capabilities = catalog["capabilities"] as? [String: Any],
          let registry = catalog["registry"] as? [String: Any],
          let resources = catalog["resources"] as? [String: Any] else {
        throw SmokeError.failed("runtime catalog did not describe the native SDK artifact")
    }

    let expectedCapabilityKeys: Set<String> = [
        "capability_ids", "output_ids", "operation_ids", "system_adapter_ids", "text_measurement",
    ]
    guard Set(capabilities.keys) == expectedCapabilityKeys,
          let capabilityIDs = capabilities["capability_ids"] as? [String],
          let outputIDs = capabilities["output_ids"] as? [String],
          let operationIDs = capabilities["operation_ids"] as? [String],
          let systemAdapterIDs = capabilities["system_adapter_ids"] as? [String],
          let textMeasurement = capabilities["text_measurement"] as? [String: Any] else {
        throw SmokeError.failed("runtime catalog capabilities were malformed")
    }
    try requireSortedUnique(capabilityIDs, field: "runtime capability IDs")
    try requireSortedUnique(outputIDs, field: "runtime output IDs")
    try requireSortedUnique(operationIDs, field: "runtime operation IDs")
    try requireSortedUnique(systemAdapterIDs, field: "runtime system adapter IDs")

    let expectedCapabilities: Set<String> = [
        "analysis", "ascii", "jpeg", "layout-cytoscape", "layout-elk", "math", "pdf", "png", "svg",
        "system-clock", "system-random", "system-timezone",
    ]
    let expectedOutputs: Set<String> = ["ascii", "jpeg", "pdf", "png", "svg"]
    let expectedOperations: Set<String> = [
        "analysis-facts-json", "analysis-json", "ascii", "document-analysis-facts-json",
        "document-analysis-json", "jpeg", "layout-json", "pdf", "png", "semantic-json", "svg",
        "validation-json",
    ]
    guard expectedCapabilities.isSubset(of: Set(capabilityIDs)),
          expectedOutputs.isSubset(of: Set(outputIDs)),
          expectedOperations.isSubset(of: Set(operationIDs)),
          Set(outputIDs).isSubset(of: Set(operationIDs)),
          Set(systemAdapterIDs).isSubset(of: Set(capabilityIDs)) else {
        throw SmokeError.failed("runtime catalog has invalid native capability relations")
    }

    guard Set(textMeasurement.keys) == ["protocol_version", "provider_ids"],
          (integer(textMeasurement["protocol_version"]) ?? 0) > 0,
          let providerIDs = textMeasurement["provider_ids"] as? [String] else {
        throw SmokeError.failed("runtime catalog text measurement metadata was malformed")
    }
    try requireSortedUnique(providerIDs, field: "runtime text measurement provider IDs")
    guard providerIDs.contains("vendored") else {
        throw SmokeError.failed("runtime catalog text measurement lacks vendored support")
    }

    guard Set(registry.keys) == ["diagram_family_count"],
          (integer(registry["diagram_family_count"]) ?? -1) > 0 else {
        throw SmokeError.failed("runtime catalog registry was malformed")
    }
    let expectedResourceKeys: Set<String> = [
        "schema_version", "general_binding_default_profile", "cli_default_profile", "limits", "profiles",
    ]
    guard Set(resources.keys) == expectedResourceKeys,
          (integer(resources["schema_version"]) ?? 0) > 0,
          resources["general_binding_default_profile"] as? String == "interactive",
          resources["cli_default_profile"] as? String == "trusted-native",
          let limits = resources["limits"] as? [[String: Any]],
          let profiles = resources["profiles"] as? [[String: Any]],
          profiles.contains(where: { ($0["id"] as? String) == "constrained" }) else {
        throw SmokeError.failed("runtime catalog resource descriptors were malformed")
    }
    let requiredResourceIDs: Set<String> = [
        "max_layout_work_units",
        "max_model_items",
        "max_model_nesting_depth",
        "max_model_text_bytes",
        "max_source_bytes",
        "max_svg_bytes",
        "max_svg_elements",
    ]
    let resourceIDs = Set(limits.compactMap { $0["id"] as? String })
    guard requiredResourceIDs.isSubset(of: resourceIDs) else {
        throw SmokeError.failed("runtime catalog omitted a native SDK resource limit")
    }
}

private func requireSortedUnique(_ values: [String], field: String) throws {
    guard values == values.sorted(), Set(values).count == values.count else {
        throw SmokeError.failed("\(field) must be sorted and unique")
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

    func measure(request: MermanTextMeasureRequest) throws -> MermanTextMeasureResult? {
        lock.lock()
        calls += 1
        lock.unlock()
        return nil
    }
}
