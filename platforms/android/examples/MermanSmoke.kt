package io.merman.examples

import io.merman.MERMAN_BINDING_OPERATION_EXPECTATIONS
import io.merman.Merman
import io.merman.MermanEngine
import io.merman.MermanEngineServices
import io.merman.MermanErrorKind
import io.merman.MermanException
import io.merman.MermanIconPack
import io.merman.MermanIconRegistry
import io.merman.MermanPdfFilterImagesOutputPlan
import io.merman.MermanRasterOutputPlan
import io.merman.MermanTextMeasureResult
import io.merman.MermanTextMeasurer
import io.merman.MermanTextMeasurementOperation
import io.merman.MermanTextMeasurementResultKind
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONObject

private val expectedRuntimeCapabilities = setOf(
    "analysis",
    "ascii",
    "jpeg",
    "layout-cytoscape",
    "layout-elk",
    "math",
    "pdf",
    "png",
    "svg",
    "system-clock",
    "system-random",
    "system-timezone",
)
private val expectedRuntimeOutputs = setOf("ascii", "jpeg", "pdf", "png", "svg")
private val expectedRuntimeOperations =
    MERMAN_BINDING_OPERATION_EXPECTATIONS.mapTo(linkedSetOf()) { it.operationId }
private val expectedSystemAdapters = setOf(
    "system-clock",
    "system-random",
    "system-timezone",
)

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
    val textMeasureCoverageSource = """
        architecture-beta
          group app(cloud)[Application platform]
          service api(server)[API service] in app
          service db(database)[Data store] in app
          api:R -[request path]- L:db
    """.trimIndent()

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

    val earlyReusableEngine = MermanEngine()
    try {
        val earlyReusableAnalysisJson = earlyReusableEngine.analyzeJson(source)
        check(earlyReusableAnalysisJson.contains("\"valid\":true")) {
            "reusable-first analysis smoke failed"
        }
    } finally {
        earlyReusableEngine.close()
    }

    val svg = Merman.renderSvg(source)
    check(svg.contains("<svg") && svg.contains("Hello") && svg.contains("World")) {
        "SVG smoke failed"
    }

    val ascii = Merman.renderAscii(source)
    check(ascii.contains("Hello") && ascii.contains("World")) {
        "ASCII smoke failed"
    }

    val semanticJson = Merman.parseJson(source)
    check(semanticJson.contains("flowchart-v2")) {
        "semantic JSON smoke failed"
    }

    val operationResult = Merman.execute("svg", source)
    check(
        operationResult.operationId == "svg" &&
            operationResult.mediaType == "image/svg+xml" &&
            operationResult.data.contentEquals(svg.toByteArray()) &&
            operationResult.metadata.operationId == "svg" &&
            operationResult.metadata.rawJson.isNotEmpty(),
    ) {
        "structured operation result smoke failed"
    }

    check(Merman.metadataJson("supported-diagrams") == Merman.supportedDiagramsJson()) {
        "generic metadata dispatch disagrees with its named helper"
    }
    check(Merman.analysisFactsJson(source).contains("\"version\":1")) {
        "analysis facts JSON smoke failed"
    }
    check(Merman.svgPlanJson(source).contains("\"schema_version\":1")) {
        "SVG plan JSON smoke failed"
    }

    val limitedPngOptions = """
        {
          "version":2,
          "raster":{"scale":20},
          "resources":{"limits":{"max_raster_pixels":4096}}
        }
    """.trimIndent()
    val pngResult = Merman.renderPngResult(source, limitedPngOptions)
    val pngPlan = pngResult.metadata.outputPlan as MermanRasterOutputPlan
    check(
        pngResult.data.startsWithBytes(0x89, 0x50, 0x4e, 0x47) &&
            pngPlan.limited &&
            pngPlan.widthPx.toLong() * pngPlan.heightPx <= 4096 &&
            pngResult.data.contentEquals(Merman.renderPng(source, limitedPngOptions)),
    ) {
        "typed PNG result smoke failed"
    }
    val jpegResult = Merman.renderJpegResult(source)
    check(
        jpegResult.data.startsWithBytes(0xff, 0xd8, 0xff) &&
            jpegResult.metadata.outputPlan is MermanRasterOutputPlan,
    ) {
        "typed JPEG result smoke failed"
    }
    val pdfOptions = """{"version":2,"pdf":{"filterScale":0.1}}"""
    val pdfResult = Merman.renderPdfResult(source, pdfOptions)
    check(
        pdfResult.data.startsWithBytes(0x25, 0x50, 0x44, 0x46) &&
            pdfResult.metadata.outputPlan is MermanPdfFilterImagesOutputPlan &&
            pdfResult.data.contentEquals(Merman.renderPdf(source, pdfOptions)),
    ) {
        "typed PDF result smoke failed"
    }

    val unknownOperationError = runCatching {
        Merman.execute("not-an-operation", source)
    }.exceptionOrNull()
    check(
        unknownOperationError is MermanException &&
            unknownOperationError.kind == MermanErrorKind.UNKNOWN_OPERATION &&
            unknownOperationError.capabilityId == null,
    ) {
        "unknown operation did not preserve its machine-readable binding error"
    }

    val layoutJson = Merman.layoutJson(source)
    check(layoutJson.contains("layout")) {
        "layout JSON smoke failed"
    }

    val analysisJson = Merman.analyzeJson(source)
    check(analysisJson.contains("\"version\":1") && analysisJson.contains("\"valid\":true")) {
        "analysis JSON smoke failed"
    }

    val validationJson = Merman.validateJson(source)
    check(validationJson.contains("\"valid\":true")) {
        "validation JSON smoke failed"
    }

    val documentSource = "Intro\n```mermaid\n$source\n```\n"
    val documentJson = Merman.analyzeDocumentJson(
        documentSource,
        "file:///tmp/example.md",
    )
    check(documentJson.contains("\"kind\":\"markdown\"") && documentJson.contains("\"valid\":true")) {
        "document analysis JSON smoke failed"
    }
    val documentFactsJson = Merman.analyzeDocumentFactsJson(
        documentSource,
        "file:///tmp/example.md",
    )
    check(
        documentFactsJson.contains("\"version\":1") &&
            documentFactsJson.contains("\"source_id\":\"mermaid-fence-1\""),
    ) {
        "document facts JSON smoke failed"
    }

    check(Merman.supportedDiagramsJson().contains("flowchart")) {
        "supported diagrams smoke failed"
    }
    check(Merman.asciiCapabilitiesJson().contains("\"support_level\":\"summary\"")) {
        "ASCII capabilities smoke failed"
    }
    check(Merman.diagramFamilyCapabilitiesJson().contains("\"diagram_type\":\"flowchart\"")) {
        "diagram family capabilities smoke failed"
    }
    val runtimeCatalogJson = Merman.runtimeCatalogJson()
    val runtimeCatalog = JSONObject(runtimeCatalogJson)
    val runtimeCapabilities = runtimeCatalog.getJSONObject("capabilities")
    val runtimeOutputContracts = runtimeCatalog.getJSONArray("output_contracts").objectMapById()
    check(
        runtimeCatalog.getInt("schema_version") == 1 &&
            runtimeCatalog.getInt("transport_api_version") == 1 &&
            runtimeCatalog.getJSONObject("registry").getInt("diagram_family_count") > 0 &&
            runtimeCatalog.getJSONObject("resources")
                .getString("general_binding_default_profile") == "interactive" &&
            runtimeCapabilities.getJSONArray("capability_ids").stringSet() ==
            expectedRuntimeCapabilities &&
            runtimeCapabilities.getJSONArray("output_ids").stringSet() ==
            expectedRuntimeOutputs &&
            runtimeCapabilities.getJSONArray("operation_ids").stringSet() ==
            expectedRuntimeOperations &&
            runtimeCapabilities.getJSONArray("system_adapter_ids").stringSet() ==
            expectedSystemAdapters &&
            runtimeOutputContracts.keys == expectedRuntimeOutputs,
    ) {
        "runtime catalog smoke failed"
    }
    check(
        runtimeOutputContracts.getValue("ascii").getString("media_type") ==
            "text/plain; charset=utf-8" &&
            runtimeOutputContracts.getValue("svg").getString("media_type") == "image/svg+xml" &&
            runtimeOutputContracts.getValue("ascii").isNull("system_fonts") &&
            runtimeOutputContracts.getValue("ascii").isNull("embedded_images") &&
            runtimeOutputContracts.getValue("svg").isNull("system_fonts") &&
            runtimeOutputContracts.getValue("svg").isNull("embedded_images"),
    ) {
        "text output environment contract smoke failed"
    }
    for ((id, mediaType) in mapOf(
        "jpeg" to "image/jpeg",
        "pdf" to "application/pdf",
        "png" to "image/png",
    )) {
        checkBinaryOutputContract(runtimeOutputContracts.getValue(id), mediaType)
    }
    check(Merman.lintRuleCatalogJson().contains("\"version\":1")) {
        "lint rule catalog envelope smoke failed"
    }
    check(Merman.lintRuleCatalogJson().contains("\"rules\":")) {
        "lint rule catalog rules envelope smoke failed"
    }
    check(Merman.lintRuleCatalogJson().contains("merman.authoring.flowchart.explicit_direction")) {
        "lint rule catalog smoke failed"
    }
    check(Merman.lintRuleCatalogJson().contains("docs/adr/0072-lint-rule-governance.md")) {
        "lint rule catalog evidence smoke failed"
    }
    check(Merman.supportedThemesJson().contains("default")) {
        "themes smoke failed"
    }
    check(Merman.presentationCatalogJson().contains("one-dark")) {
        "presentation catalog theme preset smoke failed"
    }
    check(Merman.presentationCatalogJson().contains("merman-modern")) {
        "presentation catalog profile smoke failed"
    }

    val iconRegistry = MermanIconRegistry.fromPacks(
        listOf(
            MermanIconPack(
                json = """
                    {
                      "icons":{
                        "rocket":{
                          "body":"<path data-icon=\"android-registry\" d=\"M0 0H16V16H0z\"/>"
                        }
                      }
                    }
                """.trimIndent(),
                registrationName = "smoke",
            ),
        ),
    )
    val iconServices = MermanEngineServices(iconRegistry = iconRegistry)
    val iconEngines = listOf(
        MermanEngine(services = iconServices),
        MermanEngine(services = iconServices),
    )
    val iconSource = "flowchart TD\nA@{ icon: \"smoke:rocket\", label: \"A\" }"
    for ((index, iconEngine) in iconEngines.withIndex()) {
        try {
            check(iconEngine.renderSvg(iconSource).contains("android-registry")) {
                "icon registry snapshot reuse failed for engine $index"
            }
        } finally {
            iconEngine.close()
        }
    }
    MermanEngine(services = iconServices).use { iconEngine ->
        check(iconEngine.renderSvg(iconSource).contains("android-registry")) {
            "icon registry snapshot could not be reused after earlier engines closed"
        }
    }
    val invalidRegistry =
        MermanIconRegistry.fromPacks(
            listOf(MermanIconPack("""{"prefix":"bad","icons":{"broken":{"body":"<path>"}}}""")),
        )
    check(
        runCatching {
            MermanEngine(services = MermanEngineServices(iconRegistry = invalidRegistry)).use { }
        }.exceptionOrNull() is MermanException,
    ) {
        "invalid icon XML unexpectedly published an engine"
    }

    var conflictCallbackCalls = 0
    val conflictError = runCatching {
        MermanEngine(
            optionsJson = """{"environment":{"text_measurement":"deterministic"}}""",
            services = MermanEngineServices(
                textMeasurer = {
                    conflictCallbackCalls += 1
                    null
                },
            ),
        )
    }.exceptionOrNull()
    check(conflictError is MermanException && conflictCallbackCalls == 0) {
        "constructor service conflict must fail before invoking the callback"
    }

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

    val engine = engineWithTextMeasurer { request ->
            measureCalls += 1
            if (seenMeasureTexts.size < 8) {
                seenMeasureTexts += request.text
            }
            if (seenWrapModes.size < 8) {
                seenWrapModes += request.wrapMode.code
            }
            seenPhases += request.phase.code
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
    try {
        val perCallOptionsSvg = engine.renderSvg(
            source,
            """{"svg":{"diagram_id":"android-reusable-options"}}""",
        )
        check(perCallOptionsSvg.contains("""id="android-reusable-options"""")) {
            "reusable engine did not apply per-call options"
        }
        val reusableSvg = engine.renderSvg(textMeasureSource)
        check(reusableSvg.contains("<svg") && reusableSvg.contains("Condition?")) {
            "reusable engine SVG smoke failed"
        }
        val coverageSvg = engine.renderSvg(textMeasureCoverageSource)
        check(coverageSvg.contains("<svg")) {
            "multi-phase text measurement SVG smoke failed"
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
    } finally {
        engine.close()
    }

    lateinit var reentrantEngine: MermanEngine
    var reentryRejected = false
    reentrantEngine = engineWithTextMeasurer {
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
    try {
        val reentrantSvg = reentrantEngine.renderSvg(textMeasureSource)
        check(reentrantSvg.contains("<svg") && reentryRejected) {
            "reentrant text measurer guard smoke failed"
        }
    } finally {
        reentrantEngine.close()
    }

    lateinit var closingEngine: MermanEngine
    var closeFromCallbackRejected = false
    closingEngine = engineWithTextMeasurer {
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
    try {
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

    lateinit var crossThreadEngine: MermanEngine
    var crossThreadChecked = false
    crossThreadEngine = engineWithTextMeasurer {
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
    try {
        val outerSvg = crossThreadEngine.renderSvg(textMeasureSource)
        check(outerSvg.contains("<svg") && crossThreadChecked) {
            "cross-thread callback guard smoke failed"
        }
    } finally {
        crossThreadEngine.close()
    }

    var independentEngineRendered = false
    val independentEngine = MermanEngine()
    val callbackEngine = engineWithTextMeasurer {
            if (!independentEngineRendered) {
                independentEngineRendered =
                    independentEngine.renderSvg(textMeasureSource).contains("<svg")
            }
            null
        }
    try {
        val callbackSvg = callbackEngine.renderSvg(textMeasureSource)
        check(callbackSvg.contains("<svg") && independentEngineRendered) {
            "independent engine did not remain callable during a host callback"
        }
    } finally {
        callbackEngine.close()
        independentEngine.close()
    }

    val throwingEngine = engineWithTextMeasurer {
            throw IllegalStateException("host measurement failed")
        }
    try {
        val fallbackSvg = throwingEngine.renderSvg(textMeasureSource)
        check(fallbackSvg.contains("<svg") && fallbackSvg.contains("Condition?")) {
            "throwing text measurer fallback smoke failed"
        }
        val afterExceptionSvg = throwingEngine.renderSvg(textMeasureSource)
        check(afterExceptionSvg.contains("<svg") && afterExceptionSvg.contains("Condition?")) {
            "JNI exception cleanup smoke failed"
        }
    } finally {
        throwingEngine.close()
    }

    val concurrentEngine = MermanEngine()
    try {
        val start = CountDownLatch(1)
        val done = CountDownLatch(2)
        val failures = Collections.synchronizedList(mutableListOf<Throwable>())
        repeat(2) {
            Thread {
                try {
                    start.await()
                    check(concurrentEngine.parseJson(source).contains("flowchart-v2"))
                } catch (error: Throwable) {
                    failures += error
                } finally {
                    done.countDown()
                }
            }.start()
        }
        start.countDown()
        check(done.await(10, TimeUnit.SECONDS)) {
            "callback-free concurrent operations did not complete"
        }
        check(failures.isEmpty()) {
            "callback-free concurrent operations failed: $failures"
        }
    } finally {
        concurrentEngine.close()
    }

    val concurrentlyClosedEngine = MermanEngine()
    val closeStart = CountDownLatch(1)
    val closeDone = CountDownLatch(2)
    val closeFailures = Collections.synchronizedList(mutableListOf<Throwable>())
    repeat(2) {
        Thread {
            try {
                closeStart.await()
                concurrentlyClosedEngine.close()
            } catch (error: Throwable) {
                closeFailures += error
            } finally {
                closeDone.countDown()
            }
        }.start()
    }
    closeStart.countDown()
    check(closeDone.await(10, TimeUnit.SECONDS) && closeFailures.isEmpty()) {
        "concurrent idempotent close failed: $closeFailures"
    }
    concurrentlyClosedEngine.close()
    check(
        runCatching { concurrentlyClosedEngine.parseJson(source) }
            .exceptionOrNull() is MermanException,
    ) {
        "post-close engine call unexpectedly succeeded"
    }

    val admissionSource = buildString {
        append("flowchart TD\n")
        repeat(8_000) { index ->
            append("n")
            append(index)
            append("-->n")
            append(index + 1)
            append('\n')
        }
    }
    val busyEngine = engineWithTextMeasurer { null }
    try {
        val start = CountDownLatch(1)
        val done = CountDownLatch(2)
        val outcomes = Collections.synchronizedList(mutableListOf<Throwable?>())
        repeat(2) {
            Thread {
                start.await()
                outcomes += runCatching {
                    busyEngine.analyzeJson(admissionSource)
                }.exceptionOrNull()
                done.countDown()
            }.start()
        }
        start.countDown()
        check(done.await(30, TimeUnit.SECONDS)) {
            "callback-enabled admission smoke timed out"
        }
        check(
            outcomes.count { it == null } == 1 &&
                outcomes.count {
                    it is MermanException && it.kind == MermanErrorKind.BUSY
                } == 1,
        ) {
            "callback-enabled engine did not reject its competitor with BUSY: $outcomes"
        }
    } finally {
        busyEngine.close()
    }

    var closeBusyObserved = false
    repeat(3) {
        if (closeBusyObserved) return@repeat
        val closeEngine = engineWithTextMeasurer { null }
        val started = CountDownLatch(1)
        val finished = CountDownLatch(1)
        val worker = Thread {
            started.countDown()
            runCatching {
                closeEngine.analyzeJson(admissionSource)
            }
            finished.countDown()
        }
        worker.start()
        check(started.await(2, TimeUnit.SECONDS))
        Thread.sleep(2)
        val closeError = runCatching {
            closeEngine.close()
        }.exceptionOrNull()
        check(finished.await(30, TimeUnit.SECONDS)) {
            "operation did not finish after a close attempt"
        }
        if (closeError is MermanException && closeError.kind == MermanErrorKind.BUSY) {
            closeBusyObserved = true
            closeEngine.close()
        } else if (closeError != null) {
            throw closeError
        }
    }
    check(closeBusyObserved) {
        "nonblocking close did not expose BUSY during an active operation"
    }
}

private fun engineWithTextMeasurer(textMeasurer: MermanTextMeasurer): MermanEngine =
    MermanEngine(services = MermanEngineServices(textMeasurer = textMeasurer))

private fun ByteArray.startsWithBytes(vararg prefix: Int): Boolean =
    size >= prefix.size &&
        prefix.indices.all { index -> (this[index].toInt() and 0xff) == prefix[index] }

private fun JSONArray.stringSet(): Set<String> =
    buildSet {
        for (index in 0 until length()) {
            add(getString(index))
        }
    }

private fun JSONArray.objectMapById(): Map<String, JSONObject> =
    buildMap {
        for (index in 0 until length()) {
            val value = getJSONObject(index)
            put(value.getString("id"), value)
        }
    }

private fun checkBinaryOutputContract(contract: JSONObject, mediaType: String) {
    val fonts = contract.getJSONObject("system_fonts")
    val images = contract.getJSONObject("embedded_images")
    val limits = images.getJSONObject("limits")
    check(
        contract.getString("media_type") == mediaType &&
            fonts.getString("source_id") == "host-system" &&
            fonts.getString("discovery") == "first-use" &&
            fonts.getString("cache_scope") == "process-global" &&
            fonts.getBoolean("host_dependent") &&
            !fonts.getBoolean("caller_configurable") &&
            !fonts.getBoolean("resource_bounded") &&
            images.getJSONArray("source_ids").stringSet() == setOf("data-url") &&
            !images.getBoolean("filesystem_access") &&
            !images.getBoolean("network_access") &&
            images.getBoolean("caller_configurable") &&
            limits.getLong("max_bytes_per_image") == 16L * 1024 * 1024 &&
            limits.getLong("max_total_bytes") == 32L * 1024 * 1024 &&
            limits.getLong("max_pixels_per_image") == 16L * 1024 * 1024 &&
            limits.getLong("max_total_pixels") == 32L * 1024 * 1024,
    ) {
        "binary output environment contract smoke failed for ${contract.getString("id")}"
    }
}
