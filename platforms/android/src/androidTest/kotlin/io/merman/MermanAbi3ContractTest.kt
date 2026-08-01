package io.merman

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.json.JSONObject
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MermanAbi3ContractTest {
    @Test
    fun genericExecutionPreservesTheStructuredResultEnvelope() {
        val result = MermanEngine.execute("svg", "flowchart TD\nA --> B")

        check(result.operationId == "svg")
        check(result.mediaType == "image/svg+xml")
        check(result.data.decodeToString().contains("<svg"))
        check(JSONObject(result.metadataJson).getString("operation_id") == "svg")
    }

    @Test
    fun reusableSurfaceUsesImmutableCallbackAndTryClose() {
        val constructors = MermanReusableEngine::class.java.declaredConstructors
        check(
            constructors.any { constructor ->
                constructor.parameterTypes.contentEquals(
                    arrayOf(String::class.java, MermanTextMeasurer::class.java),
                )
            },
        ) {
            "reusable engine constructor must own its optional callback"
        }
        val names = MermanReusableEngine::class.java.declaredMethods.map { it.name }.toSet()
        check("nativeTryClose" in names)
        check("setTextMeasurer" !in names)
        check("executeBytes" !in names)

        val oneShotExecute = MermanEngine::class.java.getDeclaredMethod(
            "nativeExecute",
            String::class.java,
            String::class.java,
            String::class.java,
            String::class.java,
        )
        check(oneShotExecute.returnType == MermanOperationResult::class.java)

        val reusableExecute = MermanReusableEngine::class.java.getDeclaredMethod(
            "nativeExecute",
            Long::class.javaPrimitiveType,
            String::class.java,
            String::class.java,
            String::class.java,
            String::class.java,
        )
        check(reusableExecute.returnType == MermanOperationResult::class.java)
        check(
            MermanReusableEngine::class.java
                .getDeclaredMethod("nativeTryClose", Long::class.javaPrimitiveType)
                .returnType == Boolean::class.javaPrimitiveType,
        )
    }
}
