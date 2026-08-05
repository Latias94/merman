package io.merman

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class MermanExceptionTest {
    @Test
    fun parsesStructuredIconRegistryFailureDetails() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":2,"code_name":"MERMAN_INVALID_ARGUMENT","kind":"invalid_argument","capability_id":null,"details":{"icon_registry":{"kind_id":"duplicate-registration-name","pack_index":3,"registration_name":"logos"}},"message":"duplicate icon registry name"}""",
        )

        assertEquals(
            MermanIconRegistryErrorDetails(
                kindId = "duplicate-registration-name",
                packIndex = 3,
                registrationName = "logos",
            ),
            error.iconRegistryDetails,
        )
        assertNull(error.resourceDetails)
    }

    @Test
    fun rejectsMalformedIconRegistryFailureDetails() {
        val error = MermanException(
            """{"details":{"icon_registry":{"kind_id":"","pack_index":-1}}}""",
        )

        assertNull(error.iconRegistryDetails)
    }

    @Test
    fun iconRegistryFactoryHasNoNativeLifecycle() {
        val registry = MermanIconRegistry.fromPacks(
            listOf(MermanIconPack("""{"prefix":"test","icons":{}}""")),
        )

        assertTrue(!AutoCloseable::class.java.isAssignableFrom(registry.javaClass))
        assertTrue("close" !in registry.javaClass.declaredMethods.map { it.name })
    }

    @Test
    fun rejectsTooManyIconPacksBeforeCopyingOrLoadingNativeCode() {
        val error = runCatching {
            MermanIconRegistry.fromPacks(
                List(17) { index ->
                    MermanIconPack("""{"prefix":"p$index","icons":{}}""")
                },
            )
        }.exceptionOrNull()

        assertTrue(error is MermanException)
        error as MermanException
        assertNull(error.code)
        assertNull(error.codeName)
        assertEquals("max_icon_registry_packs", error.resourceDetails?.limitId)
        assertEquals(17L, error.resourceDetails?.actual)
        assertEquals("resource_limit_exceeded", error.iconRegistryDetails?.kindId)
        assertNull(error.iconRegistryDetails?.packIndex)
    }

    @Test
    fun iconPackRejectsEmptyInputValues() {
        assertTrue(runCatching { MermanIconPack("") }.exceptionOrNull() is IllegalArgumentException)
        assertTrue(
            runCatching {
                MermanIconPack("""{"prefix":"test","icons":{}}""", "")
            }.exceptionOrNull() is IllegalArgumentException,
        )
    }
}
