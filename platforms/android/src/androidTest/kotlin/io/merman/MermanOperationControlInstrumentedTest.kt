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
    fun preCancelledOneShotWinsBeforeJavaStringDecodingAndPreservesReleaseContract() {
        val control = MermanOperationControl()
        val canceller = Thread { control.cancel() }
        canceller.start()
        canceller.join()

        assertTrue(control.isCancelled())
        val cancelled = runCatching {
            Merman.execute(
                operationId = "semantic-json",
                source = INVALID_UTF16_SOURCE,
                control = control,
            )
        }.exceptionOrNull()
        assertTrue(cancelled is MermanException)
        cancelled as MermanException
        assertEquals(12, cancelled.code)
        assertEquals("MERMAN_CANCELLED", cancelled.codeName)
        assertEquals("requested", cancelled.cancellationDetails?.reason)
        assertEquals("admission", cancelled.cancellationDetails?.phase)
        assertNull(cancelled.resourceDetails)

        control.close()
        control.close()
        control.release()

        val released = runCatching { control.cancel() }.exceptionOrNull()
        assertTrue(released is MermanException)
        assertEquals(1, (released as MermanException).code)
        assertEquals("MERMAN_INVALID_ARGUMENT", released.codeName)
    }

    @Test
    fun zeroTimeoutReusableExecutionWinsBeforeJavaStringDecoding() {
        MermanOperationControl(timeoutMs = 0).use { control ->
            MermanEngine().use { engine ->
                val cancelled = runCatching {
                    engine.execute(
                        operationId = "semantic-json",
                        source = INVALID_UTF16_SOURCE,
                        control = control,
                    )
                }.exceptionOrNull()

                assertTrue(cancelled is MermanException)
                cancelled as MermanException
                assertEquals(12, cancelled.code)
                assertEquals("deadline_exceeded", cancelled.cancellationDetails?.reason)
                assertEquals("admission", cancelled.cancellationDetails?.phase)
                assertNull(cancelled.resourceDetails)
            }
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

    private companion object {
        // A lone surrogate proves control preflight wins before strict JNI string decoding.
        const val INVALID_UTF16_SOURCE = "\uD800"
    }
}
