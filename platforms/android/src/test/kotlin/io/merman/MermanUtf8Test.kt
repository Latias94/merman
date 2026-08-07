package io.merman

import org.junit.Assert.assertEquals
import org.junit.Test

class MermanUtf8Test {
    @Test
    fun measuresUtf8BytesWithoutAllocating() {
        assertEquals(0L, utf8Length(""))
        assertEquals(1L, utf8Length("\u0000"))
        assertEquals(1L, utf8Length("a"))
        assertEquals(2L, utf8Length("é"))
        assertEquals(3L, utf8Length("你"))
        assertEquals(4L, utf8Length("😀"))
    }

    @Test
    fun rejectsUnpairedUtf16Surrogates() {
        assertEquals(-1L, utf8Length("\uD800"))
        assertEquals(-1L, utf8Length("\uDC00"))
        assertEquals(-1L, utf8Length("\uD800x"))
    }

    private fun utf8Length(value: String): Long = MermanJniStrings.utf8Length(value)
}
