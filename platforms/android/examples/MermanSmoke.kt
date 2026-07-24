package io.merman.examples

import io.merman.MermanEngine
import io.merman.MermanErrorKind
import io.merman.MermanException
import io.merman.MermanReusableEngine
import io.merman.MermanTextMeasureResult
import io.merman.MermanTextMeasurementOperation
import io.merman.MermanTextMeasurementResultKind

private val textMeasurementOperations = intArrayOf(
    MermanTextMeasurementOperation.MEASURE,
    MermanTextMeasurementOperation.COMPUTED_LENGTH,
    MermanTextMeasurementOperation.BBOX_X,
    MermanTextMeasurementOperation.BBOX_X_WITH_ASCII_OVERHANG,
    MermanTextMeasurementOperation.TITLE_BBOX_X,
    MermanTextMeasurementOperation.SIMPLE_BBOX_WIDTH,
    MermanTextMeasurementOperation.RAW_BBOX_WIDTH,
    MermanTextMeasurementOperation.TSPAN_BBOX_WIDTH,
    MermanTextMeasurementOperation.TSPAN_BBOX_HEIGHT,
    MermanTextMeasurementOperation.WRAP_PROBE_BBOX_WIDTH,
    MermanTextMeasurementOperation.SIMPLE_BBOX_HEIGHT,
    MermanTextMeasurementOperation.WRAPPED,
    MermanTextMeasurementOperation.WRAPPED_WITH_RAW_WIDTH,
    MermanTextMeasurementOperation.BOUNDING_CLIENT_RECT_WIDTH,
    MermanTextMeasurementOperation.CREATE_TEXT_BBOX_Y_OFFSET,
    MermanTextMeasurementOperation.MERMAID_CALCULATE_TEXT_DIMENSIONS,
    MermanTextMeasurementOperation.CANVAS_MEASURE_TEXT_WIDTH,
    MermanTextMeasurementOperation.CREATE_TEXT_MIDDLE_BBOX_Y_OFFSET,
    MermanTextMeasurementOperation.RAW_BBOX_HEIGHT,
)

private fun smokeTextMeasurementResult(
    operation: Int,
    width: Double,
    height: Double,
): MermanTextMeasureResult? = when (operation) {
    MermanTextMeasurementOperation.MEASURE,
    MermanTextMeasurementOperation.WRAPPED,
    MermanTextMeasurementOperation.MERMAID_CALCULATE_TEXT_DIMENSIONS ->
        MermanTextMeasureResult.metrics(
            width = width,
            height = height,
            lineCount = 1,
        )
    MermanTextMeasurementOperation.COMPUTED_LENGTH,
    MermanTextMeasurementOperation.SIMPLE_BBOX_WIDTH,
    MermanTextMeasurementOperation.RAW_BBOX_WIDTH,
    MermanTextMeasurementOperation.BOUNDING_CLIENT_RECT_WIDTH,
    MermanTextMeasurementOperation.TSPAN_BBOX_WIDTH,
    MermanTextMeasurementOperation.WRAP_PROBE_BBOX_WIDTH,
    MermanTextMeasurementOperation.CANVAS_MEASURE_TEXT_WIDTH ->
        MermanTextMeasureResult.length(width)
    MermanTextMeasurementOperation.TSPAN_BBOX_HEIGHT,
    MermanTextMeasurementOperation.SIMPLE_BBOX_HEIGHT,
    MermanTextMeasurementOperation.RAW_BBOX_HEIGHT ->
        MermanTextMeasureResult.length(height)
    MermanTextMeasurementOperation.CREATE_TEXT_BBOX_Y_OFFSET,
    MermanTextMeasurementOperation.CREATE_TEXT_MIDDLE_BBOX_Y_OFFSET ->
        MermanTextMeasureResult.length(-1.0)
    MermanTextMeasurementOperation.BBOX_X,
    MermanTextMeasurementOperation.BBOX_X_WITH_ASCII_OVERHANG,
    MermanTextMeasurementOperation.TITLE_BBOX_X -> MermanTextMeasureResult.horizontalExtents(
        left = width / 2.0,
        right = width / 2.0,
    )
    MermanTextMeasurementOperation.WRAPPED_WITH_RAW_WIDTH -> MermanTextMeasureResult.wrappedWithRawWidth(
        width = width,
        height = height,
        lineCount = 1,
        rawWidth = width,
    )
    else -> null
}

fun runMermanSmoke() {
    val source = "flowchart TD\nA[Hello] --> B[World]"
    val textMeasureSource = "flowchart TD\nA[Start] --> B{Condition?}"

    check(textMeasurementOperations.contentEquals(IntArray(19) { it })) {
        "text measurement operation constants are not the contiguous ABI range 0..18"
    }
    check(
        runCatching {
            MermanTextMeasureResult.metrics(width = 42.0, height = 24.0, lineCount = 0)
        }.exceptionOrNull() is IllegalArgumentException,
    ) {
        "metrics must reject a zero line count"
    }
    check(
        runCatching { MermanTextMeasureResult.length(Double.NaN) }
            .exceptionOrNull() is IllegalArgumentException,
    ) {
        "length must reject non-finite values"
    }
    check(
        runCatching { MermanTextMeasureResult.horizontalExtents(left = -1.0, right = 21.0) }
            .exceptionOrNull() is IllegalArgumentException,
    ) {
        "horizontal extents must reject negative dimensions"
    }
    check(
        smokeTextMeasurementResult(
            MermanTextMeasurementOperation.MERMAID_CALCULATE_TEXT_DIMENSIONS,
            42.0,
            24.0,
        )?.resultKind == MermanTextMeasurementResultKind.METRICS,
    ) {
        "MermaidCalculateTextDimensions must return metrics"
    }
    check(
        smokeTextMeasurementResult(
            MermanTextMeasurementOperation.CANVAS_MEASURE_TEXT_WIDTH,
            42.0,
            24.0,
        )?.resultKind == MermanTextMeasurementResultKind.LENGTH,
    ) {
        "CanvasMeasureTextWidth must return length"
    }
    check(
        smokeTextMeasurementResult(
            MermanTextMeasurementOperation.RAW_BBOX_HEIGHT,
            42.0,
            24.0,
        )?.let {
            it.resultKind == MermanTextMeasurementResultKind.LENGTH && it.length == 24.0
        } == true,
    ) {
        "RawBBoxHeight must return the raw bbox height as length"
    }
    check(
        smokeTextMeasurementResult(
            MermanTextMeasurementOperation.CREATE_TEXT_BBOX_Y_OFFSET,
            42.0,
            24.0,
        )?.length?.let { it < 0.0 } == true,
    ) {
        "CreateTextBBoxYOffset must preserve signed lengths"
    }
    check(
        smokeTextMeasurementResult(
            MermanTextMeasurementOperation.CREATE_TEXT_MIDDLE_BBOX_Y_OFFSET,
            42.0,
            24.0,
        )?.length?.let { it < 0.0 } == true,
    ) {
        "CreateTextMiddleBBoxYOffset must preserve signed lengths"
    }

    val earlyReusableEngine = MermanReusableEngine()
    try {
        val earlyReusableAnalysisJson = earlyReusableEngine.analyzeJson(source)
        check(earlyReusableAnalysisJson.contains("\"valid\":true")) {
            "reusable-first analysis smoke failed"
        }
    } finally {
        earlyReusableEngine.close()
    }

    val svg = MermanEngine.renderSvg(source)
    check(svg.contains("<svg") && svg.contains("Hello") && svg.contains("World")) {
        "SVG smoke failed"
    }

    val ascii = MermanEngine.renderAscii(source)
    check(ascii.contains("Hello") && ascii.contains("World")) {
        "ASCII smoke failed"
    }

    val semanticJson = MermanEngine.parseJson(source)
    check(semanticJson.contains("flowchart-v2")) {
        "semantic JSON smoke failed"
    }

    val unknownOperationError = runCatching {
        MermanEngine.executeBytes("not-an-operation", source)
    }.exceptionOrNull()
    check(
        unknownOperationError is MermanException &&
            unknownOperationError.kind == MermanErrorKind.UNKNOWN_OPERATION &&
            unknownOperationError.capabilityId == null,
    ) {
        "unknown operation did not preserve its machine-readable binding error"
    }

    val layoutJson = MermanEngine.layoutJson(source)
    check(layoutJson.contains("layout")) {
        "layout JSON smoke failed"
    }

    val analysisJson = MermanEngine.analyzeJson(source)
    check(analysisJson.contains("\"version\":1") && analysisJson.contains("\"valid\":true")) {
        "analysis JSON smoke failed"
    }

    val validationJson = MermanEngine.validateJson(source)
    check(validationJson.contains("\"valid\":true")) {
        "validation JSON smoke failed"
    }

    val documentSource = "Intro\n```mermaid\n$source\n```\n"
    val documentJson = MermanEngine.analyzeDocumentJson(
        documentSource,
        "file:///tmp/example.md",
    )
    check(documentJson.contains("\"kind\":\"markdown\"") && documentJson.contains("\"valid\":true")) {
        "document analysis JSON smoke failed"
    }
    val documentFactsJson = MermanEngine.analyzeDocumentFactsJson(
        documentSource,
        "file:///tmp/example.md",
    )
    check(
        documentFactsJson.contains("\"version\":1") &&
            documentFactsJson.contains("\"source_id\":\"mermaid-fence-1\""),
    ) {
        "document facts JSON smoke failed"
    }

    check(MermanEngine.supportedDiagramsJson().contains("flowchart")) {
        "supported diagrams smoke failed"
    }
    check(MermanEngine.asciiCapabilitiesJson().contains("\"support_level\":\"summary\"")) {
        "ASCII capabilities smoke failed"
    }
    check(MermanEngine.diagramFamilyCapabilitiesJson().contains("\"diagram_type\":\"flowchart\"")) {
        "diagram family capabilities smoke failed"
    }
    val runtimeCatalogJson = MermanEngine.runtimeCatalogJson()
    check(
        runtimeCatalogJson.contains("\"schema_version\":1") &&
            runtimeCatalogJson.contains("\"capabilities\":") &&
            runtimeCatalogJson.contains("\"registry\":") &&
            runtimeCatalogJson.contains("\"resources\":") &&
            runtimeCatalogJson.contains("\"operation_ids\":[") &&
            runtimeCatalogJson.contains("\"transport_api_version\":1") &&
            runtimeCatalogJson.contains("\"system_adapter_ids\":[") &&
            runtimeCatalogJson.contains("\"general_binding_default_profile\":\"interactive\""),
    ) {
        "runtime catalog smoke failed"
    }
    check(MermanEngine.lintRuleCatalogJson().contains("\"version\":1")) {
        "lint rule catalog envelope smoke failed"
    }
    check(MermanEngine.lintRuleCatalogJson().contains("\"rules\":")) {
        "lint rule catalog rules envelope smoke failed"
    }
    check(MermanEngine.lintRuleCatalogJson().contains("merman.authoring.flowchart.explicit_direction")) {
        "lint rule catalog smoke failed"
    }
    check(MermanEngine.lintRuleCatalogJson().contains("docs/adr/0072-lint-rule-governance.md")) {
        "lint rule catalog evidence smoke failed"
    }
    check(MermanEngine.supportedThemesJson().contains("default")) {
        "themes smoke failed"
    }
    check(MermanEngine.supportedHostThemePresetsJson().contains("one-dark")) {
        "host theme presets smoke failed"
    }

    val engine = MermanReusableEngine()
    try {
        var measureCalls = 0
        var sawCondition = false
        var sawNowrap = false
        var sawBreakSpaces = false
        var sawFontStyle = false
        var sawSpacingDefaults = false
        val seenMeasureTexts = linkedSetOf<String>()
        val seenWrapModes = linkedSetOf<Int>()
        val seenPhases = linkedSetOf<Int>()
        val seenOperations = linkedSetOf<Int>()
        val seenMaxWidthStates = linkedSetOf<String>()

        fun textMeasureSummary(): String =
            "calls=$measureCalls, texts=${seenMeasureTexts.joinToString("|")}, " +
                "wrapModes=${seenWrapModes.joinToString("|")}, " +
                "phases=${seenPhases.joinToString("|")}, " +
                "operations=${seenOperations.joinToString("|")}, " +
                "maxWidth=${seenMaxWidthStates.joinToString("|")}"

        engine.setTextMeasurer { request ->
            measureCalls += 1
            if (seenMeasureTexts.size < 8) {
                seenMeasureTexts += request.text
            }
            if (seenWrapModes.size < 8) {
                seenWrapModes += request.wrapMode
            }
            seenPhases += request.phase
            seenOperations += request.operation
            if (seenMaxWidthStates.size < 8) {
                seenMaxWidthStates += if (request.maxWidth == null) "none" else "some"
            }
            if (request.text == "Condition?") {
                sawCondition = true
                sawFontStyle = sawFontStyle ||
                    (request.fontStyle == "normal" && request.lineHeight > request.fontSize)
                sawSpacingDefaults = sawSpacingDefaults ||
                    (request.letterSpacing == 0.0 && request.wordSpacing == 0.0)
                if (request.maxWidth == null) {
                    sawNowrap = true
                } else {
                    sawBreakSpaces = true
                }
                smokeTextMeasurementResult(request.operation, width = 140.0, height = 24.0)
            } else {
                null
            }
        }
        val reusableSvg = engine.renderSvg(textMeasureSource)
        check(reusableSvg.contains("<svg") && reusableSvg.contains("Condition?")) {
            "reusable engine SVG smoke failed"
        }
        check(measureCalls > 0) {
            "text measurer callback smoke failed: ${textMeasureSummary()}"
        }
        check(seenPhases.size >= 2 && seenPhases.all { it in 0..3 }) {
            "text measurement phase smoke failed: ${textMeasureSummary()}"
        }
        check(
            seenOperations.size >= 2 &&
                seenOperations.all {
                    it in MermanTextMeasurementOperation.MEASURE..
                        MermanTextMeasurementOperation.RAW_BBOX_HEIGHT
                },
        ) {
            "text measurement operation smoke failed: ${textMeasureSummary()}"
        }
        check(sawCondition && sawNowrap && sawBreakSpaces && sawFontStyle && sawSpacingDefaults) {
            "text measurer request metadata smoke failed: ${textMeasureSummary()}"
        }
        val reusableDocumentJson = engine.analyzeDocumentJson(
            documentSource,
            "file:///tmp/example.md",
        )
        check(reusableDocumentJson.contains("\"kind\":\"markdown\"")) {
            "reusable document analysis JSON smoke failed"
        }
        val reusableDocumentFactsJson = engine.analyzeDocumentFactsJson(
            documentSource,
            "file:///tmp/example.md",
        )
        check(
            reusableDocumentFactsJson.contains("\"version\":1") &&
                reusableDocumentFactsJson.contains("\"source_id\":\"mermaid-fence-1\""),
        ) {
            "reusable document facts JSON smoke failed"
        }
        engine.setTextMeasurer(null)
    } finally {
        engine.close()
    }

    val reentrantEngine = MermanReusableEngine()
    try {
        var reentryRejected = false
        reentrantEngine.setTextMeasurer {
            if (!reentryRejected) {
                try {
                    reentrantEngine.renderSvg(textMeasureSource)
                    error("reentrant render unexpectedly succeeded")
                } catch (error: MermanException) {
                    check(error.kind == MermanErrorKind.REENTRANT_CALL) {
                        "unexpected reentrant error: ${error.message}"
                    }
                    reentryRejected = true
                }
            }
            null
        }
        val reentrantSvg = reentrantEngine.renderSvg(textMeasureSource)
        check(reentrantSvg.contains("<svg") && reentryRejected) {
            "reentrant text measurer guard smoke failed"
        }
    } finally {
        reentrantEngine.close()
    }

    val closingEngine = MermanReusableEngine()
    var closeFromCallbackRejected = false
    try {
        closingEngine.setTextMeasurer {
            if (!closeFromCallbackRejected) {
                try {
                    closingEngine.close()
                    error("callback-time close unexpectedly succeeded")
                } catch (error: MermanException) {
                    check(error.kind == MermanErrorKind.REENTRANT_CALL) {
                        "unexpected callback-time close error: ${error.message}"
                    }
                    closeFromCallbackRejected = true
                }
            }
            null
        }
        val closingSvg = closingEngine.renderSvg(textMeasureSource)
        check(closingSvg.contains("<svg") && closeFromCallbackRejected) {
            "callback-time close guard smoke failed"
        }
        check(closingEngine.renderSvg(textMeasureSource).contains("<svg")) {
            "engine was not usable after rejected callback-time close"
        }
    } finally {
        closingEngine.close()
    }

    val crossThreadEngine = MermanReusableEngine()
    try {
        var crossThreadChecked = false
        crossThreadEngine.setTextMeasurer {
            if (!crossThreadChecked) {
                var nestedError: Throwable? = null
                val nested = Thread {
                    nestedError = runCatching {
                        crossThreadEngine.renderSvg(textMeasureSource)
                    }.exceptionOrNull()
                }
                nested.start()
                nested.join(2_000)
                check(!nested.isAlive) {
                    "cross-thread callback reentry did not fail promptly"
                }
                check(
                    nestedError is MermanException &&
                        (nestedError as MermanException).kind == MermanErrorKind.REENTRANT_CALL,
                ) {
                    "cross-thread callback reentry did not preserve its typed error"
                }
                crossThreadChecked = true
            }
            null
        }
        val outerSvg = crossThreadEngine.renderSvg(textMeasureSource)
        check(outerSvg.contains("<svg") && crossThreadChecked) {
            "cross-thread callback guard smoke failed"
        }
    } finally {
        crossThreadEngine.close()
    }

    val callbackEngine = MermanReusableEngine()
    val independentEngine = MermanReusableEngine()
    try {
        var independentEngineRendered = false
        callbackEngine.setTextMeasurer {
            if (!independentEngineRendered) {
                independentEngineRendered =
                    independentEngine.renderSvg(textMeasureSource).contains("<svg")
            }
            null
        }
        val callbackSvg = callbackEngine.renderSvg(textMeasureSource)
        check(callbackSvg.contains("<svg") && independentEngineRendered) {
            "independent engine did not remain callable during a host callback"
        }
    } finally {
        callbackEngine.close()
        independentEngine.close()
    }

    val throwingEngine = MermanReusableEngine()
    try {
        throwingEngine.setTextMeasurer {
            throw IllegalStateException("host measurement failed")
        }
        val fallbackSvg = throwingEngine.renderSvg(textMeasureSource)
        check(fallbackSvg.contains("<svg") && fallbackSvg.contains("Condition?")) {
            "throwing text measurer fallback smoke failed"
        }
        throwingEngine.setTextMeasurer(null)
        val afterExceptionSvg = throwingEngine.renderSvg(textMeasureSource)
        check(afterExceptionSvg.contains("<svg") && afterExceptionSvg.contains("Condition?")) {
            "JNI exception cleanup smoke failed"
        }
    } finally {
        throwingEngine.close()
    }
}
