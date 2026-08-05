package io.merman

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MermanAbi3ContractTest {
    @Test
    fun genericExecutionPreservesTheTypedResultEnvelope() {
        val result = Merman.execute("svg", "flowchart TD\nA --> B")

        check(result.operationId == "svg")
        check(result.mediaType == "image/svg+xml")
        check(result.utf8Text.contains("<svg"))
        check(result.metadata.operationId == "svg")
        check(result.metadata.mediaType == "image/svg+xml")
        check(result.metadata.byteLength == result.data.size.toLong())
        check(result.metadata.rawJson.isNotEmpty())
    }

    @Test
    fun publicSurfaceUsesOneShotMermanAndReusableMermanEngine() {
        check(runCatching { Class.forName("io.merman.MermanReusableEngine") }.isFailure) {
            "the deleted reusable-engine compatibility class must not be packaged"
        }

        val constructors = MermanEngine::class.java.declaredConstructors
        check(
            constructors.any { constructor ->
                constructor.parameterTypes.contentEquals(
                    arrayOf(String::class.java, MermanEngineServices::class.java),
                )
            },
        ) {
            "MermanEngine must own its immutable options and service configuration"
        }

        val engineMethods = MermanEngine::class.java.declaredMethods.map { it.name }.toSet()
        check("nativeTryClose" in engineMethods)
        check("setTextMeasurer" !in engineMethods)
        check("setIconRegistry" !in engineMethods)
        check("renderPngResult" in engineMethods)
        check("renderJpegResult" in engineMethods)
        check("renderPdfResult" in engineMethods)

        val oneShotExecute = Merman::class.java.getDeclaredMethod(
            "nativeExecute",
            String::class.java,
            String::class.java,
            String::class.java,
            String::class.java,
        )
        check(oneShotExecute.returnType == MermanOperationResult::class.java)

        val reusableExecute = MermanEngine::class.java.getDeclaredMethod(
            "nativeExecute",
            Long::class.javaPrimitiveType,
            String::class.java,
            String::class.java,
            String::class.java,
            String::class.java,
        )
        check(reusableExecute.returnType == MermanOperationResult::class.java)
        check(
            MermanEngine::class.java
                .getDeclaredMethod("nativeTryClose", Long::class.javaPrimitiveType)
                .returnType == Boolean::class.javaPrimitiveType,
        )
    }

    @Test
    fun iconServicesExposeOnlyImmutableConstructionAndLifecycle() {
        val packMethods = MermanIconPack::class.java.declaredMethods.map { it.name }.toSet()
        check("setJson" !in packMethods)
        check("setRegistrationName" !in packMethods)

        val registryMethods = MermanIconRegistry::class.java.declaredMethods.map { it.name }.toSet()
        check("fromPacks" in registryMethods)
        check("close" !in registryMethods)
        check("isClosed" !in registryMethods)
        check("add" !in registryMethods)
        check("remove" !in registryMethods)
        check("clear" !in registryMethods)
        check(!AutoCloseable::class.java.isAssignableFrom(MermanIconRegistry::class.java))

        val oneShotMethods = Merman::class.java.declaredMethods.map { it.name }.toSet()
        check("nativeIconRegistryNew" !in oneShotMethods)
        check("nativeIconRegistryRelease" !in oneShotMethods)

        val nativeNew = MermanEngine::class.java.getDeclaredMethod(
            "nativeNew",
            String::class.java,
            Array<String>::class.java,
            Array<String>::class.java,
            MermanTextMeasurer::class.java,
        )
        check(nativeNew.returnType == Long::class.javaPrimitiveType)

        val servicesMethods = MermanEngineServices::class.java.declaredMethods.map { it.name }.toSet()
        check("setIconRegistry" !in servicesMethods)
        check("setTextMeasurer" !in servicesMethods)
    }

    @Test
    fun generatedOperationMatrixIsCompleteAndNamedHelpersStayAligned() {
        check(MERMAN_BINDING_OPERATION_EXPECTATIONS.size == 13)
        check(MERMAN_BINDING_OPERATION_EXPECTATIONS.map { it.operationId }.toSet().size == 13)

        val oneShotMethods = Merman::class.java.methods.map { it.name }.toSet()
        val reusableMethods = MermanEngine::class.java.methods.map { it.name }.toSet()
        for (name in listOf("analysisFactsJson", "svgPlanJson", "metadataJson", "execute")) {
            check(name in oneShotMethods) { "Merman is missing $name" }
        }
        for (name in listOf("analysisFactsJson", "svgPlanJson", "execute")) {
            check(name in reusableMethods) { "MermanEngine is missing $name" }
        }
    }
}
