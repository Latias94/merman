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

        let semanticJson = try engine.parseJsonRaw(source)
        guard semanticJson.contains("flowchart-v2") else {
            throw SmokeError.failed("semantic JSON smoke failed")
        }

        let layoutJson = try engine.layoutJsonRaw(source)
        guard layoutJson.contains("layout") else {
            throw SmokeError.failed("layout JSON smoke failed")
        }

        print("merman Apple Swift smoke passed (\(engine.packageVersion))")
    }
}

enum SmokeError: Error {
    case failed(String)
}
