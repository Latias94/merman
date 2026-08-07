package io.merman

/** Pure helpers invoked from JNI before any Java string allocation or conversion. */
internal object MermanJniStrings {
    /** Returns the exact UTF-8 length, or `-1` for an unpaired UTF-16 surrogate. */
    @JvmStatic
    fun utf8Length(value: String): Long {
        var bytes = 0L
        var index = 0
        while (index < value.length) {
            val codeUnit = value[index]
            bytes += when {
                codeUnit.code <= 0x7f -> 1L
                codeUnit.code <= 0x7ff -> 2L
                Character.isHighSurrogate(codeUnit) -> {
                    if (
                        index + 1 >= value.length ||
                        !Character.isLowSurrogate(value[index + 1])
                    ) {
                        return -1L
                    }
                    index += 1
                    4L
                }
                Character.isLowSurrogate(codeUnit) -> return -1L
                else -> 3L
            }
            index += 1
        }
        return bytes
    }
}
