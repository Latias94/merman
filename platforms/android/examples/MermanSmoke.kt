package io.merman.examples

import io.merman.MermanEngine
import io.merman.MermanException
import io.merman.MermanReusableEngine
import io.merman.MermanTextMeasureResult

fun runMermanSmoke() {
    val source = "flowchart TD\nA[Hello] --> B[World]"
    val textMeasureSource = "flowchart TD\nA[Start] --> B{Condition?}"

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
    check(documentFactsJson.contains("\"source_id\":\"mermaid-fence-1\"")) {
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
        val baselineLayoutJson = engine.layoutJson(textMeasureSource)
        val baselineWidth = flowchartNodeWidth(baselineLayoutJson, "B")
        engine.setTextMeasurer { request ->
            measureCalls += 1
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
                MermanTextMeasureResult(
                    width = 140.0,
                    height = 24.0,
                    lineCount = 1,
                )
            } else {
                null
            }
        }
        val reusableSvg = engine.renderSvg(textMeasureSource)
        check(reusableSvg.contains("<svg") && reusableSvg.contains("Condition?")) {
            "reusable engine SVG smoke failed"
        }
        val measuredLayoutJson = engine.layoutJson(textMeasureSource)
        val measuredWidth = flowchartNodeWidth(measuredLayoutJson, "B")
        check(measuredWidth > baselineWidth + 40.0) {
            "text measurer callback layout width smoke failed: baseline=$baselineWidth measured=$measuredWidth"
        }
        check(measureCalls > 0) {
            "text measurer callback smoke failed"
        }
        check(sawCondition && sawNowrap && sawBreakSpaces && sawFontStyle && sawSpacingDefaults) {
            "text measurer request metadata smoke failed"
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
        check(reusableDocumentFactsJson.contains("\"source_id\":\"mermaid-fence-1\"")) {
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
                    check(error.message?.contains("re-entered") == true) {
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
    var closeFromCallbackObserved = false
    try {
        closingEngine.setTextMeasurer {
            if (!closeFromCallbackObserved) {
                closeFromCallbackObserved = true
                closingEngine.close()
            }
            null
        }
        val closingSvg = closingEngine.renderSvg(textMeasureSource)
        check(closingSvg.contains("<svg") && closeFromCallbackObserved) {
            "close-from-callback render smoke failed"
        }
        try {
            closingEngine.renderSvg(textMeasureSource)
            error("closed reusable engine unexpectedly rendered")
        } catch (error: MermanException) {
            check(error.message?.contains("closed") == true) {
                "unexpected closed-engine error: ${error.message}"
            }
        }
    } finally {
        closingEngine.close()
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

private fun flowchartNodeWidth(layoutJson: String, nodeId: String): Double {
    val layoutStart = layoutJson.indexOf("\"FlowchartV2\"")
    check(layoutStart >= 0) {
        "FlowchartV2 layout not found"
    }
    val layoutSection = layoutJson.substring(layoutStart)
    val nodesStart = layoutSection.indexOf("\"nodes\"")
    val edgesStart = layoutSection.indexOf("\"edges\"", startIndex = nodesStart)
    check(nodesStart >= 0 && edgesStart > nodesStart) {
        "FlowchartV2 nodes section not found"
    }
    val nodesSection = layoutSection.substring(nodesStart, edgesStart)
    val nodePattern = Regex("""\{[^{}]*"id"\s*:\s*"$nodeId"[^{}]*}""")
    val node = nodePattern.find(nodesSection)?.value
    check(node != null) {
        "FlowchartV2 node not found: $nodeId"
    }
    val widthPattern = Regex(""""width"\s*:\s*(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)""")
    val width = widthPattern.find(node)?.groupValues?.get(1)?.toDoubleOrNull()
    check(width != null) {
        "FlowchartV2 node width not found: $nodeId"
    }
    return width
}
