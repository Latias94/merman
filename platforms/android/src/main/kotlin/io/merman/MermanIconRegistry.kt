package io.merman

/**
 * Immutable reusable snapshot of IconifyJSON packs.
 *
 * [fromPacks] retains only immutable Kotlin values. Each [MermanEngine] constructor borrows fresh
 * arrays for one transactional native build and returns only after that engine owns the parsed
 * registry. This wrapper therefore has no native handle or lifecycle API.
 */
class MermanIconRegistry private constructor(
    private val packs: List<MermanIconPack>,
) {
    /** Borrows fresh arrays for exactly one synchronous engine-construction call. */
    internal inline fun <T> withBorrowedPacks(
        call: (Array<String>, Array<String?>) -> T,
    ): T = call(
        packs.map(MermanIconPack::json).toTypedArray(),
        packs.map(MermanIconPack::registrationName).toTypedArray(),
    )

    companion object {
        private val packLimit = MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS
            .getValue("icon-registry")
            .resourceLimits
            .single { it.id == "max_icon_registry_packs" }

        /** Snapshots the immutable pack values for reuse across engine constructors. */
        @JvmStatic
        fun fromPacks(packs: List<MermanIconPack>): MermanIconRegistry {
            if (packs.size.toLong() > packLimit.value) {
                throw MermanException.iconRegistryPackCountLimit(
                    limit = packLimit,
                    actual = packs.size.toLong(),
                )
            }
            return MermanIconRegistry(packs.toList())
        }
    }
}
