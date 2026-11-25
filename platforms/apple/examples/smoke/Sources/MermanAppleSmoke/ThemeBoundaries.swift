import Foundation
import Merman

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
        let options = try themeJSON([
            "theme": "cyberpunk",
            "site_config": ["layout": "dagre", "htmlLabels": false],
            "svg": ["diagram_id": "apple-dense-\(name)"],
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
        let metadata = try themeObject(Data(result.metadata.rawJson.utf8))
        let outcome = metadata["theme_outcome"] as? [String: Any]
        try requireBoundary(outcome?["mode"] as? String == "merman_native"
                            && outcome?["preset_id"] as? String == "cyberpunk",
                            "\(name): native theme outcome disappeared")
    }
    print("Apple dense scenes passed: Class namespace/relations/notes and extended XY series")
}

private func requireBoundary(_ condition: Bool, _ message: String) throws {
    if !condition { throw BoundarySmokeError.failed(message) }
}

private enum BoundarySmokeError: Error {
    case failed(String)
}
