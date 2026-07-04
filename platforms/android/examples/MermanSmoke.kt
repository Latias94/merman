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
}
