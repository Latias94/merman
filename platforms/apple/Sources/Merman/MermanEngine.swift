import Foundation
import MermanFFI

public enum MermanError: Error, LocalizedError {
    case abiMismatch(expected: UInt32, actual: UInt32)
    case structSizeMismatch(name: String, expected: Int, actual: Int)
    case binding(code: Int32, codeName: String, message: String)
    case utf8Output

    public var errorDescription: String? {
        switch self {
        case let .abiMismatch(expected, actual):
            return "Merman ABI mismatch: expected \(expected), got \(actual)"
        case let .structSizeMismatch(name, expected, actual):
            return "Merman ABI struct size mismatch for \(name): expected \(expected), got \(actual)"
        case let .binding(_, codeName, message):
            return "\(codeName): \(message)"
        case .utf8Output:
            return "Merman native output was not UTF-8"
        }
    }
}

public final class MermanEngine {
    public static let abiVersion: UInt32 = 1
    private static let okCode: Int32 = 0

    public let packageVersion: String

    public init() throws {
        try Self.checkAbi()
        packageVersion = String(cString: merman_package_version())
    }

    public func renderSvg(_ source: String, optionsJson: String? = nil) throws -> String {
        try call(merman_render_svg, source: source, optionsJson: optionsJson)
    }

    public func parseJsonRaw(_ source: String, optionsJson: String? = nil) throws -> String {
        try call(merman_parse_json, source: source, optionsJson: optionsJson)
    }

    public func layoutJsonRaw(_ source: String, optionsJson: String? = nil) throws -> String {
        try call(merman_layout_json, source: source, optionsJson: optionsJson)
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
}

private struct NativeErrorPayload: Decodable {
    let codeName: String
    let message: String

    enum CodingKeys: String, CodingKey {
        case codeName = "code_name"
        case message
    }
}
