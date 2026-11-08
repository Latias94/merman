import Foundation
import Merman

// Consume the same repository vectors as the Python, Dart, and native C smokes.
func verifyThemeAuthoringGoldens(client: Merman) throws {
    let fixtures = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .appendingPathComponent("../../../../../../crates/merman-theme-authoring-fixtures/fixtures/authoring-v1")
        .standardizedFileURL
    func read(_ name: String) throws -> Data {
        try Data(contentsOf: fixtures.appendingPathComponent(name))
    }
    func json(_ data: Data) throws -> Any {
        try JSONSerialization.jsonObject(with: data)
    }
    func encoded(_ value: Any) throws -> Data {
        try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed])
    }
    func requireEqual(_ actual: Any, _ expected: Any, _ label: String) throws {
        guard try encoded(actual) == encoded(expected) else {
            throw ThemeGoldenError.mismatch(label)
        }
    }
    let support = try json(read("support.json")) as! [[String: Any]]
    let errors = try json(read("errors.json")) as! [[String: Any]]
    let engine = try MermanEngine(optionsJson: nil, services: nil)
    defer { try? engine.close() }
    let consumers: [(String, (MermanOperationRequestV4) throws -> MermanOperationResult)] = [
        ("one-shot", { try client.execute(request: $0) }),
        ("reusable", { try engine.execute(request: $0) }),
    ]
    for (consumer, execute) in consumers {
        func request(_ operation: String, _ source: String, _ options: String? = nil) -> MermanOperationRequestV4 {
            MermanOperationRequestV4(
                operationId: operation, source: source, uri: nil, optionsJson: options, control: nil
            )
        }
        func executeJSON(_ operation: String, _ source: String) throws -> Any {
            let result = try execute(request(operation, source))
            try requireEqual(result.operationId, operation, "\(consumer)/operation")
            try requireEqual(result.mediaType, "application/json", "\(consumer)/media")
            return try json(result.data)
        }
        let pairedRecipe = try json(read("paired.recipe.json"))
        let inspection = try executeJSON("materialize-theme-json", String(decoding: encoded([
            "schema_version": 1, "action": "inspect", "recipe": pairedRecipe,
        ]), as: UTF8.self)) as! [String: Any]
        try requireEqual(inspection["supported_appearances"]!, ["light", "dark"], "\(consumer)/paired appearances")
        try requireEqual(inspection["default_appearance"]!, "light", "\(consumer)/paired default")
        var pairedSpecs: [Any] = []
        var pairedSVGs: [Data] = []
        for appearance in ["light", "dark", "light"] {
            let selected = try executeJSON("materialize-theme-json", String(decoding: encoded([
                "schema_version": 1, "action": "materialize",
                "recipe": pairedRecipe, "appearance": appearance,
            ]), as: UTF8.self)) as! [String: Any]
            try requireEqual(selected["effective_appearance"]!, appearance, "\(consumer)/paired selection")
            pairedSpecs.append(selected["spec"]!)
            let options: [String: Any] = [
                "theme": ["recipe": pairedRecipe, "appearance": appearance],
                "site_config": ["htmlLabels": false],
                "svg": ["diagram_id": "apple-paired-theme"],
            ]
            let rendered = try execute(request(
                "svg", "flowchart LR\nA[Review] --> B[Publish]",
                String(decoding: encoded(options), as: UTF8.self)
            ))
            pairedSVGs.append(rendered.data)
        }
        try requireEqual(pairedSpecs[0], pairedSpecs[2], "\(consumer)/paired materialization isolation")
        guard try encoded(pairedSpecs[0]) != encoded(pairedSpecs[1]),
              pairedSVGs[0] != pairedSVGs[1], pairedSVGs[0] == pairedSVGs[2] else {
            throw ThemeGoldenError.mismatch("\(consumer): paired appearance isolation")
        }
        let exportedPair = try executeJSON("export-theme-preset-json", "editor") as! [String: Any]
        let exportedDesign = exportedPair["design"] as! [String: Any]
        let appearances = exportedDesign["appearances"] as! [String: Any]
        try requireEqual(appearances.keys.sorted(), ["dark", "light"], "\(consumer)/complete pair export")
        for name in ["light", "dark"] {
            let actual = try executeJSON(
                "materialize-theme-json", String(decoding: read("\(name).design.json"), as: UTF8.self)
            )
            let expected: [String: Any] = [
                "schema_version": 1, "authoring_schema_version": 1,
                "expansion_version": 1, "spec_schema_version": 1,
                "spec": try json(read("\(name).spec.canonical.json")),
                "references": try json(read("\(name).references.json")),
            ]
            try requireEqual(actual, expected, "\(consumer)/\(name)")
        }
        for vector in support {
            let actual = try executeJSON(
                "describe-theme-support-json", String(decoding: encoded(vector["query"]!), as: UTF8.self)
            )
            try requireEqual(actual, vector["expected"]!, "\(consumer)/\(vector["id"]!)")
        }
        for vector in errors {
            let operations = vector["id"] as? String == "encoded-byte-limit"
                ? ["materialize-theme-json", "describe-theme-support-json", "export-theme-preset-json"]
                : ["materialize-theme-json"]
            for operation in operations {
                let label = "\(consumer)/\(operation)/\(vector["id"]!)"
                do {
                    _ = try execute(request(operation, vector["source"] as! String, vector["options_json"] as? String))
                    throw ThemeGoldenError.mismatch("\(label): invalid request accepted")
                } catch let error as MermanError {
                    switch error {
                    case let .Binding(_, codeName, _, _, _, _, _, _, detailsJson, _):
                        try requireEqual(codeName, vector["code_name"]!, label)
                        guard let data = detailsJson?.data(using: .utf8),
                              let details = try json(data) as? [String: Any] else {
                            throw ThemeGoldenError.mismatch("\(label): missing error details")
                        }
                        try requireEqual(details["resource"] ?? NSNull(), vector["resource"] ?? NSNull(), label)
                        var authoring: Any = details["theme_authoring"] ?? NSNull()
                        if var envelope = authoring as? [String: Any],
                           var diagnostics = envelope["diagnostics"] as? [[String: Any]] {
                            for index in diagnostics.indices {
                                guard let message = diagnostics[index].removeValue(forKey: "message") as? String,
                                      !message.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                                    throw ThemeGoldenError.mismatch("\(label): missing diagnostic message")
                                }
                            }
                            envelope["diagnostics"] = diagnostics
                            authoring = envelope
                        }
                        let expected = operation == "materialize-theme-json" ? vector["theme_authoring"]! : NSNull()
                        try requireEqual(authoring, expected, label)
                    }
                }
            }
        }
    }
    print("Apple theme goldens passed: paired appearances, 2 raw materializations, \(support.count) support queries, 5 errors per consumer")
}

private enum ThemeGoldenError: Error {
    case mismatch(String)
}
