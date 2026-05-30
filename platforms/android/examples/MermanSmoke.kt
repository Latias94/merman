package io.merman.examples

import io.merman.MermanEngine

fun runMermanSmoke() {
    val source = "flowchart TD\nA[Hello] --> B[World]"

    val svg = MermanEngine.renderSvg(source)
    check(svg.contains("<svg") && svg.contains("Hello") && svg.contains("World")) {
        "SVG smoke failed"
    }

    val semanticJson = MermanEngine.parseJson(source)
    check(semanticJson.contains("flowchart-v2")) {
        "semantic JSON smoke failed"
    }

    val layoutJson = MermanEngine.layoutJson(source)
    check(layoutJson.contains("layout")) {
        "layout JSON smoke failed"
    }
}
