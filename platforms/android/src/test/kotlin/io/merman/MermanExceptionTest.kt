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
            """{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","kind":"generic","capability_id":null,"details":{"resource":{"cause":"ceiling","limit_id":"max_layout_work_units","phase":"layout_model","actual":"9007199254740992","max":"9007199254740991","profile":"interactive"}},"message":"layout work exceeded"}""",
        )

        assertEquals("9007199254740992", error.exactResourceDetails?.actual)
        assertEquals("9007199254740991", error.exactResourceDetails?.max)
        assertEquals(9_007_199_254_740_992L, error.resourceDetails?.actual)
        assertEquals(9_007_199_254_740_991L, error.resourceDetails?.max)
    }

    @Test
    fun rejectsResourceCountsOutsideUnsignedLongRange() {
        assertInvalidNativeErrorPayload(
            """{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","kind":"generic","capability_id":null,"details":{"resource":{"cause":"ceiling","limit_id":"max_source_bytes","phase":"source","actual":"18446744073709551616","max":"4","profile":"interactive"}},"message":"source exceeded"}""",
        )
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
        assertInvalidNativeErrorPayload(
            """{"version":1,"ok":false,"code":1,"code_name":"MERMAN_INVALID_ARGUMENT","kind":"generic","capability_id":null,"details":{"icon_registry":{"kind_id":"","pack_index":-1}},"message":"invalid icon registry"}""",
        )
    }

    @Test
    fun parsesStructuredDiagnosticFailureDetails() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":{"start":3,"end":8,"kind":"exact"},"field":null,"diagram_type":"flowchart-v2","requested_max_width":10,"actual_width":42,"width_profile":"unicode","fallback_reason":"primary_overflow"}},"message":"invalid flowchart"}""",
        )

        assertEquals(
            MermanDiagnosticErrorDetails(
                code = "merman.test",
                span = MermanDiagnosticSpan(start = 3, end = 8, kind = "exact"),
                field = null,
                diagramType = "flowchart-v2",
                requestedMaxWidth = 10,
                actualWidth = 42,
                widthProfile = "unicode",
                fallbackReason = "primary_overflow",
            ),
            error.diagnosticDetails,
        )
    }

    @Test
    fun rejectsMalformedOptionalDiagnosticFields() {
        listOf(
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":null,"field":null,"diagram_type":null,"requested_max_width":-1}},"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":null,"field":null,"diagram_type":null,"actual_width":"42"}},"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":null,"field":null,"diagram_type":null,"width_profile":7}},"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":null,"field":null,"diagram_type":null,"fallback_reason":false}},"message":"invalid flowchart"}""",
        ).forEach(::assertInvalidNativeErrorPayload)
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
    fun parsesCanonicalMissingCapabilityFailure() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":7,"code_name":"MERMAN_UNSUPPORTED_OPERATION","kind":"missing-capability","capability_id":"svg","details":{"diagnostic":{"code":"merman.svg.missing-capability","span":null,"field":null,"diagram_type":null}},"message":"SVG is unavailable"}""",
        )

        assertEquals(7, error.code)
        assertEquals("MERMAN_UNSUPPORTED_OPERATION", error.codeName)
        assertEquals(MermanErrorKind.MISSING_CAPABILITY, error.kind)
        assertEquals("svg", error.capabilityId)
        assertEquals("merman.svg.missing-capability", error.diagnosticDetails?.code)
    }

    @Test
    fun acceptsUnknownAdditiveDetailFieldsInSchemaOne() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"future_context":{"source":"parser"}},"message":"invalid flowchart"}""",
        )

        assertEquals(5, error.code)
        assertEquals("MERMAN_PARSE_ERROR", error.codeName)
        assertEquals(MermanErrorKind.GENERIC, error.kind)
        assertNull(error.diagnosticDetails)
    }

    @Test
    fun rejectsContradictoryCancellationTerminalsAsTransportContractFailures() {
        val conflictingDetails = listOf(
            """"resource":{"cause":"ceiling","limit_id":"max_source_bytes","phase":"source","actual":5,"max":4,"profile":"interactive"}""",
            """"diagnostic":{"code":"merman.test","span":null,"field":null,"diagram_type":null}""",
        )

        conflictingDetails.forEach { conflictingDetail ->
            assertInvalidNativeErrorPayload(
                """{"version":1,"ok":false,"code":12,"code_name":"MERMAN_CANCELLED","kind":"generic","capability_id":null,"details":{"cancellation":{"reason":"requested","phase":"layout"},$conflictingDetail},"message":"operation cancelled"}""",
            )
        }
    }

    @Test
    fun rejectsInconsistentNativeErrorIdentityAsTransportContractFailure() {
        listOf(
            """{"version":1,"ok":false,"code":12,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"cancellation":{"reason":"requested","phase":"layout"}},"message":"operation cancelled"}""",
            """{"version":1,"ok":false,"code":7,"code_name":"MERMAN_UNSUPPORTED_OPERATION","kind":"missing-capability","capability_id":null,"message":"missing capability"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":"svg","message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"resource":{"cause":"ceiling","limit_id":"max_source_bytes","phase":"source","actual":5,"max":4,"profile":"interactive"}},"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","kind":"generic","capability_id":null,"message":"source exceeded"}""",
            """{"version":1,"ok":false,"code":11,"code_name":"MERMAN_BUSY","kind":"busy","capability_id":"svg","message":"engine busy"}""",
        ).forEach(::assertInvalidNativeErrorPayload)
    }

    @Test
    fun rejectsInvalidNativeErrorEnvelopeHeaderAsTransportContractFailure() {
        listOf(
            """{"version":2,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"message":"invalid flowchart"}""",
            """{"version":1,"ok":true,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"message":"invalid flowchart"""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"message":"invalid flowchart"} trailing""",
        ).forEach(::assertInvalidNativeErrorPayload)
    }

    @Test
    fun rejectsWrongTypeAndNonZeroInsertionPointDiagnosticSpans() {
        listOf(
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":"bad","field":null,"diagram_type":null}},"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":{"start":3,"end":4,"kind":"insertion-point"},"field":null,"diagram_type":null}},"message":"invalid flowchart"}""",
        ).forEach(::assertInvalidNativeErrorPayload)
    }

    @Test
    fun rejectsInvalidCancellationVocabularyAsTransportContractFailure() {
        listOf(
            """{"version":1,"ok":false,"code":12,"code_name":"MERMAN_CANCELLED","kind":"generic","capability_id":null,"details":{"cancellation":{"reason":"bogus","phase":"layout"}},"message":"operation cancelled"}""",
            """{"version":1,"ok":false,"code":12,"code_name":"MERMAN_CANCELLED","kind":"generic","capability_id":null,"details":{"cancellation":{"reason":"requested","phase":"unknown phase"}},"message":"operation cancelled"}""",
            """{"version":1,"ok":false,"code":12,"code_name":"MERMAN_CANCELLED","kind":"generic","capability_id":null,"details":{"cancellation":{"reason":"requested","phase":"FuturePhase"}},"message":"operation cancelled"}""",
        ).forEach(::assertInvalidNativeErrorPayload)
    }

    @Test
    fun acceptsFutureCancellationPhaseIdentifiers() {
        val error = MermanException(
            """{"version":1,"ok":false,"code":12,"code_name":"MERMAN_CANCELLED","kind":"generic","capability_id":null,"details":{"cancellation":{"reason":"requested","phase":"future-render-stage"}},"message":"operation cancelled"}""",
        )

        assertEquals("future-render-stage", error.cancellationDetails?.phase)
    }

    @Test
    fun rejectsCoercedNestedNativeFieldTypes() {
        listOf(
            """{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","kind":"generic","capability_id":null,"details":{"resource":{"cause":7,"limit_id":"max_source_bytes","phase":"source","actual":5,"max":4,"profile":"interactive"}},"message":"source exceeded"}""",
            """{"version":1,"ok":false,"code":10,"code_name":"MERMAN_RESOURCE_LIMIT_EXCEEDED","kind":"generic","capability_id":null,"details":{"resource":{"cause":"ceiling","limit_id":"max_source_bytes","phase":"source","actual":5.5,"max":4,"profile":"interactive"}},"message":"source exceeded"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":{"start":"3","end":4,"kind":"exact"},"field":null,"diagram_type":null}},"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":{"start":true,"end":4,"kind":"exact"},"field":null,"diagram_type":null}},"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":6,"code_name":"MERMAN_RENDER_ERROR","kind":"generic","capability_id":null,"details":{"icon_registry":{"kind_id":"invalid_xml","pack_index":"2","registration_name":null}},"message":"icon body is invalid"}""",
            """{"version":1,"ok":false,"code":5,"code_name":"MERMAN_PARSE_ERROR","kind":"generic","capability_id":null,"details":{"diagnostic":{"code":"merman.test","span":null,"field":7,"diagram_type":null}},"message":"invalid flowchart"}""",
            """{"version":1,"ok":false,"code":6,"code_name":"MERMAN_RENDER_ERROR","kind":"generic","capability_id":null,"details":{"icon_registry":{"kind_id":"invalid_xml","pack_index":null,"registration_name":7}},"message":"icon body is invalid"}""",
        ).forEach(::assertInvalidNativeErrorPayload)
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

    private fun assertInvalidNativeErrorPayload(payload: String) {
        val error = MermanException(payload)

        assertEquals(9, error.code)
        assertEquals("MERMAN_INTERNAL_ERROR", error.codeName)
        assertEquals(MermanErrorKind.GENERIC, error.kind)
        assertNull(error.capabilityId)
        assertNull(error.exactResourceDetails)
        assertNull(error.resourceDetails)
        assertNull(error.diagnosticDetails)
        assertNull(error.iconRegistryDetails)
        assertNull(error.cancellationDetails)
        assertEquals("Merman Android transport returned an invalid error payload", error.message)
    }
}
