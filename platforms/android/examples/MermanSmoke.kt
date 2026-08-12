package io.merman.examples

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
    listOf(
        "png" to { engine.renderPng("flowchart TD\nA --> B") },
        "jpeg" to { engine.renderJpeg("flowchart TD\nA --> B") },
        "pdf" to { engine.renderPdf("flowchart TD\nA --> B") },
        "math" to { engine.renderSvg("flowchart TD\nA[\"\$\$x^2\$\$\"] --> B") },
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
