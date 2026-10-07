import Foundation
import Merman

private let repositoryRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().appendingPathComponent("../../../../../..")
    .standardizedFileURL

private func requireRecipe(_ condition: Bool, _ message: String) throws {
    if !condition { throw RecipeSmokeError.failed(message) }
}

private func encodeRecipeJSON(_ value: Any) throws -> Data {
    try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
}

private func recipeSources() throws -> [(String, String)] {
    var sources = [("class", "classDiagram\nclass Account {\n+String name\n+save()\n}\n")]
    for family in ["flowchart", "sequence", "xychart"] {
        let file = repositoryRoot.appendingPathComponent(
            "crates/merman-theme-fixtures/fixtures/public-cyberpunk/\(family).mmd"
        )
        sources.append((family, try String(contentsOf: file, encoding: .utf8)))
    }
    return sources
}

private func recipeOptions(_ theme: Any, family: String) throws -> String {
    // Cyberpunk Flowchart effects are qualified on the classic writer. The family-scoped
    // setting leaves the other recipes on their default Mermaid looks.
    String(decoding: try encodeRecipeJSON([
        "version": 3, "theme": theme,
        "site_config": ["htmlLabels": false, "flowchart": ["look": "classic"]],
        "svg": ["diagram_id": "apple-recipe-\(family)"],
    ]), as: UTF8.self)
}

// A separate invocation consumes the exported envelope directly, without preset regeneration.
func importThemeRecipeInFreshProcess() throws -> Bool {
    guard CommandLine.arguments.dropFirst().first == "--import-theme-recipe" else { return false }
    try requireRecipe(CommandLine.arguments.count == 4, "expected recipe file and output directory")
    let file = URL(fileURLWithPath: CommandLine.arguments[2])
    let output = URL(fileURLWithPath: CommandLine.arguments[3], isDirectory: true)
    let recipe = try JSONSerialization.jsonObject(with: Data(contentsOf: file))
    try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
    let client = Merman()
    for (family, source) in try recipeSources() {
        let result = try client.execute(request: MermanOperationRequestV4(
            operationId: "svg", source: source, uri: nil,
            optionsJson: recipeOptions(recipe, family: family), control: nil
        ))
        let document = try XMLDocument(data: result.data, options: [.nodeLoadExternalEntitiesNever])
        try requireRecipe(result.operationId == "svg" && result.mediaType == "image/svg+xml"
                          && document.rootElement()?.localName == "svg", "fresh consumer returned invalid SVG")
        try result.data.write(to: output.appendingPathComponent("\(family).svg"))
        try Data(result.metadata.rawJson.utf8).write(to: output.appendingPathComponent("\(family).metadata.json"))
    }
    return true
}

func verifyThemePresetRecipes(client: Merman) throws {
    let fixture = repositoryRoot.appendingPathComponent(
        "crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/preset-catalog.json"
    )
    let expected = try JSONSerialization.jsonObject(with: Data(contentsOf: fixture))
    let engine = try MermanEngine(optionsJson: nil, services: nil)
    defer { try? engine.close() }
    for catalogJSON in [try client.themeCatalogJson(), try engine.themeCatalogJson()] {
        let catalog = try JSONSerialization.jsonObject(with: Data(catalogJSON.utf8)) as! [String: Any]
        try requireRecipe(catalog["schema_version"] as? Int == 1, "catalog schema changed")
        try requireRecipe(catalog["structured_spec_available"] as? Bool == true, "spec unavailable")
        try requireRecipe(encodeRecipeJSON(catalog["presets"]!) == encodeRecipeJSON(expected),
                          "preset catalog differs from shared golden")
    }
    let sources = try recipeSources()
    let consumers: [(String, (MermanOperationRequestV4) throws -> MermanOperationResult)] = [
        ("one-shot", { try client.execute(request: $0) }),
        ("reusable", { try engine.execute(request: $0) }),
    ]
    var cyberpunk: [String: Any] = [:]
    for (mode, execute) in consumers {
        let widthQuery: [String: Any] = [
            "schema_version": 1, "family": "class", "output": "standalone-svg",
            "subject": ["kind": "rule", "target": "node", "facet": "stroke-width"],
        ]
        let support = try execute(MermanOperationRequestV4(
            operationId: "describe-theme-support-json",
            source: String(decoding: encodeRecipeJSON(widthQuery), as: UTF8.self),
            uri: nil, optionsJson: nil, control: nil
        ))
        let widthSupport = try JSONSerialization.jsonObject(with: support.data) as! [String: Any]
        try requireRecipe(widthSupport["state"] as? String == "unsupported",
                          "Class node width must not advertise support")
        try requireRecipe(widthSupport["reason_ids"] as? [String] == ["theme-support.no-supported-route"],
                          "Class node width lost its support explanation")
        func render(_ theme: Any, _ family: String, _ source: String) throws -> Data {
            let result = try execute(MermanOperationRequestV4(
                operationId: "svg", source: source, uri: nil,
                optionsJson: recipeOptions(theme, family: family), control: nil
            ))
            try requireRecipe(result.operationId == "svg" && result.mediaType == "image/svg+xml",
                              "\(mode): unexpected render result")
            return result.data
        }
        for entry in expected as! [[String: Any]] {
            let preset = entry["id"] as! String
            let result = try execute(MermanOperationRequestV4(
                operationId: "export-theme-preset-json", source: preset, uri: nil,
                optionsJson: nil, control: nil
            ))
            try requireRecipe(result.operationId == "export-theme-preset-json"
                              && result.mediaType == "application/json", "export result changed")
            let recipe = try JSONSerialization.jsonObject(with: result.data) as! [String: Any]
            try requireRecipe(recipe["schema_version"] as? Int == 1
                              && recipe["kind"] as? String == "complete_spec", "recipe envelope changed")
            let spec = recipe["complete_spec"] as! [String: Any]
            if ["brutalist", "spotless", "cyberpunk"].contains(preset) {
                try requireRecipe(spec["assets"] == nil && spec["mermaid"] == nil,
                                  "\(preset): native recipe embeds assets or compatibility settings")
            }
            let state = "stateDiagram-v2\n[*] --> Active\nActive --> [*]\n"
            try requireRecipe(render(["preset": preset], "state", state) == render(recipe, "state", state),
                              "\(mode)/\(preset): export changed SVG")
            if preset != "cyberpunk" { continue }
            if !cyberpunk.isEmpty {
                try requireRecipe(encodeRecipeJSON(cyberpunk) == encodeRecipeJSON(recipe),
                                  "one-shot and reusable exports disagree")
            }
            cyberpunk = recipe
            for (family, source) in sources {
                try requireRecipe(render(["preset": preset], family, source) == render(recipe, family, source),
                                  "\(mode)/\(family): complete recipe changed SVG")
            }
            var unversioned = recipe
            unversioned.removeValue(forKey: "schema_version")
            for invalid in [unversioned, recipe.merging(["schema_version": 0]) { _, new in new },
                            recipe.merging(["schema_version": 2]) { _, new in new },
                            recipe.merging(["preset": preset]) { _, new in new },
                            recipe.merging(["spec": [:]]) { _, new in new }] {
                do {
                    _ = try render(invalid, "state", state)
                    throw RecipeSmokeError.failed("\(mode): invalid recipe accepted")
                } catch let error as MermanError {
                    switch error {
                    case let .Binding(_, codeName, _, _, _, _, _, _, _, _):
                        try requireRecipe(codeName == "MERMAN_OPTIONS_JSON_ERROR", "wrong recipe rejection")
                    }
                }
            }
        }
    }

    // Edit explicit Class paints, not every coincidentally equal color in the complete recipe.
    var edited = cyberpunk
    var spec = edited["complete_spec"] as! [String: Any]
    var styles = spec["styles"] as! [[String: Any]]
    styles.append([
        "kind": "rule", "family": "class", "target": "node",
        "style": ["fill": "#123abc", "stroke": ["paint": "#456def"]],
    ])
    spec["styles"] = styles
    edited["complete_spec"] = spec
    var unsupported = edited
    styles.append([
        "kind": "rule", "family": "class", "target": "node",
        "style": ["stroke": ["width": 5]],
    ])
    spec["styles"] = styles
    unsupported["complete_spec"] = spec
    let outputParent = ProcessInfo.processInfo.environment["MERMAN_APPLE_THEME_SMOKE_OUTPUT"]
        .map { URL(fileURLWithPath: $0, isDirectory: true) }
        ?? FileManager.default.temporaryDirectory
    let outputRoot = outputParent.appendingPathComponent("merman-apple-recipes-\(UUID())", isDirectory: true)
    try FileManager.default.createDirectory(at: outputRoot, withIntermediateDirectories: true)
    for (name, recipe) in [("cyberpunk", cyberpunk), ("class-customized", edited),
                           ("class-unsupported-width", unsupported)] {
        let file = outputRoot.appendingPathComponent("\(name).json")
        let bytes = try encodeRecipeJSON(recipe)
        try bytes.write(to: file)
        let imported = outputRoot.appendingPathComponent(name, isDirectory: true)
        let child = Process()
        child.executableURL = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
        child.arguments = ["--import-theme-recipe", file.path, imported.path]
        try child.run()
        child.waitUntilExit()
        try requireRecipe(child.terminationReason == .exit && child.terminationStatus == 0,
                          "fresh recipe consumer failed")
        try requireRecipe(Data(contentsOf: file) == bytes, "import rewrote the exported recipe")
        for (family, source) in sources {
            let actual = try Data(contentsOf: imported.appendingPathComponent("\(family).svg"))
            let local = try engine.execute(request: MermanOperationRequestV4(
                operationId: "svg", source: source, uri: nil,
                optionsJson: recipeOptions(recipe, family: family), control: nil
            ))
            try requireRecipe(actual == local.data, "\(name)/\(family): fresh import changed SVG")
            let metadata = try JSONSerialization.jsonObject(with: Data(contentsOf:
                imported.appendingPathComponent("\(family).metadata.json"))) as! [String: Any]
            let localMetadata = try JSONSerialization.jsonObject(with: Data(local.metadata.rawJson.utf8))
            try requireRecipe(encodeRecipeJSON(metadata) == encodeRecipeJSON(localMetadata),
                              "\(name)/\(family): fresh import changed operation metadata")
            let evidence = metadata["theme_execution_evidence"] as! [String: Any]
            if name != "class-unsupported-width" || family != "class" {
                try requireRecipe(evidence["theme_status"] as? String == "verified"
                                  && evidence["target_status"] as? String == "unverified",
                                  "\(name)/\(family): unexpected qualification claim: "
                                  + String(decoding: encodeRecipeJSON(evidence), as: UTF8.self))
            }
            if name == "class-customized" {
                let original = try Data(contentsOf: outputRoot.appendingPathComponent("cyberpunk/\(family).svg"))
                if family == "class" {
                    try requireRecipe(actual != original, "Class edit did not reach the renderer")
                    let document = try XMLDocument(data: actual, options: [.nodeLoadExternalEntitiesNever])
                    let paths = try document.nodes(forXPath:
                        "//*[local-name()='g' and contains(concat(' ', normalize-space(@class), ' '), ' outer-path ')]/*[local-name()='path']"
                    ).compactMap { $0 as? XMLElement }
                    try requireRecipe(paths.contains { $0.attribute(forName: "fill")?.stringValue == "#123abc" },
                                      "custom fill missed the Class shape")
                    try requireRecipe(paths.contains {
                        $0.attribute(forName: "stroke")?.stringValue == "#456def"
                    }, "custom stroke missed the Class shape")
                } else {
                    try requireRecipe(actual == original, "Class rule leaked into \(family)")
                }
            }
            if name == "class-unsupported-width" {
                let customized = try Data(contentsOf:
                    outputRoot.appendingPathComponent("class-customized/\(family).svg"))
                try requireRecipe(actual == customized, "unsupported Class width changed \(family) SVG")
                if family == "class" {
                    try requireRecipe(evidence["theme_status"] as? String == "residual"
                                      && evidence["target_status"] as? String == "rejected",
                                      "unsupported width received positive admission")
                    let diagnostics = evidence["diagnostics"] as! [[String: Any]]
                    try requireRecipe(diagnostics.contains {
                        $0["code"] as? String == "unsupported-geometry"
                            && $0["target"] as? String == "node"
                            && $0["source_document"] as? String == "complete_spec"
                            && $0["source_paths"] as? [String] == ["/styles/\(styles.count - 1)"]
                    }, "unsupported width lost its source-addressed diagnostic")
                }
            }
        }
    }
    print("Apple preset recipes passed: 10 exports per consumer, 4 Cyberpunk families, 10 invalid recipes, 3 fresh-process imports, unsupported Class width")
    print("Apple recipe artifacts: \(outputRoot.path)")
}

private enum RecipeSmokeError: Error {
    case failed(String)
}
