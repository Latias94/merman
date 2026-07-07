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
        val baselineSvg = engine.renderSvg(textMeasureSource)
        val baselineWidth = foreignObjectWidthBeforeLabel(baselineSvg, "Condition?")
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
        val measuredWidth = foreignObjectWidthBeforeLabel(reusableSvg, "Condition?")
        check(measuredWidth > baselineWidth + 40.0) {
            "text measurer callback width smoke failed: baseline=$baselineWidth measured=$measuredWidth"
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

private fun foreignObjectWidthBeforeLabel(svg: String, label: String): Double {
    val labelStart = svg.indexOf(label)
    check(labelStart >= 0) {
        "label text not found: $label"
    }
    val beforeLabel = svg.substring(0, labelStart)
    val widthMarker = "<foreignObject width=\""
    val widthStart = beforeLabel.lastIndexOf(widthMarker)
    check(widthStart >= 0) {
        "foreignObject width marker not found"
    }
    val valueStart = widthStart + widthMarker.length
    val valueEnd = svg.indexOf('"', valueStart)
    check(valueEnd > valueStart) {
        "foreignObject width end not found"
    }
    return svg.substring(valueStart, valueEnd).toDouble()
}
