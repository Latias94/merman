package io.merman

/** One immutable UTF-8 IconifyJSON pack plus an optional registration-name override. */
class MermanIconPack(
    val json: String,
    val registrationName: String? = null,
) {
    init {
        require(json.isNotEmpty()) { "IconifyJSON must not be empty" }
        require(registrationName == null || registrationName.isNotEmpty()) {
            "Use null when no registration-name override is required"
        }
    }
}
