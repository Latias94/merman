import Foundation
import Merman

func verifyThemeV2(client: Merman, engine: MermanEngine) throws {
    let catalogJson = try client.themeCatalogJson()
    let engineCatalogJson = try engine.themeCatalogJson()
    let catalog = try themeObject(Data(catalogJson.utf8))
    guard catalogJson == engineCatalogJson,
          catalog["schema_version"] as? Int == 2,
          let presets = catalog["presets"] as? [[String: Any]], presets.count == 13,
          (catalog["mermaid_themes"] as? [String])?.count == 11,
          let families = catalog["families"] as? [[String: Any]],
          families.contains(where: { $0["id"] as? String == "flowchart" && ($0["capabilities"] as? Int ?? 0) > 0 }),
          let studio = presets.first(where: { $0["id"] as? String == "studio" }),
          studio["appearances"] as? [String] == ["light", "dark"],
          studio["default_appearance"] as? String == "light",
          (studio["swatches"] as? [String])?.count == 6 else {
        throw ThemeSmokeError.failed("unexpected theme v2 catalog")
    }

    let consumers: [(String, (MermanOperationRequestV4) throws -> MermanOperationResult)] = [
        ("one-shot", client.execute(request:)),
        ("reusable", engine.execute(request:)),
    ]
    for (name, execute) in consumers {
        func run(_ operationId: String, _ source: String, options: [String: Any]? = nil) throws -> MermanOperationResult {
            try execute(MermanOperationRequestV4(
                operationId: operationId, source: source, uri: nil,
                optionsJson: options.map(themeJSON), control: nil
            ))
        }
        func resolve(_ document: Any, appearance: String? = nil) throws -> [String: Any] {
            var request = ["document": document]
            if let appearance { request["appearance"] = appearance }
            return try themeObject(run("resolve-theme", themeJSON(request)).data)
        }

        let exported = try run("export-theme-preset", "studio")
        let document = try themeObject(exported.data)
        guard exported.operationId == "export-theme-preset", exported.mediaType == "application/json",
              document["schema_version"] as? Int == 2, document["id"] as? String == "studio",
              let appearances = document["appearances"] as? [String: Any],
              Set(appearances.keys) == Set(["light", "dark"]) else {
            throw ThemeSmokeError.failed("\(name) did not export the editable preset")
        }
        let restored = String(decoding: exported.data, as: UTF8.self)
        for input in [document, restored] as [Any] {
            let resolved = try resolve(input, appearance: "dark")
            guard resolved["ok"] as? Bool == true,
                  (resolved["resolved"] as? [String: Any])?["appearance"] as? String == "dark",
                  resolved["notices"] is [Any] else {
                throw ThemeSmokeError.failed("\(name) could not resolve Studio Dark")
            }
        }

        for (text, column, path, message) in [
            ("{\n  \"schema_version\": 2,\n  \"extends\": ]\n}", 14, "extends", "expected value"),
            ("{\n  \"schema_version\": 2,\n  \"schema_version\": 2\n}", 18, ".", "duplicate key"),
        ] {
            let result = try resolve(text)
            guard result["ok"] as? Bool == false, result["line"] as? Int == 3,
                  result["column"] as? Int == column, result["path"] as? String == path,
                  (result["message"] as? String)?.contains(message) == true else {
                throw ThemeSmokeError.failed("\(name) lost document-relative diagnostics: \(result)")
            }
        }

        let source = "---\nconfig:\n  layout: dagre\n---\nflowchart LR\nA[Review] --> B[Publish]"
        let rendered = try run("svg", source, options: [
            "theme": ["document": document, "appearance": "dark"],
            "background": "#123456",
        ])
        let metadata = try themeObject(Data(rendered.metadata.rawJson.utf8))
        guard let outcome = metadata["theme_outcome"] as? [String: Any],
              outcome["mode"] as? String == "merman_native",
              outcome["appearance"] as? String == "dark",
              outcome["notices"] is [Any] else {
            throw ThemeSmokeError.failed("\(name) lost the SVG theme outcome")
        }
        let svg = try XMLDocument(data: rendered.data, options: [.nodeLoadExternalEntitiesNever])
        let fills = try svg.nodes(forXPath:
            "//*[local-name()='g' and @class='merman-theme-canvas']/*[local-name()='rect'][1]/@fill"
        )
        guard fills.first?.stringValue?.lowercased() == "#123456" else {
            throw ThemeSmokeError.failed("\(name) background did not replace the theme canvas")
        }
    }
    print("Apple Theme v2 authoring smoke passed")
}

func themeJSON(_ value: [String: Any]) throws -> String {
    String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
}

func themeObject(_ data: Data) throws -> [String: Any] {
    guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        throw ThemeSmokeError.failed("expected a JSON object")
    }
    return object
}

private enum ThemeSmokeError: Error {
    case failed(String)
}
