package io.merman.examples

import io.merman.Merman
import io.merman.MermanEngine
import io.merman.MermanEngineServices
import io.merman.MermanException
import io.merman.MermanIconPack
import io.merman.MermanIconPackSet
import io.merman.MermanOperationResult
import org.json.JSONObject

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
    val catalog = JSONObject(Merman.themeCatalogJson())
    check(catalog.getInt("schema_version") == 2) { "unexpected theme catalog schema" }
    val presets = catalog.getJSONArray("presets")
    val studio = (0 until presets.length()).map(presets::getJSONObject)
        .single { it.getString("id") == "studio" }
    val appearances = studio.getJSONArray("appearances")
    check((0 until appearances.length()).map(appearances::getString) == listOf("light", "dark") &&
        studio.getString("default_appearance") == "light") {
        "Studio lost its declared appearances"
    }
    checkThemeV2("one-shot") { operation, source, options -> Merman.execute(operation, source, options) }
    checkThemeV2("reusable") { operation, source, options -> engine.execute(operation, source, options) }
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

private fun checkThemeV2(
    consumer: String,
    execute: (String, String, String?) -> MermanOperationResult,
) {
    val exported = execute("export-theme-preset", "studio", null)
    val documentText = exported.utf8Text
    val document = JSONObject(documentText)
    val appearances = document.getJSONObject("appearances")
    check(exported.mediaType == "application/json" && document.getInt("schema_version") == 2 &&
        document.getString("id") == "studio" && appearances.has("light") && appearances.has("dark")) {
        "$consumer did not export an editable paired preset"
    }

    val resolutions = mutableListOf<String>()
    val drawings = mutableListOf<String>()
    listOf("light", "dark", "light").forEachIndexed { index, appearance ->
        val request = JSONObject()
            .put("document", if (index == 2) documentText else document)
            .put("appearance", appearance)
        val result = execute("resolve-theme", request.toString(), null).utf8Text
        val resolution = JSONObject(result)
        check(resolution.getBoolean("ok") &&
            resolution.getJSONObject("resolved").getString("appearance") == appearance &&
            resolution.optJSONArray("notices") != null) {
            "$consumer resolved the wrong appearance: $resolution"
        }
        resolutions.add(result)

        val options = JSONObject()
            .put("theme", JSONObject().put("document", document).put("appearance", appearance))
            .put("site_config", JSONObject().put("htmlLabels", false).put("layout", "dagre"))
            .put("svg", JSONObject().put("diagram_id", "android-paired-theme"))
        val drawing = execute("svg", "flowchart LR\nA[Review] --> B[Publish]", options.toString())
        val outcome = drawing.metadata.jsonObject.getJSONObject("theme_outcome")
        check(outcome.getString("mode") == "merman_native" &&
            outcome.getString("appearance") == appearance &&
            outcome.optJSONArray("notices") != null) {
            "$consumer SVG lost its theme outcome: $outcome"
        }
        drawings.add(drawing.utf8Text)
    }
    check(resolutions[0] != resolutions[1] && resolutions[0] == resolutions[2] &&
        drawings[0] != drawings[1] && drawings[0] == drawings[2]) {
        "$consumer lost paired-theme isolation"
    }

    fun checkDocumentError(raw: String, column: Int, path: String, message: String) {
        val request = JSONObject().put("document", raw)
        val diagnostic = JSONObject(execute("resolve-theme", request.toString(), null).utf8Text)
        check(!diagnostic.getBoolean("ok") && diagnostic.getInt("line") == 3 &&
            diagnostic.getInt("column") == column && diagnostic.getString("path") == path &&
            diagnostic.getString("message").contains(message)) {
            "$consumer lost the theme document diagnostic: $diagnostic"
        }
    }
    checkDocumentError("{\n  \"schema_version\": 2,\n  \"extends\": ]\n}", 14, "extends", "expected value")
    checkDocumentError("{\n  \"schema_version\": 2,\n  \"schema_version\": 2\n}", 18, ".", "duplicate key")

    try {
        execute("resolve-theme", "{}", null)
        error("$consumer accepted a resolve-theme request without a document")
    } catch (error: MermanException) {
        check(error.codeName == "MERMAN_INVALID_ARGUMENT") {
            "$consumer lost the invalid request error identity"
        }
    }
}
