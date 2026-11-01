package org.example.merman.consumer

import androidx.test.ext.junit.runners.AndroidJUnit4
import io.merman.examples.runMermanSmoke
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class InstalledMavenSmokeTest {
    @Test
    fun rendersWithTheResolvedMavenPublication() {
        runMermanSmoke()
    }
}
