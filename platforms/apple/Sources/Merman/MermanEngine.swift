import Foundation
import MermanFFI

public typealias MermanTextMeasureRequest = MermanHostTextMeasureRequest
public typealias MermanTextMeasureResult = MermanHostTextMeasureResult
public typealias MermanTextMeasureCallback = MermanHostTextMeasureCallback

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

public struct MermanAsciiCapabilityEvidence: Decodable {
    public let kind: String
    public let source: String
    public let note: String
}

public struct MermanAsciiCapability: Decodable {
    public let diagramType: String
    public let displayName: String
    public let supportLevel: String
    public let summaryFallback: Bool
    public let supportedSemantics: [String]
    public let limits: [String]
    public let evidence: [MermanAsciiCapabilityEvidence]

    enum CodingKeys: String, CodingKey {
        case diagramType = "diagram_type"
        case displayName = "display_name"
        case supportLevel = "support_level"
        case summaryFallback = "summary_fallback"
        case supportedSemantics = "supported_semantics"
        case limits
        case evidence
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
    public static let abiVersion: UInt32 = 2
    private static let okCode: Int32 = 0

    public let packageVersion: String
    private var supportedDiagramsCache: [String]?
    private var asciiCapabilitiesCache: [MermanAsciiCapability]?
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

    public func asciiCapabilities() throws -> [MermanAsciiCapability] {
        if let asciiCapabilitiesCache {
            return asciiCapabilitiesCache
        }
        let text = try decode(merman_ascii_capabilities_json())
        let values = try decodeJson([MermanAsciiCapability].self, from: Data(text.utf8))
        asciiCapabilitiesCache = values
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

    public func reusableEngine(optionsJson: String? = nil) throws -> MermanReusableEngine {
        try MermanReusableEngine(optionsJson: optionsJson)
    }

    fileprivate static func checkAbi() throws {
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

        let expectedEngineResultSize = MemoryLayout<MermanEngineResult>.size
        let actualEngineResultSize = merman_engine_result_struct_size()
        guard actualEngineResultSize == expectedEngineResultSize else {
            throw MermanError.structSizeMismatch(
                name: "MermanEngineResult",
                expected: expectedEngineResultSize,
                actual: actualEngineResultSize
            )
        }

        let expectedTextRequestSize = MemoryLayout<MermanHostTextMeasureRequest>.size
        let actualTextRequestSize = merman_host_text_measure_request_struct_size()
        guard actualTextRequestSize == expectedTextRequestSize else {
            throw MermanError.structSizeMismatch(
                name: "MermanHostTextMeasureRequest",
                expected: expectedTextRequestSize,
                actual: actualTextRequestSize
            )
        }

        let expectedTextResultSize = MemoryLayout<MermanHostTextMeasureResult>.size
        let actualTextResultSize = merman_host_text_measure_result_struct_size()
        guard actualTextResultSize == expectedTextResultSize else {
            throw MermanError.structSizeMismatch(
                name: "MermanHostTextMeasureResult",
                expected: expectedTextResultSize,
                actual: actualTextResultSize
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

    fileprivate static func decode(_ result: MermanResult) throws -> String {
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

        if result.code == okCode {
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

    private func decode(_ result: MermanResult) throws -> String {
        try Self.decode(result)
    }

    private func metadata(_ function: () -> MermanResult) throws -> [String] {
        let text = try Self.decode(function())
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

public final class MermanReusableEngine {
    private var engine: OpaquePointer?

    public init(optionsJson: String? = nil) throws {
        try MermanEngine.checkAbi()

        let optionBytes = Array((optionsJson ?? "").utf8)
        let result = optionBytes.withUnsafeBufferPointer { optionBuffer in
            merman_engine_new(
                optionBytes.isEmpty ? nil : optionBuffer.baseAddress,
                optionBytes.count
            )
        }

        if result.code == 0, let engine = result.engine {
            self.engine = engine
            merman_buffer_free(result.data)
            return
        }

        _ = result.engine.map(merman_engine_free)
        throw try Self.decodeEngineError(result)
    }

    deinit {
        close()
    }

    public func setTextMeasureCallback(
        _ callback: MermanTextMeasureCallback?,
        userData: UnsafeMutableRawPointer? = nil
    ) throws {
        let engine = try requireEngine()
        _ = try MermanEngine.decode(
            merman_engine_set_text_measure_callback(engine, callback, userData)
        )
    }

    public func renderSvg(_ source: String) throws -> String {
        try call(merman_engine_render_svg, source: source)
    }

    public func renderAscii(_ source: String) throws -> String {
        try call(merman_engine_render_ascii, source: source)
    }

    public func parseJsonRaw(_ source: String) throws -> String {
        try call(merman_engine_parse_json, source: source)
    }

    public func layoutJsonRaw(_ source: String) throws -> String {
        try call(merman_engine_layout_json, source: source)
    }

    public func validateJsonRaw(_ source: String) throws -> String {
        try call(merman_engine_validate_json, source: source)
    }

    public func validate(_ source: String) throws -> MermanValidationResult {
        let data = try Data(validateJsonRaw(source).utf8)
        do {
            return try JSONDecoder().decode(MermanValidationResult.self, from: data)
        } catch {
            throw MermanError.jsonDecode(message: "Merman JSON decode failed: \(error)")
        }
    }

    public func close() {
        guard let engine else {
            return
        }
        merman_engine_free(engine)
        self.engine = nil
    }

    private func call(
        _ function: (OpaquePointer?, UnsafePointer<UInt8>?, Int) -> MermanResult,
        source: String
    ) throws -> String {
        let engine = try requireEngine()
        let sourceBytes = Array(source.utf8)
        return try sourceBytes.withUnsafeBufferPointer { sourceBuffer in
            try MermanEngine.decode(
                function(
                    engine,
                    sourceBytes.isEmpty ? nil : sourceBuffer.baseAddress,
                    sourceBytes.count
                )
            )
        }
    }

    private func requireEngine() throws -> OpaquePointer {
        guard let engine else {
            throw MermanError.binding(
                code: -1,
                codeName: "SWIFT_ENGINE_CLOSED",
                message: "Merman reusable engine is closed"
            )
        }
        return engine
    }

    private static func decodeEngineError(_ result: MermanEngineResult) throws -> MermanError {
        defer { merman_buffer_free(result.data) }

        let payload: Data
        if let pointer = result.data.data, result.data.len > 0 {
            payload = Data(bytes: pointer, count: result.data.len)
        } else {
            payload = Data()
        }

        guard let text = String(data: payload, encoding: .utf8) else {
            return .utf8Output
        }

        if let errorPayload = try? JSONDecoder().decode(NativeErrorPayload.self, from: payload) {
            return .binding(
                code: result.code,
                codeName: errorPayload.codeName,
                message: errorPayload.message
            )
        }

        return .binding(
            code: result.code,
            codeName: "MERMAN_ERROR",
            message: text
        )
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
