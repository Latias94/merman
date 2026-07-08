package io.merman

import androidx.test.ext.junit.runners.AndroidJUnit4
import io.merman.examples.runMermanSmoke
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MermanInstrumentedSmokeTest {
    @Test
    fun runsPublicSmokeIncludingThrowingTextMeasurerFallback() {
        runMermanSmoke()
    }
}
