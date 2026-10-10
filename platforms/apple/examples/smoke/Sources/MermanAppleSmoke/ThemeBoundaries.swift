import CoreText
import Foundation
import Merman

private func requireBoundary(_ condition: Bool, _ message: String) throws {
    if !condition { throw BoundarySmokeError.failed(message) }
}

private func boundaryJSON(_ value: Any) throws -> String {
    String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]), as: UTF8.self)
}

private func nodePaints(_ data: Data, node: String, property: String) throws -> [String] {
    let document = try XMLDocument(data: data, options: [.nodeLoadExternalEntitiesNever])
    let paths = try document.nodes(forXPath:
        "//*[local-name()='g' and contains(@id, '-classId-\(node)-')]//*[local-name()='g' and contains(concat(' ', normalize-space(@class), ' '), ' outer-path ')]/*[local-name()='path']"
    ).compactMap { $0 as? XMLElement }
    // Class draws fill and outline on separate paths; each disables the other paint with `none`.
    let paints = paths.compactMap { $0.attribute(forName: property)?.stringValue }.filter { $0 != "none" }
    try requireBoundary(!paints.isEmpty, "missing \(node) terminal \(property)")
    return paints
}

func verifyThemePaintAndFontBoundaries(client: Merman) throws {
    let engine = try MermanEngine(optionsJson: nil, services: nil)
    defer { try? engine.close() }
    let consumers: [(String, (MermanOperationRequestV4) throws -> MermanOperationResult)] = [
        ("one-shot", { try client.execute(request: $0) }),
        ("reusable", { try engine.execute(request: $0) }),
    ]
    let outputParent = ProcessInfo.processInfo.environment["MERMAN_APPLE_THEME_SMOKE_OUTPUT"]
        .map { URL(fileURLWithPath: $0, isDirectory: true) }
        ?? FileManager.default.temporaryDirectory
    let output = outputParent.appendingPathComponent("merman-apple-boundaries-\(UUID())", isDirectory: true)
    try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
    let source = "classDiagram\nclass SourceOwned\nclass FillOwned\nclass ThemeOwned\n"
        + "style SourceOwned fill:#aa0000,stroke:#bb0000\nstyle FillOwned fill:#cc0000\n"
    func rule(_ style: [String: Any]) -> [String: Any] {
        ["kind": "rule", "family": "class", "target": "node", "style": style]
    }
    let base = rule(["fill": "#123abc", "stroke": ["paint": "#456def"]])
    let patches: [(String, [String: Any])] = [
        ("omitted", ["stroke": ["paint": "#654321"]]),
        ("clear", ["fill": NSNull(), "stroke": ["paint": "#654321"]]),
        ("transparent", ["fill": "transparent", "stroke": ["paint": "#654321"]]),
    ]
    for (mode, execute) in consumers {
        var baselineFill: [String] = []
        for (name, rules) in [("baseline", [[String: Any]]()), ("base", [base])]
            + patches.map({ ($0.0, [base, rule($0.1)]) }) {
            var options: [String: Any] = [
                "version": 3,
                "theme": ["schema_version": 1, "kind": "complete_spec", "complete_spec": ["styles": rules]],
                "site_config": ["htmlLabels": false], "svg": ["diagram_id": "apple-boundary-class"],
            ]
            let optionsJSON = try boundaryJSON(options)
            let result = try execute(MermanOperationRequestV4(
                operationId: "svg", source: source, uri: nil, optionsJson: optionsJSON, control: nil
            ))
            try result.data.write(to: output.appendingPathComponent("\(mode)-\(name).svg"))
            try Data(result.metadata.rawJson.utf8).write(to: output.appendingPathComponent("\(mode)-\(name).metadata.json"))
            try Data(optionsJSON.utf8).write(to: output.appendingPathComponent("\(mode)-\(name).options.json"))
            for (node, property, expected) in [("SourceOwned", "fill", "#aa0000"),
                                                ("SourceOwned", "stroke", "#bb0000"),
                                                ("FillOwned", "fill", "#cc0000")] {
                try requireBoundary(nodePaints(result.data, node: node, property: property).allSatisfy { $0 == expected },
                                    "\(mode)/\(name): source-owned \(node).\(property) changed")
            }
            let fill = try nodePaints(result.data, node: "ThemeOwned", property: "fill")
            if name == "baseline" { baselineFill = fill; continue }
            let expectedStroke = name == "base" ? "#456def" : "#654321"
            for node in ["ThemeOwned", "FillOwned"] {
                try requireBoundary(nodePaints(result.data, node: node, property: "stroke").allSatisfy { $0 == expectedStroke },
                                    "\(mode)/\(name): an independent stroke facet changed")
            }
            if name == "clear" {
                try requireBoundary(fill == baselineFill && !fill.contains("transparent") && !fill.contains("#123abc"),
                                    "Clear must mask the prior fill without becoming transparent")
            } else {
                let expectedFill = name == "transparent" ? "transparent" : "#123abc"
                try requireBoundary(fill.allSatisfy { $0 == expectedFill }, "\(mode)/\(name): wrong typed fill")
            }
            let metadata = try JSONSerialization.jsonObject(with: Data(result.metadata.rawJson.utf8)) as! [String: Any]
            let evidence = metadata["theme_execution_evidence"] as! [String: Any]
            try requireBoundary(evidence["theme_status"] as? String == (name == "clear" ? "residual" : "verified"),
                                "\(mode)/\(name): wrong theme admission: \(result.metadata.rawJson)")
            if name == "clear" {
                let diagnostics = evidence["diagnostics"] as! [[String: Any]]
                try requireBoundary(evidence["target_status"] as? String == "rejected" && diagnostics.contains {
                    $0["target"] as? String == "node" && $0["source_document"] as? String == "complete_spec"
                        && $0["source_paths"] as? [String] == ["/styles/1"]
                }, "Clear lost its source-addressed unsupported-route evidence")
                options["environment"] = ["theme_portability": "require-portable"]
                do {
                    _ = try execute(MermanOperationRequestV4(
                        operationId: "svg", source: source, uri: nil,
                        optionsJson: boundaryJSON(options), control: nil
                    ))
                    throw BoundarySmokeError.failed("strict Clear unexpectedly rendered")
                } catch let error as MermanError {
                    switch error {
                    case let .Binding(_, codeName, _, _, _, _, _, _, _, _):
                        try requireBoundary(codeName == "MERMAN_RENDER_ERROR", "wrong strict Clear error")
                    }
                }
            } else {
                try requireBoundary(evidence["target_status"] as? String == "unverified",
                                    "paint success must not certify the export target")
            }
        }
    }

    let missingFamily = "MermanSmokeMissingFontA17C92"
    let families = CTFontManagerCopyAvailableFontFamilyNames() as! [String]
    try requireBoundary(!families.contains(missingFamily), "missing-font fixture is installed on this host")
    let fontSpec: [String: Any] = ["typography": ["default": ["font_stack": [missingFamily, "sans-serif"]]]]
    // Even a font header must be rejected before font resource decoding.
    let assetSpec: [String: Any] = ["assets": ["fonts": [[
        "id": "caller-font", "format": "woff2", "data_base64": "d09GMg==",
    ]]]]
    for (name, spec) in [("family-only", fontSpec), ("font-asset", assetSpec)] {
        for selection in [["spec": spec], ["schema_version": 1, "kind": "complete_spec", "complete_spec": spec]] {
            let options = try boundaryJSON(["version": 3, "theme": selection, "site_config": ["htmlLabels": false]])
            let renderSource = "flowchart LR\nA[Missing host font] --> B[Fallback]\n"
            let operations: [() throws -> String] = [
                { try client.renderSvg(source: renderSource, optionsJson: options) },
                { try engine.renderSvg(source: renderSource, optionsJson: options) },
                {
                    let configured = try MermanEngine(optionsJson: options, services: nil)
                    defer { try? configured.close() }
                    // An asset must fail in the constructor itself, before any render request.
                    if name == "font-asset" { return "" }
                    return try configured.renderSvg(source: renderSource, optionsJson: nil)
                },
            ]
            for operation in operations {
                do {
                    let svg = try operation()
                    try requireBoundary(name == "family-only", "unsupported font asset was accepted")
                    try requireBoundary(svg.contains(missingFamily) && svg.contains("sans-serif")
                                        && !svg.contains("@font-face") && !svg.contains("data:font"),
                                        "font names must survive without embedding or silently substituting a font")
                } catch let error as MermanError {
                    switch error {
                    case let .Binding(_, codeName, kind, capabilityId, _, _, _, _, _, message):
                        try requireBoundary(name == "font-asset" && kind == .generic
                                            && codeName == "MERMAN_INVALID_ARGUMENT" && capabilityId == nil
                                            && message.contains("invalid theme: embedded theme font resources are not supported"),
                                            "wrong font resource rejection error")
                    }
                }
            }
        }
    }
    print("Apple theme boundaries passed: 10 Class cases, 2 strict Clear rejections, 6 missing-family renders, 6 unsupported font resource rejections")
    print("Apple boundary artifacts: \(output.path)")
}

func verifyDenseThemeScenes(client: Merman) throws {
    let classSource = """
    classDiagram
      namespace Core {
        class Account {
          +String id
          +save()
        }
        class Invoice {
          +Decimal total
          +approve()
        }
        class Payment {
          +String provider
          +capture()
        }
        class Receipt {
          +String number
          +issue()
        }
        class Ledger {
          +append()
          +reconcile()
        }
        class User {
          +String email
        }
      }
      User --> Account : owns
      Account --> Invoice : creates
      Invoice --> Payment : settles
      Payment --> Receipt : produces
      Receipt --> Ledger : records
      note for Account "primary aggregate"
      note for Payment "external gateway"
    """
    let xySource = """
    xychart-beta
      title "Quarterly Requests"
      x-axis "Quarter" [Q1, Q2, Q3, Q4, Q5, Q6, Q7, Q8]
      y-axis "Requests" 0 --> 100
      bar [12, 18, 25, 31, 44, 52, 65, 73]
      line [20, 22, 29, 35, 40, 49, 61, 70]
      bar [8, 14, 19, 27, 33, 41, 48, 57]
      line [16, 21, 24, 30, 38, 46, 55, 68]
    """
    for (name, source, minimums) in [
        ("dense-class", classSource, (12, 5)),
        ("extended-xy", xySource, (4, 8)),
    ] {
        let options = try boundaryJSON([
            "version": 3, "theme": ["preset": "cyberpunk"],
            "site_config": ["htmlLabels": false], "svg": ["diagram_id": "apple-dense-\(name)"],
        ])
        let result = try client.execute(request: MermanOperationRequestV4(
            operationId: "svg", source: source, uri: nil, optionsJson: options, control: nil
        ))
        let document = try XMLDocument(data: result.data, options: [.nodeLoadExternalEntitiesNever])
        try requireBoundary(result.mediaType == "image/svg+xml" && document.rootElement()?.localName == "svg",
                            "\(name): dense scene did not produce SVG")
        let nodes = try document.nodes(forXPath: "//*[local-name()='g' and contains(concat(' ', normalize-space(@class), ' '), ' node ')]")
        let paths = try document.nodes(forXPath: "//*[local-name()='path']")
        let bars = try document.nodes(forXPath: "//*[local-name()='rect']")
        let dataElements = name == "dense-class" ? paths.count : bars.count
        try requireBoundary(nodes.count >= minimums.0 || dataElements >= minimums.1,
                            "\(name): output was unexpectedly sparse (nodes=\(nodes.count), data=\(dataElements))")
        try requireBoundary(!result.metadata.rawJson.isEmpty, "\(name): metadata disappeared")
    }
    print("Apple dense scenes passed: Class namespace/relations/notes and extended XY series")
}

private enum BoundarySmokeError: Error {
    case failed(String)
}
