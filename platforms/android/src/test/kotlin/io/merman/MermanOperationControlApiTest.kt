package io.merman

import java.lang.reflect.Modifier
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class MermanOperationControlApiTest {
    @Test
    fun operationControlIsPubliclyCloseableAndThreadShareable() {
        val controlClass = MermanOperationControl::class.java
        val publicMethods = controlClass.methods.map { it.name }.toSet()
        val publicConstructors = controlClass.constructors.map { it.parameterTypes.toList() }

        assertTrue(AutoCloseable::class.java.isAssignableFrom(controlClass))
        assertTrue(emptyList<Class<*>>() in publicConstructors)
        assertTrue(listOf(Long::class.javaObjectType) in publicConstructors)
        assertTrue(publicMethods.containsAll(setOf("cancel", "isCancelled", "release", "close")))
        assertEquals(2, Merman.TRANSPORT_API_VERSION)
    }

    @Test
    fun statelessExecuteKeepsTheLegacySignatureAndAddsControlledDispatch() {
        val signatures = Merman::class.java.declaredMethods
            .filter { it.name == "execute" && Modifier.isPublic(it.modifiers) }
            .map { it.parameterTypes.toList() }
            .toSet()

        assertTrue(
            listOf(
                String::class.java,
                String::class.java,
                String::class.java,
                String::class.java,
            ) in signatures,
        )
        assertTrue(
            listOf(
                String::class.java,
                String::class.java,
                MermanOperationControl::class.java,
                String::class.java,
                String::class.java,
            ) in signatures,
        )
    }

    @Test
    fun reusableExecuteKeepsTheLegacySignatureAndAddsControlledDispatch() {
        val signatures = MermanEngine::class.java.declaredMethods
            .filter { it.name == "execute" && Modifier.isPublic(it.modifiers) }
            .map { it.parameterTypes.toList() }
            .toSet()

        assertTrue(
            listOf(
                String::class.java,
                String::class.java,
                String::class.java,
                String::class.java,
            ) in signatures,
        )
        assertTrue(
            listOf(
                String::class.java,
                String::class.java,
                MermanOperationControl::class.java,
                String::class.java,
                String::class.java,
            ) in signatures,
        )
    }
}
