package io.merman

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MermanOperationControlInstrumentedTest {
    @Test
    fun preCancelledOneShotUsesCanonicalMixedErrorPrecedence() {
        assertCanonicalMixedErrorPrecedence { operationId, source, optionsJson, uri, control ->
            Merman.execute(
                operationId = operationId,
                source = source,
                control = control,
                optionsJson = optionsJson,
                uri = uri,
            )
        }
    }

    @Test
    fun preCancelledReusableExecutionUsesCanonicalMixedErrorPrecedence() {
        MermanEngine().use { engine ->
            assertCanonicalMixedErrorPrecedence { operationId, source, optionsJson, uri, control ->
                engine.execute(
                    operationId = operationId,
                    source = source,
                    control = control,
                    optionsJson = optionsJson,
                    uri = uri,
                )
            }
        }
    }

    @Test
    fun releaseRemainsIdempotent() {
        val control = MermanOperationControl()
        control.close()
        control.close()
        control.release()

        val released = runCatching { control.cancel() }.exceptionOrNull()
        assertTrue(released is MermanException)
        assertEquals(1, (released as MermanException).code)
        assertEquals("MERMAN_INVALID_ARGUMENT", released.codeName)
    }

    @Test
    fun zeroTimeoutValidRequestIsCancelledAtAdmission() {
        MermanOperationControl(timeoutMs = 0).use { control ->
            val cancelled = runCatching {
                Merman.execute(
                    operationId = "semantic-json",
                    source = VALID_SOURCE,
                    control = control,
                    optionsJson = MALFORMED_OPTIONS_JSON,
                )
            }.exceptionOrNull()

            assertTrue(cancelled is MermanException)
            cancelled as MermanException
            assertEquals(12, cancelled.code)
            assertEquals("MERMAN_CANCELLED", cancelled.codeName)
            assertEquals("deadline_exceeded", cancelled.cancellationDetails?.reason)
            assertEquals("admission", cancelled.cancellationDetails?.phase)
            assertNull(cancelled.resourceDetails)
        }
    }

    @Test
    fun negativeTimeoutIsATypedInvalidArgument() {
        val failure = runCatching {
            MermanOperationControl(timeoutMs = -1)
        }.exceptionOrNull()

        assertTrue(failure is MermanException)
        assertEquals(1, (failure as MermanException).code)
        assertEquals("MERMAN_INVALID_ARGUMENT", failure.codeName)
    }

    private fun assertCanonicalMixedErrorPrecedence(
        execute: (
            operationId: String,
            source: String,
            optionsJson: String?,
            uri: String?,
            control: MermanOperationControl,
        ) -> Unit,
    ) {
        val cases = listOf(
            MixedErrorCase(
                name = "unknown operation",
                operationId = "unknown-operation",
                source = VALID_SOURCE,
                uri = null,
                expectedCode = 7,
                expectedCodeName = "MERMAN_UNSUPPORTED_OPERATION",
                expectedKind = MermanErrorKind.UNKNOWN_OPERATION,
                expectedMessage = "unknown operation",
                expectCancellation = false,
            ),
            MixedErrorCase(
                name = "required URI missing",
                operationId = "document-analysis-json",
                source = VALID_SOURCE,
                uri = null,
                expectedCode = 1,
                expectedCodeName = "MERMAN_INVALID_ARGUMENT",
                expectedKind = MermanErrorKind.GENERIC,
                expectedMessage = "requires a document URI",
                expectCancellation = false,
            ),
            MixedErrorCase(
                name = "invalid Java string",
                operationId = "semantic-json",
                source = INVALID_UTF16_SOURCE,
                uri = null,
                expectedCode = null,
                expectedCodeName = null,
                expectedKind = MermanErrorKind.GENERIC,
                expectedMessage = "not valid Unicode",
                expectCancellation = false,
            ),
            MixedErrorCase(
                name = "valid request identity",
                operationId = "semantic-json",
                source = VALID_SOURCE,
                uri = null,
                expectedCode = 12,
                expectedCodeName = "MERMAN_CANCELLED",
                expectedKind = MermanErrorKind.GENERIC,
                expectedMessage = "cancelled",
                expectCancellation = true,
            ),
        )

        for (case in cases) {
            MermanOperationControl().use { control ->
                control.cancel()
                assertTrue("${case.name}: control must be cancelled", control.isCancelled())

                val failure = runCatching {
                    execute(
                        case.operationId,
                        case.source,
                        MALFORMED_OPTIONS_JSON,
                        case.uri,
                        control,
                    )
                }.exceptionOrNull()

                assertTrue("${case.name}: unexpected failure $failure", failure is MermanException)
                failure as MermanException
                assertEquals(case.name, case.expectedCode, failure.code)
                assertEquals(case.name, case.expectedCodeName, failure.codeName)
                assertEquals(case.name, case.expectedKind, failure.kind)
                assertTrue(
                    "${case.name}: unexpected message ${failure.message}",
                    failure.message.orEmpty().contains(case.expectedMessage),
                )
                if (case.expectCancellation) {
                    assertEquals(case.name, "requested", failure.cancellationDetails?.reason)
                    assertEquals(case.name, "admission", failure.cancellationDetails?.phase)
                } else {
                    assertNull(case.name, failure.cancellationDetails)
                }
                assertNull(case.name, failure.resourceDetails)
            }
        }
    }

    private data class MixedErrorCase(
        val name: String,
        val operationId: String,
        val source: String,
        val uri: String?,
        val expectedCode: Int?,
        val expectedCodeName: String?,
        val expectedKind: MermanErrorKind,
        val expectedMessage: String,
        val expectCancellation: Boolean,
    )

    private companion object {
        const val VALID_SOURCE = "flowchart TD\nA --> B"
        const val MALFORMED_OPTIONS_JSON = "{"
        // A lone surrogate proves strict JNI request decoding precedes cancellation admission.
        const val INVALID_UTF16_SOURCE = "\uD800"
    }
}
