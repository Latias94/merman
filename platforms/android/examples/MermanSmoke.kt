package io.merman.examples

import io.merman.MermanEngine
import io.merman.MermanReusableEngine
import io.merman.MermanTextMeasureResult

fun runMermanSmoke() {
    val source = "flowchart TD\nA[Hello] --> B[World]"

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

    val validationJson = MermanEngine.validateJson(source)
    check(validationJson.contains("\"valid\":true")) {
        "validation JSON smoke failed"
    }

    check(MermanEngine.supportedDiagramsJson().contains("flowchart")) {
        "supported diagrams smoke failed"
    }
    check(MermanEngine.asciiSupportedDiagramsJson().contains("sequence")) {
        "ASCII supported diagrams smoke failed"
    }
    check(MermanEngine.diagramFamilyCapabilitiesJson().contains("\"diagram_type\":\"flowchart\"")) {
        "diagram family capabilities smoke failed"
    }
    check(MermanEngine.supportedThemesJson().contains("default")) {
        "themes smoke failed"
    }
    check(MermanEngine.supportedHostThemePresetsJson().contains("one-dark")) {
        "host theme presets smoke failed"
    }

    val engine = MermanReusableEngine()
    try {
        engine.setTextMeasurer { request ->
            if (request.text == "Hello") {
                MermanTextMeasureResult(
                    width = 42.0,
                    height = request.lineHeight,
                    lineCount = 1,
                )
            } else {
                null
            }
        }
        val reusableSvg = engine.renderSvg(source)
        check(reusableSvg.contains("<svg") && reusableSvg.contains("Hello")) {
            "reusable engine SVG smoke failed"
        }
        engine.setTextMeasurer(null)
    } finally {
        engine.close()
    }
}
