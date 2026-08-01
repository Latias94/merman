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
            case let .Binding(_, _, kind, capabilityId, _, _):
                guard kind == .unknownOperation, capabilityId == nil else {
                    throw SmokeError.failed("unknown operation lost its machine-readable error fields")
                }
            }
        }

        do {
            _ = try engine.renderSvg(
                source: source,
                optionsJson: #"{"version":2,"resources":{"profile":"constrained","limits":{"max_source_bytes":8}}}"#
            )
            throw SmokeError.failed("resource failure did not return a binding error")
        } catch let error as MermanError {
            switch error {
            case let .Binding(_, codeName, _, _, resource, _):
                guard codeName == "MERMAN_RESOURCE_LIMIT_EXCEEDED",
                      resource?.limitId == "max_source_bytes",
                      resource?.phase == "source",
                      (resource?.actual ?? 0) > (resource?.max ?? UInt64.max),
                      resource?.profile == "constrained" else {
                    throw SmokeError.failed("resource failure lost its structured details")
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
    let requiredCatalogKeys: Set<String> = [
        "schema_version", "transport_api_version", "package_version", "capabilities", "output_contracts",
        "registry", "resources",
    ]
    guard requiredCatalogKeys.isSubset(of: Set(catalog.keys)),
          integer(catalog["schema_version"]) == 1,
          (integer(catalog["transport_api_version"]) ?? -1) == engine.bindingApiVersion(),
          catalog["package_version"] as? String == engine.packageVersion(),
          let capabilities = catalog["capabilities"] as? [String: Any],
          let outputContracts = catalog["output_contracts"] as? [[String: Any]],
          let registry = catalog["registry"] as? [String: Any],
          let resources = catalog["resources"] as? [String: Any] else {
        throw SmokeError.failed("runtime catalog did not describe the native SDK artifact")
    }

    let requiredCapabilityKeys: Set<String> = [
        "capability_ids", "output_ids", "operation_ids", "system_adapter_ids", "text_measurement",
    ]
    guard requiredCapabilityKeys.isSubset(of: Set(capabilities.keys)),
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
        "svg-plan-json", "validation-json",
    ]
    guard expectedCapabilities == Set(capabilityIDs),
          expectedOutputs == Set(outputIDs),
          expectedOperations == Set(operationIDs),
          Set(systemAdapterIDs) == [
              "system-clock", "system-random", "system-timezone",
          ],
          Set(outputIDs).isSubset(of: Set(operationIDs)),
          Set(systemAdapterIDs).isSubset(of: Set(capabilityIDs)) else {
        throw SmokeError.failed("runtime catalog has invalid native capability relations")
    }
    try validateRuntimeOutputContracts(outputContracts, outputIDs: outputIDs)

    let requiredTextMeasurementKeys: Set<String> = ["protocol_version", "provider_ids"]
    guard requiredTextMeasurementKeys.isSubset(of: Set(textMeasurement.keys)),
          (integer(textMeasurement["protocol_version"]) ?? 0) > 0,
          let providerIDs = textMeasurement["provider_ids"] as? [String] else {
        throw SmokeError.failed("runtime catalog text measurement metadata was malformed")
    }
    try requireSortedUnique(providerIDs, field: "runtime text measurement provider IDs")
    guard providerIDs.contains("vendored") else {
        throw SmokeError.failed("runtime catalog text measurement lacks vendored support")
    }

    let requiredRegistryKeys: Set<String> = ["diagram_family_count"]
    guard requiredRegistryKeys.isSubset(of: Set(registry.keys)),
          (integer(registry["diagram_family_count"]) ?? -1) > 0 else {
        throw SmokeError.failed("runtime catalog registry was malformed")
    }
    let requiredResourceKeys: Set<String> = [
        "general_binding_default_profile", "cli_default_profile", "limits", "profiles",
    ]
    guard requiredResourceKeys.isSubset(of: Set(resources.keys)),
          resources["general_binding_default_profile"] as? String == "interactive",
          resources["cli_default_profile"] as? String == "trusted-native",
          let limits = resources["limits"] as? [[String: Any]],
          let profiles = resources["profiles"] as? [[String: Any]],
          profiles.contains(where: { ($0["id"] as? String) == "constrained" }) else {
        throw SmokeError.failed("runtime catalog resource descriptors were malformed")
    }
    let requiredResourceIDs: Set<String> = [
        "max_ascii_grid_cells",
        "max_document_diagrams",
        "max_embedded_image_bytes",
        "max_embedded_image_pixels",
        "max_layout_work_units",
        "max_model_items",
        "max_model_nesting_depth",
        "max_model_text_bytes",
        "max_nested_svg_images",
        "max_raster_height",
        "max_raster_pixels",
        "max_raster_width",
        "max_source_bytes",
        "max_svg_bytes",
        "max_svg_conversion_filter_primitives_per_filter",
        "max_svg_conversion_isolation_depth",
        "max_svg_conversion_subroots",
        "max_svg_elements",
        "max_total_embedded_image_bytes",
        "max_total_embedded_image_pixels",
        "max_total_svg_conversion_filter_primitives",
        "svg_backend_tree_nodes",
    ]
    let resourceIDs = Set(limits.compactMap { $0["id"] as? String })
    guard requiredResourceIDs.isSubset(of: resourceIDs) else {
        throw SmokeError.failed("runtime catalog omitted a native SDK resource limit")
    }
}

private func validateRuntimeOutputContracts(
    _ contracts: [[String: Any]],
    outputIDs: [String]
) throws {
    let requiredContractKeys: Set<String> = ["id", "media_type", "system_fonts", "embedded_images"]
    var contractIDs: [String] = []
    for contract in contracts {
        guard requiredContractKeys.isSubset(of: Set(contract.keys)),
              let id = contract["id"] as? String,
              !id.isEmpty,
              let mediaType = contract["media_type"] as? String,
              !mediaType.isEmpty else {
            throw SmokeError.failed("runtime output contract was malformed")
        }
        contractIDs.append(id)
        try validateSystemFontContract(contract["system_fonts"], outputID: id)
        try validateEmbeddedImageContract(contract["embedded_images"], outputID: id)
        try validateNativeOutputEnvironmentFacts(contract, outputID: id, mediaType: mediaType)
    }
    try requireSortedUnique(contractIDs, field: "runtime output contract IDs")
    guard contractIDs == outputIDs else {
        throw SmokeError.failed("runtime output contract IDs did not match runtime output IDs")
    }
}

private func validateSystemFontContract(_ value: Any?, outputID: String) throws {
    if value is NSNull { return }
    guard let contract = value as? [String: Any] else {
        throw SmokeError.failed("\(outputID) system font contract was not an object or null")
    }
    let requiredKeys: Set<String> = [
        "source_id", "discovery", "cache_scope", "host_dependent", "caller_configurable",
        "resource_bounded",
    ]
    guard requiredKeys.isSubset(of: Set(contract.keys)),
          let sourceID = contract["source_id"] as? String,
          !sourceID.isEmpty,
          let discovery = contract["discovery"] as? String,
          !discovery.isEmpty,
          let cacheScope = contract["cache_scope"] as? String,
          !cacheScope.isEmpty,
          boolean(contract["host_dependent"]) != nil,
          boolean(contract["caller_configurable"]) != nil,
          boolean(contract["resource_bounded"]) != nil else {
        throw SmokeError.failed("\(outputID) system font contract was malformed")
    }
}

private func validateEmbeddedImageContract(_ value: Any?, outputID: String) throws {
    if value is NSNull { return }
    guard let contract = value as? [String: Any] else {
        throw SmokeError.failed("\(outputID) embedded image contract was not an object or null")
    }
    let requiredKeys: Set<String> = [
        "source_ids", "filesystem_access", "network_access", "caller_configurable", "limits",
    ]
    guard requiredKeys.isSubset(of: Set(contract.keys)),
          let sourceIDs = contract["source_ids"] as? [String],
          boolean(contract["filesystem_access"]) != nil,
          boolean(contract["network_access"]) != nil,
          boolean(contract["caller_configurable"]) != nil,
          let limits = contract["limits"] as? [String: Any] else {
        throw SmokeError.failed("\(outputID) embedded image contract was malformed")
    }
    try requireSortedUnique(sourceIDs, field: "\(outputID) embedded image source IDs")

    let limitKeys: Set<String> = [
        "max_bytes_per_image", "max_total_bytes", "max_pixels_per_image", "max_total_pixels",
    ]
    guard limitKeys.isSubset(of: Set(limits.keys)),
          limitKeys.allSatisfy({ isNullOrPositiveInteger(limits[$0]) }) else {
        throw SmokeError.failed("\(outputID) embedded image limits were malformed")
    }
}

private func validateNativeOutputEnvironmentFacts(
    _ contract: [String: Any],
    outputID: String,
    mediaType: String
) throws {
    let expectedMediaTypes = [
        "ascii": "text/plain; charset=utf-8",
        "jpeg": "image/jpeg",
        "pdf": "application/pdf",
        "png": "image/png",
        "svg": "image/svg+xml",
    ]
    guard expectedMediaTypes[outputID] == mediaType else {
        throw SmokeError.failed("\(outputID) runtime media type drifted")
    }
    if outputID == "ascii" || outputID == "svg" {
        guard contract["system_fonts"] is NSNull,
              contract["embedded_images"] is NSNull else {
            throw SmokeError.failed("\(outputID) unexpectedly declared binary-export resources")
        }
        return
    }

    guard let fonts = contract["system_fonts"] as? [String: Any],
          fonts["source_id"] as? String == "host-system",
          fonts["discovery"] as? String == "first-use",
          fonts["cache_scope"] as? String == "process-global",
          boolean(fonts["host_dependent"]) == true,
          boolean(fonts["caller_configurable"]) == false,
          boolean(fonts["resource_bounded"]) == false,
          let images = contract["embedded_images"] as? [String: Any],
          images["source_ids"] as? [String] == ["data-url"],
          boolean(images["filesystem_access"]) == false,
          boolean(images["network_access"]) == false,
          boolean(images["caller_configurable"]) == true,
          let limits = images["limits"] as? [String: Any],
          positiveInteger(limits["max_bytes_per_image"]) == 16 * 1024 * 1024,
          positiveInteger(limits["max_total_bytes"]) == 32 * 1024 * 1024,
          positiveInteger(limits["max_pixels_per_image"]) == 16 * 1024 * 1024,
          positiveInteger(limits["max_total_pixels"]) == 32 * 1024 * 1024 else {
        throw SmokeError.failed("\(outputID) binary export environment facts drifted")
    }
}

private func boolean(_ value: Any?) -> Bool? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) == CFBooleanGetTypeID() else {
        return nil
    }
    return number.boolValue
}

private func positiveInteger(_ value: Any?) -> UInt64? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          !CFNumberIsFloatType(number),
          let result = UInt64(number.stringValue),
          result > 0 else {
        return nil
    }
    return result
}

private func isNullOrPositiveInteger(_ value: Any?) -> Bool {
    value is NSNull || positiveInteger(value) != nil
}

private func requireSortedUnique(_ values: [String], field: String) throws {
    guard values.allSatisfy({ !$0.isEmpty }),
          values == values.sorted(),
          Set(values).count == values.count else {
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
