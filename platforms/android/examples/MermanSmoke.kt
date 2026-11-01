package io.merman.examples

import io.merman.Merman
import io.merman.MermanEngine
import io.merman.MermanEngineServices
import io.merman.MermanIconPack
import io.merman.MermanIconPackSet

fun runMermanSmoke() {
    val iconPackSet = MermanIconPackSet.fromPacks(
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
    var measureCalls = 0
    val engine = MermanEngine(
        optionsJson = """{"resources":{"profile":"interactive"}}""",
        services = MermanEngineServices(
            iconPackSet = iconPackSet,
            textMeasurer = {
                measureCalls += 1
                null
            },
        ),
    )
    val iconSvg = engine.renderSvg(
        "flowchart TD\nA@{ icon: \"smoke:rocket\", label: \"Hello\" } --> B[World]",
    )
    check(iconSvg.contains("<svg") && iconSvg.contains("Hello") &&
        iconSvg.contains("android-registry") && measureCalls > 0) {
        "service-backed SVG smoke failed"
    }
    check(engine.renderAscii("flowchart TD\nA --> B").contains("A")) {
        "ASCII smoke failed"
    }
    check(engine.analyzeJson("flowchart TD\nA --> B").isNotEmpty()) {
        "analysis smoke failed"
    }
    val mathSource = "flowchart TD\nA[\"\$\$x^2\$\$\"] --> B"
    listOf(
        "one-shot" to Merman.renderSvg(mathSource),
        "reusable" to engine.renderSvg(mathSource),
    ).forEach { (consumer, svg) ->
        val factory = javax.xml.parsers.DocumentBuilderFactory.newInstance()
        factory.isNamespaceAware = true
        val document = factory.newDocumentBuilder()
            .parse(java.io.ByteArrayInputStream(svg.toByteArray(Charsets.UTF_8)))
        val viewports = document.getElementsByTagNameNS("http://www.w3.org/2000/svg", "svg")
        check(viewports.length > 1 && !svg.contains("\$\$")) {
            "$consumer did not render a formula SVG viewport"
        }
        for (index in 1 until viewports.length) {
            val viewport = viewports.item(index) as org.w3c.dom.Element
            val viewBox = viewport.getAttribute("viewBox").trim().split(Regex("[,\\s]+"))
                .map { it.toDouble() }
            val paths = viewport.getElementsByTagNameNS("http://www.w3.org/2000/svg", "path")
            check(viewBox.size == 4 && viewBox.all { it.isFinite() } &&
                viewBox[2] > 0 && viewBox[3] > 0 && (0 until paths.length).any {
                    (paths.item(it) as org.w3c.dom.Element).getAttribute("d").isNotBlank()
                }) {
                "$consumer formula lost its finite viewport or outlined glyphs"
            }
        }
    }
    MermanEngine(optionsJson = """{"resources":{"profile":"constrained"}}""").use { constrained ->
        try {
            constrained.renderSvg(mathSource)
            error("constrained engine unexpectedly admitted native math")
        } catch (error: io.merman.MermanException) {
            check(error.kind == io.merman.MermanErrorKind.MISSING_CAPABILITY &&
                error.capabilityId == "math") {
                "constrained math rejection lost its capability identity"
            }
        }
    }
    val invalidThemeSource = """
        {"styles":{"node":{"typography":{"font_stack":[]}}}}
    """.trimIndent()
    listOf(
        "one-shot" to { Merman.execute("materialize-theme-json", invalidThemeSource) },
        "reusable" to { engine.execute("materialize-theme-json", invalidThemeSource) },
    ).forEach { (consumer, operation) ->
        try {
            operation()
            error("$consumer accepted an invalid theme design")
        } catch (error: io.merman.MermanException) {
            val details = error.detailsJson ?: error("$consumer lost theme authoring details")
            val authoring = org.json.JSONObject(details).getJSONObject("theme_authoring")
            check(
                error.codeName == "MERMAN_INVALID_ARGUMENT" &&
                    authoring.getInt("schema_version") == 1 &&
                    authoring.getJSONArray("diagnostics").getJSONObject(0).let { diagnostic ->
                        diagnostic.getString("code") == "theme-authoring.invalid-token-value" &&
                            diagnostic.getString("path") == "/styles/node/typography/font_stack"
                    },
            ) {
                "$consumer lost structured theme authoring diagnostics"
            }
        }
    }
    val encodedBudgetOptions = """
        {"resources":{"profile":"constrained","limits":{"max_theme_encoded_bytes":1}}}
    """.trimIndent()
    listOf(
        "one-shot-budget" to {
            Merman.execute("materialize-theme-json", "{}", encodedBudgetOptions)
        },
        "reusable-budget" to {
            engine.execute("materialize-theme-json", "{}", encodedBudgetOptions)
        },
    ).forEach { (consumer, operation) ->
        try {
            operation()
            error("$consumer accepted a theme input over its encoded-byte budget")
        } catch (error: io.merman.MermanException) {
            val resource = error.resourceDetails ?: error("$consumer lost resource details")
            check(
                error.codeName == "MERMAN_RESOURCE_LIMIT_EXCEEDED" &&
                    resource.limitId == "max_theme_encoded_bytes" &&
                    resource.phase == "theme_input" &&
                    resource.max == 1L && resource.actual >= 2L &&
                    org.json.JSONObject(error.detailsJson ?: "{}").has("theme_authoring"),
            ) {
                "$consumer used the wrong admission path for theme resource limits"
            }
        }
    }
    listOf(
        "png" to { engine.renderPng("flowchart TD\nA --> B") },
        "jpeg" to { engine.renderJpeg("flowchart TD\nA --> B") },
        "pdf" to { engine.renderPdf("flowchart TD\nA --> B") },
    ).forEach { (capabilityId, operation) ->
        try {
            operation()
            error("default native artifact unexpectedly supports $capabilityId")
        } catch (error: io.merman.MermanException) {
            check(
                error.kind == io.merman.MermanErrorKind.MISSING_CAPABILITY &&
                    error.capabilityId == capabilityId,
            ) {
                "$capabilityId failure lost its missing-capability contract"
            }
        }
    }
    engine.close()
}
