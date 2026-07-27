package io.merman

enum class MermanErrorKind(val wireName: String) {
    GENERIC("generic"),
    UNKNOWN_OPERATION("unknown-operation"),
    MISSING_CAPABILITY("missing-capability"),
    BUSY("busy"),
    REENTRANT_CALL("reentrant-call"),
    ;

    internal companion object {
        fun fromWireName(value: String?): MermanErrorKind =
            entries.firstOrNull { it.wireName == value } ?: GENERIC
    }
}
