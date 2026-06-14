import Foundation
import MermanFFI

public enum MermanError: Error, LocalizedError {
    case abiMismatch(expected: UInt32, actual: UInt32)
    case structSizeMismatch(name: String, expected: Int, actual: Int)
    case binding(code: Int32, codeName: String, message: String)
    case jsonDecode(message: String)
    case utf8Output

    public var errorDescription: String? {
        switch self {
        case let .abiMismatch(expected, actual):
            return "Merman ABI mismatch: expected \(expected), got \(actual)"
        case let .structSizeMismatch(name, expected, actual):
            return "Merman ABI struct size mismatch for \(name): expected \(expected), got \(actual)"
        case let .binding(_, codeName, message):
            return "\(codeName): \(message)"
        case let .jsonDecode(message):
            return message
        case .utf8Output:
            return "Merman native output was not UTF-8"
        }
    }
}

public struct MermanValidationResult: Decodable {
    public let valid: Bool
    public let error: String?
    public let code: Int32
    public let codeName: String

    enum CodingKeys: String, CodingKey {
        case valid
        case error
        case code
        case codeName = "code_name"
    }
}

public struct MermanDiagramFamilyCapability: Decodable {
    public let diagramType: String
    public let metadataId: String?
    public let hasSemanticParser: Bool
    public let hasRenderParser: Bool

    enum CodingKeys: String, CodingKey {
        case diagramType = "diagram_type"
        case metadataId = "metadata_id"
        case hasSemanticParser = "has_semantic_parser"
        case hasRenderParser = "has_render_parser"
    }
}

public final class MermanEngine {
    public static let abiVersion: UInt32 = 1
    private static let okCode: Int32 = 0

    public let packageVersion: String
    private var supportedDiagramsCache: [String]?
    private var asciiSupportedDiagramsCache: [String]?
    private var diagramFamilyCapabilitiesCache: [MermanDiagramFamilyCapability]?
    private var themesCache: [String]?
    private var hostThemePresetsCache: [String]?

    public init() throws {
        try Self.checkAbi()
        packageVersion = String(cString: merman_package_version())
    }

    public func renderSvg(_ source: String, optionsJson: String? = nil) throws -> String {
        try call(merman_render_svg, source: source, optionsJson: optionsJson)
    }

    public func renderAscii(_ source: String, optionsJson: String? = nil) throws -> String {
        try call(merman_render_ascii, source: source, optionsJson: optionsJson)
    }

    public func parseJsonRaw(_ source: String, optionsJson: String? = nil) throws -> String {
        try call(merman_parse_json, source: source, optionsJson: optionsJson)
    }

    public func layoutJsonRaw(_ source: String, optionsJson: String? = nil) throws -> String {
        try call(merman_layout_json, source: source, optionsJson: optionsJson)
    }

    public func validateJsonRaw(_ source: String, optionsJson: String? = nil) throws -> String {
        try call(merman_validate_json, source: source, optionsJson: optionsJson)
    }

    public func validate(_ source: String, optionsJson: String? = nil) throws -> MermanValidationResult {
        let data = try Data(validateJsonRaw(source, optionsJson: optionsJson).utf8)
        return try decodeJson(MermanValidationResult.self, from: data)
    }

    public func supportedDiagrams() throws -> [String] {
        if let supportedDiagramsCache {
            return supportedDiagramsCache
        }
        let values = try metadata(merman_supported_diagrams_json)
        supportedDiagramsCache = values
        return values
    }

    public func asciiSupportedDiagrams() throws -> [String] {
        if let asciiSupportedDiagramsCache {
            return asciiSupportedDiagramsCache
        }
        let values = try metadata(merman_ascii_supported_diagrams_json)
        asciiSupportedDiagramsCache = values
        return values
    }

    public func diagramFamilyCapabilities() throws -> [MermanDiagramFamilyCapability] {
        if let diagramFamilyCapabilitiesCache {
            return diagramFamilyCapabilitiesCache
        }
        let text = try decode(merman_diagram_family_capabilities_json())
        let values = try decodeJson([MermanDiagramFamilyCapability].self, from: Data(text.utf8))
        diagramFamilyCapabilitiesCache = values
        return values
    }

    public func supportedThemes() throws -> [String] {
        if let themesCache {
            return themesCache
        }
        let values = try metadata(merman_supported_themes_json)
        themesCache = values
        return values
    }

    public func supportedHostThemePresets() throws -> [String] {
        if let hostThemePresetsCache {
            return hostThemePresetsCache
        }
        let values = try metadata(merman_supported_host_theme_presets_json)
        hostThemePresetsCache = values
        return values
    }

    private static func checkAbi() throws {
        let actualAbi = merman_abi_version()
        guard actualAbi == abiVersion else {
            throw MermanError.abiMismatch(expected: abiVersion, actual: actualAbi)
        }

        let expectedBufferSize = MemoryLayout<MermanBuffer>.size
        let actualBufferSize = merman_buffer_struct_size()
        guard actualBufferSize == expectedBufferSize else {
            throw MermanError.structSizeMismatch(
                name: "MermanBuffer",
                expected: expectedBufferSize,
                actual: actualBufferSize
            )
        }

        let expectedResultSize = MemoryLayout<MermanResult>.size
        let actualResultSize = merman_result_struct_size()
        guard actualResultSize == expectedResultSize else {
            throw MermanError.structSizeMismatch(
                name: "MermanResult",
                expected: expectedResultSize,
                actual: actualResultSize
            )
        }
    }

    private func call(
        _ function: (
            UnsafePointer<UInt8>?,
            Int,
            UnsafePointer<UInt8>?,
            Int
        ) -> MermanResult,
        source: String,
        optionsJson: String?
    ) throws -> String {
        let sourceBytes = Array(source.utf8)
        let optionBytes = Array((optionsJson ?? "").utf8)

        return try sourceBytes.withUnsafeBufferPointer { sourceBuffer in
            try optionBytes.withUnsafeBufferPointer { optionBuffer in
                let sourcePointer = sourceBytes.isEmpty ? nil : sourceBuffer.baseAddress
                let optionPointer = optionBytes.isEmpty ? nil : optionBuffer.baseAddress
                let result = function(
                    sourcePointer,
                    sourceBytes.count,
                    optionPointer,
                    optionBytes.count
                )
                return try decode(result)
            }
        }
    }

    private func decode(_ result: MermanResult) throws -> String {
        defer { merman_buffer_free(result.data) }

        let payload: Data
        if let pointer = result.data.data, result.data.len > 0 {
            payload = Data(bytes: pointer, count: result.data.len)
        } else {
            payload = Data()
        }

        guard let text = String(data: payload, encoding: .utf8) else {
            throw MermanError.utf8Output
        }

        if result.code == Self.okCode {
            return text
        }

        if let errorPayload = try? JSONDecoder().decode(NativeErrorPayload.self, from: payload) {
            throw MermanError.binding(
                code: result.code,
                codeName: errorPayload.codeName,
                message: errorPayload.message
            )
        }

        throw MermanError.binding(
            code: result.code,
            codeName: "MERMAN_ERROR",
            message: text
        )
    }

    private func metadata(_ function: () -> MermanResult) throws -> [String] {
        let text = try decode(function())
        return try decodeJson([String].self, from: Data(text.utf8))
    }

    private func decodeJson<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        do {
            return try JSONDecoder().decode(type, from: data)
        } catch {
            throw MermanError.jsonDecode(message: "Merman JSON decode failed: \(error)")
        }
    }
}

private struct NativeErrorPayload: Decodable {
    let codeName: String
    let message: String

    enum CodingKeys: String, CodingKey {
        case codeName = "code_name"
        case message
    }
}
