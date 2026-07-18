import Foundation
import Merman

@main
struct MermanAppleSmoke {
    private static let phaseLock = NSLock()
    private static var observedPhases = Set<Int32>()
    private static var observedOperations = Set<Int32>()

    fileprivate static func recordMeasurement(_ request: MermanTextMeasureRequest) {
        phaseLock.lock()
        observedPhases.insert(request.phase)
        observedOperations.insert(request.operation)
        phaseLock.unlock()
    }

    static func main() throws {
        try assertTextMeasurementContract()

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

        let documentSource = "Intro\n```mermaid\n\(source)\n```\n"
        let documentJson = try engine.analyzeDocumentJsonRaw(documentSource, uri: "file:///tmp/example.md")
        guard documentJson.contains("\"kind\":\"markdown\""), documentJson.contains("\"valid\":true") else {
            throw SmokeError.failed("document analysis smoke failed")
        }
        let documentFactsJson = try engine.analyzeDocumentFactsJsonRaw(
            documentSource,
            uri: "file:///tmp/example.md"
        )
        guard documentFactsJson.contains("\"source_id\":\"mermaid-fence-1\"") else {
            throw SmokeError.failed("document facts smoke failed")
        }

        let reusable = try engine.reusableEngine()
        try reusable.setTextMeasureCallback(mermanAppleSmokeMeasureText)
        let reusableSvg = try reusable.renderSvg(source)
        guard reusableSvg.contains("<svg"), reusableSvg.contains("Hello") else {
            throw SmokeError.failed("reusable renderSvg smoke failed")
        }
        phaseLock.lock()
        let phases = observedPhases
        let operations = observedOperations
        phaseLock.unlock()
        guard !phases.isEmpty, phases.allSatisfy({ (0...3).contains($0) }) else {
            throw SmokeError.failed("invalid host text measurement phases: \(phases)")
        }
        guard !operations.isEmpty, operations.allSatisfy({ (0...18).contains($0) }) else {
            throw SmokeError.failed("invalid host text measurement operations: \(operations)")
        }
        let reusableDocumentJson = try reusable.analyzeDocumentJsonRaw(
            documentSource,
            uri: "file:///tmp/example.md"
        )
        guard reusableDocumentJson.contains("\"kind\":\"markdown\"") else {
            throw SmokeError.failed("reusable document analysis smoke failed")
        }
        let reusableDocumentFactsJson = try reusable.analyzeDocumentFactsJsonRaw(
            documentSource,
            uri: "file:///tmp/example.md"
        )
        guard reusableDocumentFactsJson.contains("\"source_id\":\"mermaid-fence-1\"") else {
            throw SmokeError.failed("reusable document facts smoke failed")
        }
        reusable.close()

        try ReusableEngineLifecycleSmoke.run(engine: engine)

        guard try engine.supportedDiagrams().contains("flowchart") else {
            throw SmokeError.failed("supported diagrams smoke failed")
        }

        let flowchartCapability = try engine.diagramFamilyCapabilities().contains { capability in
            capability.diagramType == "flowchart"
                && capability.logicalFamilyKind == "flowchart"
                && capability.metadataId == "flowchart"
                && capability.renderModelKind == "flowchart"
                && capability.hasDetector
                && capability.hasSemanticParser
                && capability.hasEditorParser
                && capability.hasCombinedParser
                && capability.hasRenderParser
                && !capability.hasHeader
                && capability.configNamespace == "flowchart"
        }
        guard flowchartCapability else {
            throw SmokeError.failed("diagram family capabilities smoke failed")
        }

        let ganttAsciiCapability = try engine.asciiCapabilities().contains { capability in
            capability.diagramType == "gantt"
                && capability.supportLevel == "summary"
                && !capability.summaryFallback
        }
        guard ganttAsciiCapability else {
            throw SmokeError.failed("ASCII capabilities smoke failed")
        }

        guard try engine.lintRuleCatalog().contains(where: {
            $0.id == "merman.authoring.flowchart.explicit_direction"
                && $0.evidence.contains("docs/adr/0072-lint-rule-governance.md")
        }) else {
            throw SmokeError.failed("lint rule catalog smoke failed")
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

private enum ReusableEngineLifecycleSmoke {
    private static let callbackEntered = DispatchSemaphore(value: 0)
    private static let callbackMayReturn = DispatchSemaphore(value: 0)
    private static let renderFinished = DispatchSemaphore(value: 0)
    private static let closeFinished = DispatchSemaphore(value: 0)
    private static let stateLock = NSLock()
    private static var reentryEngine: MermanReusableEngine?
    private static var renderError: Error?
    private static var reentryError: Error?
    private static var reentryReturned = false
    private static var didBlockCallback = false

    static func run(engine: MermanEngine) throws {
        let reusable = try engine.reusableEngine()
        resetState()
        reentryEngine = reusable
        defer { reentryEngine = nil }

        try reusable.setTextMeasureCallback(mermanReusableLifecycleMeasureText)

        let source = "flowchart TD\nA[Concurrent] --> B[Close]"
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                _ = try reusable.renderSvg(source)
            } catch {
                recordRenderError(error)
            }
            renderFinished.signal()
        }

        guard callbackEntered.wait(timeout: .now() + .seconds(5)) == .success else {
            throw SmokeError.failed("reusable lifecycle smoke did not enter text measure callback")
        }

        DispatchQueue.global(qos: .userInitiated).async {
            reusable.close()
            closeFinished.signal()
        }

        if closeFinished.wait(timeout: .now() + .milliseconds(100)) == .success {
            throw SmokeError.failed("reusable close returned before the in-flight native call finished")
        }

        callbackMayReturn.signal()
        guard renderFinished.wait(timeout: .now() + .seconds(5)) == .success else {
            throw SmokeError.failed("reusable lifecycle smoke render did not finish")
        }
        guard closeFinished.wait(timeout: .now() + .seconds(5)) == .success else {
            throw SmokeError.failed("reusable lifecycle smoke close did not finish")
        }

        if let error = takeRenderError() {
            throw SmokeError.failed("reusable lifecycle render failed: \(error)")
        }
        try assertReentryWasRejected()
        try assertClosedEngineRejectsCalls(reusable, source: source)
    }

    private static func exerciseReentryGuard() {
        guard let engine = reentryEngine else {
            return
        }
        do {
            _ = try engine.renderSvg("flowchart TD\nX --> Y")
            recordReentryReturned()
        } catch {
            recordReentryError(error)
        }
    }

    private static func assertReentryWasRejected() throws {
        stateLock.lock()
        let returned = reentryReturned
        let error = reentryError
        stateLock.unlock()

        if returned {
            throw SmokeError.failed("reusable engine allowed callback reentry")
        }
        guard let error,
              case let MermanError.binding(_, codeName, _) = error,
              codeName == "SWIFT_ENGINE_REENTERED"
        else {
            throw SmokeError.failed("reusable engine did not report SWIFT_ENGINE_REENTERED")
        }
    }

    private static func assertClosedEngineRejectsCalls(
        _ engine: MermanReusableEngine,
        source: String
    ) throws {
        do {
            _ = try engine.renderSvg(source)
            throw SmokeError.failed("closed reusable engine accepted renderSvg")
        } catch let error as MermanError {
            guard case let .binding(_, codeName, _) = error, codeName == "SWIFT_ENGINE_CLOSED" else {
                throw SmokeError.failed("closed reusable engine reported unexpected error: \(error)")
            }
        }
    }

    private static func recordRenderError(_ error: Error) {
        stateLock.lock()
        renderError = error
        stateLock.unlock()
    }

    private static func takeRenderError() -> Error? {
        stateLock.lock()
        let error = renderError
        renderError = nil
        stateLock.unlock()
        return error
    }

    private static func recordReentryError(_ error: Error) {
        stateLock.lock()
        reentryError = error
        stateLock.unlock()
    }

    private static func recordReentryReturned() {
        stateLock.lock()
        reentryReturned = true
        stateLock.unlock()
    }

    private static func markFirstBlockingCallback() -> Bool {
        stateLock.lock()
        defer { stateLock.unlock() }
        if didBlockCallback {
            return false
        }
        didBlockCallback = true
        return true
    }

    private static func resetState() {
        stateLock.lock()
        renderError = nil
        reentryError = nil
        reentryReturned = false
        didBlockCallback = false
        stateLock.unlock()
    }

    fileprivate static func measureText(_ request: MermanTextMeasureRequest) -> MermanTextMeasureResult {
        let text = mermanSmokeText(request)
        if text == "Concurrent" {
            if markFirstBlockingCallback() {
                exerciseReentryGuard()
                callbackEntered.signal()
                _ = callbackMayReturn.wait(timeout: .now() + .seconds(5))
            }
            return mermanHandledTextMeasurement(
                operation: request.operation,
                width: 64.0,
                height: request.line_height
            )
        }
        return MermanTextMeasureResult()
    }
}

enum SmokeError: Error {
    case failed(String)
}

private func mermanAppleSmokeMeasureText(
    _ request: MermanTextMeasureRequest,
    _ _: UnsafeMutableRawPointer?
) -> MermanTextMeasureResult {
    MermanAppleSmoke.recordMeasurement(request)
    let text = mermanSmokeText(request)
    if text == "Hello" {
        return mermanHandledTextMeasurement(
            operation: request.operation,
            width: 42.0,
            height: request.line_height
        )
    }
    return MermanTextMeasureResult()
}

private func mermanReusableLifecycleMeasureText(
    _ request: MermanTextMeasureRequest,
    _ _: UnsafeMutableRawPointer?
) -> MermanTextMeasureResult {
    ReusableEngineLifecycleSmoke.measureText(request)
}

private func mermanSmokeText(_ request: MermanTextMeasureRequest) -> String {
    request.text.map {
        String(decoding: UnsafeBufferPointer(start: $0, count: request.text_len), as: UTF8.self)
    } ?? ""
}

private func assertTextMeasurementContract() throws {
    let operationCodes = MermanTextMeasurementOperation.allCases.map(\.rawValue)
    guard operationCodes == Array(Int32(0)...Int32(18)) else {
        throw SmokeError.failed("text measurement constants are not the contiguous ABI range 0...18")
    }

    let dimensions = mermanHandledTextMeasurement(
        operation: MermanTextMeasurementOperation.mermaidCalculateTextDimensions.rawValue,
        width: 42.0,
        height: 24.0
    )
    guard dimensions.result_kind == MermanTextMeasurementResultKind.metrics.rawValue else {
        throw SmokeError.failed("MermaidCalculateTextDimensions must return metrics")
    }

    let canvasWidth = mermanHandledTextMeasurement(
        operation: MermanTextMeasurementOperation.canvasMeasureTextWidth.rawValue,
        width: 42.0,
        height: 24.0
    )
    guard canvasWidth.result_kind == MermanTextMeasurementResultKind.length.rawValue else {
        throw SmokeError.failed("CanvasMeasureTextWidth must return length")
    }

    let rawBBoxHeight = mermanHandledTextMeasurement(
        operation: MermanTextMeasurementOperation.rawBBoxHeight.rawValue,
        width: 42.0,
        height: 24.0
    )
    guard rawBBoxHeight.result_kind == MermanTextMeasurementResultKind.length.rawValue,
          rawBBoxHeight.length == 24.0
    else {
        throw SmokeError.failed("RawBBoxHeight must return the raw bbox height as length")
    }

    let yOffset = mermanHandledTextMeasurement(
        operation: MermanTextMeasurementOperation.createTextBBoxYOffset.rawValue,
        width: 42.0,
        height: 24.0
    )
    guard MermanTextMeasurementOperation.createTextBBoxYOffset.acceptsSignedLength,
          yOffset.length < 0
    else {
        throw SmokeError.failed("CreateTextBBoxYOffset must preserve signed lengths")
    }
    let middleYOffset = mermanHandledTextMeasurement(
        operation: MermanTextMeasurementOperation.createTextMiddleBBoxYOffset.rawValue,
        width: 42.0,
        height: 24.0
    )
    guard MermanTextMeasurementOperation.createTextMiddleBBoxYOffset.acceptsSignedLength,
          middleYOffset.length < 0
    else {
        throw SmokeError.failed("CreateTextMiddleBBoxYOffset must preserve signed lengths")
    }
}

private func mermanHandledTextMeasurement(
    operation: Int32,
    width: Double,
    height: Double
) -> MermanTextMeasureResult {
    var result = MermanTextMeasureResult()
    guard let operation = MermanTextMeasurementOperation(rawValue: operation) else {
        return result
    }
    result.handled = 1
    result.result_kind = operation.requiredResultKind.rawValue
    switch operation {
    case .measure, .wrapped, .mermaidCalculateTextDimensions:
        result.width = width
        result.height = height
        result.line_count = 1
    case .computedLength,
         .simpleBBoxWidth,
         .rawBBoxWidth,
         .boundingClientRectWidth,
         .tspanBBoxWidth,
         .wrapProbeBBoxWidth,
         .canvasMeasureTextWidth:
        result.length = width
    case .tspanBBoxHeight, .simpleBBoxHeight, .rawBBoxHeight:
        result.length = height
    case .createTextBBoxYOffset, .createTextMiddleBBoxYOffset:
        result.length = -1.0
    case .bboxX, .bboxXWithAsciiOverhang, .titleBBoxX:
        result.bbox_left = width / 2
        result.bbox_right = width / 2
    case .wrappedWithRawWidth:
        result.width = width
        result.height = height
        result.raw_width = width
        result.line_count = 1
        result.has_raw_width = 1
    }
    return result
}
