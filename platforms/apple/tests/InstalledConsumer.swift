import Foundation
import Merman

@main
struct InstalledConsumer {
    static func main() throws {
        let client = Merman()
        precondition(client.bindingApiVersionV8() == 8)
        let engine = try MermanEngine(
            optionsJson: resourceOptionsJson(profile: .interactive, overrides: []), services: nil
        )
        try verifyInstalledTheme(engine: engine)
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

private func verifyInstalledTheme(engine: MermanEngine) throws {
    let catalog = try JSONSerialization.jsonObject(with: Data(engine.themeCatalogJson().utf8)) as? [String: Any]
    guard catalog?["schema_version"] as? Int == 2,
          (catalog?["presets"] as? [[String: Any]])?.contains(where: { $0["id"] as? String == "studio" }) == true else {
        throw ConsumerError.failed("installed package lost the Theme v2 catalog")
    }
    let exported = try engine.execute(request: MermanOperationRequestV4(
        operationId: "export-theme-preset", source: "studio", uri: nil, optionsJson: nil, control: nil
    ))
    let envelope = try JSONSerialization.data(withJSONObject: [
        "document": String(decoding: exported.data, as: UTF8.self), "appearance": "dark",
    ])
    let resolution = try engine.execute(request: MermanOperationRequestV4(
        operationId: "resolve-theme", source: String(decoding: envelope, as: UTF8.self),
        uri: nil, optionsJson: nil, control: nil
    ))
    let resolved = try JSONSerialization.jsonObject(with: resolution.data) as? [String: Any]
    guard resolved?["ok"] as? Bool == true,
          (resolved?["resolved"] as? [String: Any])?["appearance"] as? String == "dark" else {
        throw ConsumerError.failed("installed package could not resolve Studio Dark")
    }

    let source = "---\nconfig:\n  layout: dagre\n---\nflowchart LR\nA --> B"
    let rendered = try engine.execute(request: MermanOperationRequestV4(
        operationId: "svg", source: source, uri: nil,
        optionsJson: #"{"theme":{"preset":"studio","appearance":"dark"}}"#,
        control: nil
    ))
    let metadata = try JSONSerialization.jsonObject(with: Data(rendered.metadata.rawJson.utf8)) as? [String: Any]
    guard let outcome = metadata?["theme_outcome"] as? [String: Any],
          outcome["mode"] as? String == "merman_native",
          outcome["preset_id"] as? String == "studio",
          outcome["appearance"] as? String == "dark" else {
        throw ConsumerError.failed("installed package lost the native SVG theme outcome")
    }
}

private enum ConsumerError: Error {
    case failed(String)
}
