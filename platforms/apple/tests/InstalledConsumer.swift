import Foundation
import Merman

@main
struct InstalledConsumer {
    static func main() throws {
        let client = Merman()
        precondition(client.bindingApiVersionV7() == 7)
        let engine = try MermanEngine(
            optionsJson: resourceOptionsJson(profile: .interactive, overrides: []), services: nil
        )
        let basicSource = "---\nconfig:\n  layout: elk\n---\nflowchart TD\nA[Hello] --> B[World]"
        let mathSource = "flowchart TD\nA[\"$$x^2$$\"] --> B[Done]"
        func render(_ source: String) throws -> [String: Any] {
            let result = try engine.execute(request: MermanOperationRequestV4(
                operationId: "svg", source: source, uri: nil,
                optionsJson: #"{"svg":{"pipeline":"resvg-safe","diagram_id":"apple-installed"}}"#,
                control: nil
            ))
            guard let svg = String(data: result.data, encoding: .utf8) else {
                throw ConsumerError.failed("SVG is not UTF-8")
            }
            return [
                "svg": svg,
                "metadata": try JSONSerialization.jsonObject(with: Data(result.metadata.rawJson.utf8)),
            ]
        }
        let basic = try render(basicSource)
        let mathematical = try render(mathSource)
        func requireMissing(_ capability: String, _ operation: () throws -> Void) throws {
            do {
                try operation()
            } catch let error as MermanError {
                switch error {
                case let .Binding(_, _, kind, id, _, _, _, _, _, _):
                    guard kind == .missingCapability, id == capability else {
                        throw ConsumerError.failed("unexpected rejection for \(capability)")
                    }
                    return
                }
            }
            throw ConsumerError.failed("expected capability rejection for \(capability)")
        }
        try requireMissing("png") { _ = try engine.renderPng(source: basicSource, optionsJson: nil) }
        try requireMissing("jpeg") { _ = try engine.renderJpeg(source: basicSource, optionsJson: nil) }
        try requireMissing("pdf") { _ = try engine.renderPdf(source: basicSource, optionsJson: nil) }
        let constrained = try MermanEngine(
            optionsJson: resourceOptionsJson(profile: .constrained, overrides: []), services: nil
        )
        try requireMissing("math") { _ = try constrained.renderSvg(source: mathSource, optionsJson: nil) }
        try constrained.close()
        try engine.close()
        let output: [String: Any] = [
            "catalog": try JSONSerialization.jsonObject(with: Data(client.runtimeCatalogJson().utf8)),
            "basic": basic,
            "math": mathematical,
            "rejected_capability_ids": ["jpeg", "math", "pdf", "png"],
        ]
        let data = try JSONSerialization.data(withJSONObject: output, options: [.sortedKeys])
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data([10]))
    }
}

private enum ConsumerError: Error {
    case failed(String)
}
