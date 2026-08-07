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
    engine.close()
}
