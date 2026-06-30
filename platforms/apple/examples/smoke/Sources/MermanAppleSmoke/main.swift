import Foundation
import Merman

@main
struct MermanAppleSmoke {
    static func main() throws {
        let engine = try MermanEngine()
        let source = "flowchart TD\nA[Hello] --> B[World]"

        let svg = try engine.renderSvg(source)
        guard svg.contains("<svg"), svg.contains("Hello"), svg.contains("World") else {
            throw SmokeError.failed("SVG smoke failed")
        }

        let ascii = try engine.renderAscii(source)
        guard ascii.contains("Hello"), ascii.contains("World") else {
            throw SmokeError.failed("ASCII smoke failed")
        }

        let semanticJson = try engine.parseJsonRaw(source)
        guard semanticJson.contains("flowchart-v2") else {
            throw SmokeError.failed("semantic JSON smoke failed")
        }

        let layoutJson = try engine.layoutJsonRaw(source)
        guard layoutJson.contains("layout") else {
            throw SmokeError.failed("layout JSON smoke failed")
        }

        let validation = try engine.validate(source)
        guard validation.valid else {
            throw SmokeError.failed("validation smoke failed")
        }

        let reusable = try engine.reusableEngine()
        let callback: MermanTextMeasureCallback = { request, _ in
            let text = request.text.map {
                String(decoding: UnsafeBufferPointer(start: $0, count: request.text_len), as: UTF8.self)
            } ?? ""
            if text == "Hello" {
                return MermanTextMeasureResult(
                    handled: 1,
                    width: 42.0,
                    height: request.line_height,
                    line_count: 1
                )
            }
            return MermanTextMeasureResult(handled: 0, width: 0.0, height: 0.0, line_count: 0)
        }
        try reusable.setTextMeasureCallback(callback)
        let reusableSvg = try reusable.renderSvg(source)
        guard reusableSvg.contains("<svg"), reusableSvg.contains("Hello") else {
            throw SmokeError.failed("reusable renderSvg smoke failed")
        }
        reusable.close()

        guard try engine.supportedDiagrams().contains("flowchart") else {
            throw SmokeError.failed("supported diagrams smoke failed")
        }

        let ganttAsciiCapability = try engine.asciiCapabilities().contains { capability in
            capability.diagramType == "gantt"
                && capability.supportLevel == "summary"
                && !capability.summaryFallback
        }
        guard ganttAsciiCapability else {
            throw SmokeError.failed("ASCII capabilities smoke failed")
        }

        guard try engine.supportedThemes().contains("default") else {
            throw SmokeError.failed("themes smoke failed")
        }

        guard try engine.supportedHostThemePresets().contains("one-dark") else {
            throw SmokeError.failed("host theme presets smoke failed")
        }

        print("merman Apple Swift smoke passed (\(engine.packageVersion))")
    }
}

enum SmokeError: Error {
    case failed(String)
}
