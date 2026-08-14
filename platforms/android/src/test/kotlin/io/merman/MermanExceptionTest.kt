package io.merman

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class MermanExceptionTest {
    @Test
    fun parsesStructuredResourceFailureDetails() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","kind":"generic","capability_id":null,"details":{"resource":{"cause":"arithmetic_overflow","limit_id":"max_embedded_image_bytes","phase":"embedded_image_decode","actual":5,"max":4,"profile":"constrained"}},"message":"embedded image is too large"}""",
        )

        assertEquals(
            MermanResourceErrorDetails(
                cause = "arithmetic_overflow",
                limitId = "max_embedded_image_bytes",
                phase = "embedded_image_decode",
                actual = 5,
                max = 4,
                profile = "constrained",
            ),
            error.resourceDetails,
        )
        assertEquals(
            MermanExactResourceErrorDetails(
                cause = "arithmetic_overflow",
                limitId = "max_embedded_image_bytes",
                phase = "embedded_image_decode",
                actual = "5",
                max = "4",
                profile = "constrained",
            ),
            error.exactResourceDetails,
        )
    }

    @Test
    fun preservesUnsignedResourceCountsBeyondLongRange() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","kind":"generic","capability_id":null,"details":{"resource":{"cause":"arithmetic_overflow","limit_id":"max_layout_work_units","phase":"layout_model","actual":"18446744073709551615","max":"9223372036854775808","profile":"interactive"}},"message":"layout work accounting overflowed"}""",
        )

        assertEquals(
            MermanExactResourceErrorDetails(
                cause = "arithmetic_overflow",
                limitId = "max_layout_work_units",
                phase = "layout_model",
                actual = "18446744073709551615",
                max = "9223372036854775808",
                profile = "interactive",
            ),
            error.exactResourceDetails,
        )
        assertNull(error.resourceDetails)
    }

    @Test
    fun keepsLongCompatibilityForStringEncodedLongRangeCounts() {
        val error = MermanException(
            """{"details":{"resource":{"cause":"ceiling","limit_id":"max_layout_work_units","phase":"layout_model","actual":"9007199254740992","max":"9007199254740991","profile":"interactive"}}}""",
        )

        assertEquals("9007199254740992", error.exactResourceDetails?.actual)
        assertEquals("9007199254740991", error.exactResourceDetails?.max)
        assertEquals(9_007_199_254_740_992L, error.resourceDetails?.actual)
        assertEquals(9_007_199_254_740_991L, error.resourceDetails?.max)
    }

    @Test
    fun rejectsResourceCountsOutsideUnsignedLongRange() {
        val error = MermanException(
            """{"details":{"resource":{"cause":"ceiling","limit_id":"max_source_bytes","phase":"source","actual":"18446744073709551616","max":"4","profile":"interactive"}}}""",
        )

        assertNull(error.exactResourceDetails)
        assertNull(error.resourceDetails)
    }

    @Test
    fun parsesStructuredIconRegistryFailureDetails() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":1,"code_name":"MERMAN_INVALID_ARGUMENT","kind":"generic","capability_id":null,"details":{"icon_registry":{"kind_id":"duplicate-registration-name","pack_index":3,"registration_name":"logos"}},"message":"duplicate icon registry name"}""",
        )

        assertEquals(1, error.code)
        assertEquals("MERMAN_INVALID_ARGUMENT", error.codeName)
        assertEquals(MermanErrorKind.GENERIC, error.kind)
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
    fun parsesStructuredDiagnosticFailureDetails() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":{"start":3,"end":8,"kind":"exact"},"field":null,"diagram_type":"flowchart-v2"}},"message":"invalid flowchart"}""",
        )

        assertEquals(
            MermanDiagnosticErrorDetails(
                code = "merman.test",
                span = MermanDiagnosticSpan(start = 3, end = 8, kind = "exact"),
                field = null,
                diagramType = "flowchart-v2",
            ),
            error.diagnosticDetails,
        )
    }

    @Test
    fun parsesStructuredCancellationFailureDetails() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":12,"code_name":"MERMAN_CANCELLED","kind":"generic","capability_id":null,"details":{"cancellation":{"reason":"deadline_exceeded","phase":"admission"}},"message":"operation cancelled"}""",
        )

        assertEquals(
            MermanCancelledDetails(reason = "deadline_exceeded", phase = "admission"),
            error.cancellationDetails,
        )
        assertNull(error.resourceDetails)
    }

    @Test
    fun rejectsMalformedCancellationFailureDetails() {
        val error = MermanException(
            """{"details":{"cancellation":{"reason":"","phase":"admission"}}}""",
        )

        assertNull(error.cancellationDetails)
    }

    @Test
    fun iconPackSetFactoryHasNoNativeLifecycle() {
        val iconPackSet = MermanIconPackSet.fromPacks(
            listOf(MermanIconPack("""{"prefix":"test","icons":{}}""")),
        )

        assertTrue(!AutoCloseable::class.java.isAssignableFrom(iconPackSet.javaClass))
        assertTrue("close" !in iconPackSet.javaClass.declaredMethods.map { it.name })
    }

    @Test
    fun rejectsTooManyIconPacksBeforeCopyingOrLoadingNativeCode() {
        val error = runCatching {
            MermanIconPackSet.fromPacks(
                List(17) { index ->
                    MermanIconPack("""{"prefix":"p$index","icons":{}}""")
                },
            )
        }.exceptionOrNull()

        assertTrue(error is MermanException)
        error as MermanException
        assertEquals(10, error.code)
        assertEquals("MERMAN_RESOURCE_LIMIT_EXCEEDED", error.codeName)
        assertEquals(MermanErrorKind.GENERIC, error.kind)
        assertEquals("icon pack count exceeds the fixed registry ceiling", error.message)
        assertEquals("ceiling", error.resourceDetails?.cause)
        assertEquals("max_icon_registry_packs", error.resourceDetails?.limitId)
        assertEquals("icon_registry_input", error.resourceDetails?.phase)
        assertEquals(17L, error.resourceDetails?.actual)
        assertEquals(16L, error.resourceDetails?.max)
        assertEquals("constructor-fixed", error.resourceDetails?.profile)
        assertEquals("17", error.exactResourceDetails?.actual)
        assertEquals("16", error.exactResourceDetails?.max)
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
