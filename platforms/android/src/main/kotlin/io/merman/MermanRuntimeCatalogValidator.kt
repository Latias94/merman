package io.merman

import org.json.JSONArray
import org.json.JSONObject

internal const val ANDROID_TRANSPORT_API_VERSION: Int = 2

/** Validates the generated Android artifact contract while preserving additive unknown IDs. */
internal object MermanRuntimeCatalogValidator {
    private const val TRANSPORT_ID: String = "android-jni"

    fun validate(json: String): String {
        val catalog = try {
            JSONObject(json)
        } catch (error: Exception) {
            throw MermanException("Invalid Merman runtime catalog: ${error.message}")
        }

        if (requiredInt(catalog, "schema_version") != MERMAN_RUNTIME_CATALOG_SCHEMA_VERSION) {
            throw MermanException("Unsupported Merman runtime contract schema")
        }
        if (requiredInt(catalog, "transport_api_version") != ANDROID_TRANSPORT_API_VERSION) {
            throw MermanException("Merman Android transport API version mismatch")
        }
        val packageVersion = catalog.opt("package_version")
        if (packageVersion !is String || packageVersion.isEmpty()) {
            throw MermanException("Merman runtime catalog is missing package_version")
        }
        if (MERMAN_BINDING_OPTIONS_SCHEMA_VERSION != MERMAN_BINDING_CONTRACT_OPTIONS_SCHEMA_VERSION) {
            throw MermanException("Merman generated options contracts disagree")
        }
        requireOptionsSchema(catalog.opt("options_schema_versions"))

        val transport = MERMAN_BINDING_TRANSPORT_EXPOSURE_SPECS[TRANSPORT_ID]
            ?: throw MermanException("Merman generated Android transport contract is missing")
        requirePayloadSchemas(catalog.opt("payload_schemas"), transport.payloadSchemaIds)

        val capabilities = catalog.opt("capabilities") as? JSONObject
            ?: throw MermanException("Merman runtime capabilities are missing")
        val capabilityIds = requiredSortedStringList(
            capabilities.opt("capability_ids"),
            "capabilities.capability_ids",
        )
        validateCapabilityRelations(catalog, capabilities, capabilityIds)
        val textMeasurementProviderIds = validateTextMeasurementProtocol(
            capabilities.opt("text_measurement"),
        )
        validateOptionalOptionGroups(catalog.opt("option_group_ids"), capabilityIds.toSet())
        validateOptionalConstructorServices(
            catalog = catalog,
            capabilityIds = capabilityIds.toSet(),
            candidateIds = transport.constructorServiceCandidateIds,
            availableProviderIds = textMeasurementProviderIds,
        )

        return json
    }

    private fun validateCapabilityRelations(
        catalog: JSONObject,
        capabilities: JSONObject,
        capabilityIds: List<String>,
    ) {
        val outputIds = requiredSortedStringList(
            capabilities.opt("output_ids"),
            "capabilities.output_ids",
        )
        val operationIds = requiredSortedStringList(
            capabilities.opt("operation_ids"),
            "capabilities.operation_ids",
        )
        val systemAdapterIds = requiredSortedStringList(
            capabilities.opt("system_adapter_ids"),
            "capabilities.system_adapter_ids",
        )
        val capabilitySet = capabilityIds.toSet()
        if (systemAdapterIds.any { it !in capabilitySet }) {
            throw MermanException("Merman runtime system adapters must be advertised capabilities")
        }
        validateCapabilityImplications(capabilityIds, capabilitySet)

        val knownOperations = MERMAN_BINDING_OPERATION_EXPECTATIONS.associateBy {
            it.operationId
        }
        val operationSet = operationIds.toSet()
        val outputSet = outputIds.toSet()
        for (operationId in operationIds) {
            val expectation = knownOperations[operationId] ?: continue
            expectation.availabilityCapabilityId?.let { capabilityId ->
                if (capabilityId !in capabilitySet) {
                    throw MermanException(
                        "Merman runtime operation `$operationId` is missing capability `$capabilityId`",
                    )
                }
            }
            expectation.outputId?.let { outputId ->
                if (outputId !in outputSet) {
                    throw MermanException(
                        "Merman runtime operation `$operationId` is missing output `$outputId`",
                    )
                }
            }
        }
        val metadataIds = validateMetadataIds(catalog.opt("metadata_ids"), capabilitySet)
        validateKnownArtifactIdentity(
            capabilityIds = capabilityIds,
            outputIds = outputIds,
            systemAdapterIds = systemAdapterIds,
            operationIds = operationIds,
            metadataIds = metadataIds,
        )
        validateOutputContracts(catalog.opt("output_contracts"), outputIds)
        validateRegistry(catalog.opt("registry"))
        validateResources(catalog.opt("resources"), operationSet)
    }

    private fun validateCapabilityImplications(
        capabilityIds: List<String>,
        capabilitySet: Set<String>,
    ) {
        for (id in capabilityIds) {
            val spec = MERMAN_BINDING_CAPABILITY_SPECS[id] ?: continue
            val missing = spec.implicationIds.firstOrNull { it !in capabilitySet }
            if (missing != null) {
                throw MermanException(
                    "Merman runtime capability `$id` is missing implied capability `$missing`",
                )
            }
        }
    }

    private fun validateMetadataIds(value: Any?, capabilityIds: Set<String>): List<String> {
        val ids = requiredSortedStringList(value, "metadata_ids")
        for (id in ids) {
            val requiredCapability = MERMAN_BINDING_METADATA_SPECS[id]?.requiredCapabilityId
            if (requiredCapability != null && requiredCapability !in capabilityIds) {
                throw MermanException(
                    "Merman runtime metadata `$id` requires capability `$requiredCapability`",
                )
            }
        }
        return ids
    }

    private fun validateKnownArtifactIdentity(
        capabilityIds: List<String>,
        outputIds: List<String>,
        systemAdapterIds: List<String>,
        operationIds: List<String>,
        metadataIds: List<String>,
    ) {
        val expected = MERMAN_ANDROID_ARTIFACT_EXPECTATION
        validateKnownIds(
            capabilityIds,
            expected.capabilityIds,
            MERMAN_BINDING_CAPABILITY_SPECS.keys,
            "capability IDs",
        )
        validateKnownIds(
            outputIds,
            expected.outputIds,
            MERMAN_ANDROID_OUTPUT_CONTRACT_JSON_BY_ID.keys,
            "output IDs",
        )
        validateKnownIds(
            systemAdapterIds,
            expected.systemAdapterIds,
            MERMAN_BINDING_CAPABILITY_SPECS.keys,
            "system adapter IDs",
        )
        validateKnownIds(
            operationIds,
            expected.operationIds,
            MERMAN_BINDING_OPERATION_EXPECTATIONS.mapTo(hashSetOf()) { it.operationId },
            "operation IDs",
        )
        validateKnownIds(
            metadataIds,
            expected.metadataIds,
            MERMAN_BINDING_METADATA_SPECS.keys,
            "metadata IDs",
        )
    }

    private fun validateKnownIds(
        actual: List<String>,
        expected: List<String>,
        known: Set<String>,
        label: String,
    ) {
        if (actual.filter { it in known } != expected) {
            throw MermanException(
                "Merman runtime known $label do not match the generated Android artifact contract",
            )
        }
    }

    private fun validateOutputContracts(value: Any?, outputIds: List<String>) {
        val contracts = value as? JSONArray
            ?: throw MermanException("Merman runtime output contracts are missing")
        val ids = mutableListOf<String>()
        for (index in 0 until contracts.length()) {
            val contract = contracts.opt(index) as? JSONObject
                ?: throw MermanException("Merman runtime output contract entries must be objects")
            val id = requiredRuntimeIdentifier(contract, "id", "output_contracts[$index]")
            if (ids.isNotEmpty() && ids.last() >= id) {
                throw MermanException("Merman runtime output contracts must be sorted and unique by ID")
            }
            requiredString(contract, "media_type", "output_contracts[$index]")
            if (!contract.has("system_fonts") || !contract.has("embedded_images")) {
                throw MermanException("Merman runtime output contract `$id` is incomplete")
            }
            validateSystemFonts(contract.opt("system_fonts"))
            validateEmbeddedImages(contract.opt("embedded_images"))
            MERMAN_ANDROID_OUTPUT_CONTRACT_JSON_BY_ID[id]?.let { expectedJson ->
                if (!containsExpectedJson(contract, JSONObject(expectedJson))) {
                    throw MermanException(
                        "Merman runtime output contract `$id` disagrees with the generated " +
                            "Android artifact contract",
                    )
                }
            }
            ids += id
        }
        if (ids != outputIds) {
            throw MermanException("Merman runtime output contracts do not match output_ids")
        }
    }

    private fun validateSystemFonts(value: Any?) {
        if (value == null || value === JSONObject.NULL) return
        val fonts = value as? JSONObject
            ?: throw MermanException("Merman runtime system-font output contract is malformed")
        requiredString(fonts, "source_id", "system_fonts")
        requiredString(fonts, "discovery", "system_fonts")
        requiredString(fonts, "cache_scope", "system_fonts")
        if (
            fonts.opt("host_dependent") !is Boolean ||
            fonts.opt("caller_configurable") !is Boolean ||
            fonts.opt("resource_bounded") !is Boolean
        ) {
            throw MermanException("Merman runtime system-font output contract is malformed")
        }
    }

    private fun validateEmbeddedImages(value: Any?) {
        if (value == null || value === JSONObject.NULL) return
        val images = value as? JSONObject
            ?: throw MermanException("Merman runtime embedded-image output contract is malformed")
        if (
            images.opt("filesystem_access") !is Boolean ||
            images.opt("network_access") !is Boolean ||
            images.opt("caller_configurable") !is Boolean
        ) {
            throw MermanException("Merman runtime embedded-image output contract is malformed")
        }
        requiredSortedStringList(images.opt("source_ids"), "embedded_images.source_ids")
        val limits = images.opt("limits") as? JSONObject
            ?: throw MermanException("Merman runtime embedded-image limits are malformed")
        for (field in listOf(
            "max_bytes_per_image",
            "max_total_bytes",
            "max_pixels_per_image",
            "max_total_pixels",
        )) {
            when (val raw = limits.opt(field)) {
                JSONObject.NULL -> Unit
                is Int, is Long -> if ((raw as Number).toLong() <= 0L) {
                    throw MermanException("Merman runtime embedded-image limit `$field` must be positive")
                }
                else -> throw MermanException("Merman runtime embedded-image limit `$field` is malformed")
            }
        }
    }

    private fun containsExpectedJson(actual: Any?, expected: Any?): Boolean {
        if (expected == null || expected === JSONObject.NULL) {
            return actual == null || actual === JSONObject.NULL
        }
        return when (expected) {
            is JSONObject -> {
                val actualObject = actual as? JSONObject ?: return false
                expected.keys().asSequence().all { key ->
                    actualObject.has(key) &&
                        containsExpectedJson(actualObject.opt(key), expected.opt(key))
                }
            }
            is JSONArray -> {
                val actualArray = actual as? JSONArray ?: return false
                actualArray.length() == expected.length() &&
                    (0 until expected.length()).all { index ->
                        containsExpectedJson(actualArray.opt(index), expected.opt(index))
                    }
            }
            is Number -> actual is Number && actual.toString() == expected.toString()
            else -> actual == expected
        }
    }

    private fun validateRegistry(value: Any?) {
        val registry = value as? JSONObject
            ?: throw MermanException("Merman runtime registry metadata is missing")
        if (requiredLong(registry, "diagram_family_count") < 0L) {
            throw MermanException("Merman runtime diagram_family_count must be non-negative")
        }
    }

    private fun validateResources(value: Any?, operationIds: Set<String>) {
        val resources = value as? JSONObject
            ?: throw MermanException("Merman runtime resource metadata is missing")
        val generalProfile = requiredRuntimeIdentifier(
            resources,
            "general_binding_default_profile",
            "resources",
        )
        val cliProfile = requiredRuntimeIdentifier(resources, "cli_default_profile", "resources")
        val limits = resources.opt("limits") as? JSONArray
            ?: throw MermanException("Merman runtime resource limits are missing")
        if (limits.length() == 0) {
            throw MermanException("Merman runtime resource limits must not be empty")
        }
        val limitIds = linkedSetOf<String>()
        val limitMinimums = mutableMapOf<String, Long>()
        val hardCapIds = mutableSetOf<String>()
        for (index in 0 until limits.length()) {
            val limit = limits.opt(index) as? JSONObject
                ?: throw MermanException("Merman runtime resource limit entries must be objects")
            val id = requiredFieldIdentifier(limit, "id", "resources.limits[$index]")
            if (!limitIds.add(id)) {
                throw MermanException("Merman runtime resource limits must be unique by ID")
            }
            requiredString(limit, "phase", "resources.limits[$index]")
            requiredString(limit, "description", "resources.limits[$index]")
            if (limit.opt("overridable") !is Boolean || limit.opt("hard_cap") !is Boolean) {
                throw MermanException("Merman runtime resource limit flags are malformed")
            }
            if (limit.opt("hard_cap") == true && limit.opt("overridable") == true) {
                throw MermanException("Merman runtime hard resource limits cannot be overridable")
            }
            val minimumValue = requiredLong(limit, "minimum_value")
            if (minimumValue < 0L) {
                throw MermanException("Merman runtime resource limit minimum_value must be non-negative")
            }
            limitMinimums[id] = minimumValue
            if (limit.opt("hard_cap") == true) hardCapIds += id
            val limitOperations = requiredSortedStringList(
                limit.opt("operation_ids"),
                "resources.limits.$id.operation_ids",
            )
            if (limitOperations.any { it !in operationIds }) {
                throw MermanException("Merman runtime resource limit `$id` names an unavailable operation")
            }
        }
        val profiles = resources.opt("profiles") as? JSONArray
            ?: throw MermanException("Merman runtime resource profiles are missing")
        if (profiles.length() == 0) {
            throw MermanException("Merman runtime resource profiles must not be empty")
        }
        val profileIds = linkedSetOf<String>()
        var recommendedProfileId: String? = null
        for (index in 0 until profiles.length()) {
            val profile = profiles.opt(index) as? JSONObject
                ?: throw MermanException("Merman runtime resource profile entries must be objects")
            val id = requiredRuntimeIdentifier(profile, "id", "resources.profiles[$index]")
            if (!profileIds.add(id)) {
                throw MermanException("Merman runtime resource profiles must be unique by ID")
            }
            requiredString(profile, "purpose", "resources.profiles[$index]")
            requiredString(profile, "trust_assumption", "resources.profiles[$index]")
            val recommended = profile.opt("recommended_binding_default") as? Boolean
                ?: throw MermanException("Merman runtime resource profile is malformed")
            val profileLimits = profile.opt("limits") as? JSONObject
                ?: throw MermanException("Merman runtime resource profile is malformed")
            val profileLimitIds = profileLimits.keys().asSequence().toSet()
            if (profileLimitIds != limitIds) {
                throw MermanException("Merman runtime resource profile does not cover declared limits")
            }
            for (limitId in profileLimitIds) {
                when (val raw = profileLimits.opt(limitId)) {
                    JSONObject.NULL -> if (limitId in hardCapIds) {
                        throw MermanException("Merman runtime hard resource limit cannot be null")
                    }
                    is Int, is Long -> {
                        val value = (raw as Number).toLong()
                        if (value < limitMinimums.getValue(limitId)) {
                            throw MermanException("Merman runtime resource profile limit is below its minimum")
                        }
                    }
                    else -> throw MermanException("Merman runtime resource profile limit is malformed")
                }
            }
            if (recommended) {
                if (recommendedProfileId != null) {
                    throw MermanException("Merman runtime must recommend exactly one resource profile")
                }
                recommendedProfileId = id
            }
        }
        if (generalProfile !in profileIds || cliProfile !in profileIds) {
            throw MermanException("Merman runtime default resource profile is not advertised")
        }
        if (recommendedProfileId != generalProfile) {
            throw MermanException("Merman runtime must recommend the general binding default profile")
        }
    }

    private fun requireOptionsSchema(value: Any?) {
        val versions = value as? JSONArray
            ?: throw MermanException("Merman runtime options schema versions are missing")
        var supported = false
        for (index in 0 until versions.length()) {
            val version = requiredArrayInt(versions, index, "options_schema_versions")
            if (version == MERMAN_BINDING_OPTIONS_SCHEMA_VERSION) {
                supported = true
            }
        }
        if (!supported) {
            throw MermanException(
                "Merman runtime catalog does not advertise options schema " +
                    MERMAN_BINDING_OPTIONS_SCHEMA_VERSION,
            )
        }
    }

    private fun requirePayloadSchemas(value: Any?, requiredIds: Set<String>) {
        val schemas = value as? JSONArray
            ?: throw MermanException("Merman runtime payload schemas are missing")
        val versions = linkedMapOf<String, Int>()
        for (index in 0 until schemas.length()) {
            val schema = schemas.opt(index) as? JSONObject
                ?: throw MermanException("Merman runtime payload schema entries must be objects")
            val id = requiredRuntimeIdentifier(schema, "id", "payload_schemas[$index]")
            val version = requiredInt(schema, "version")
            if (versions.put(id, version) != null) {
                throw MermanException("Merman runtime payload schema `$id` is duplicated")
            }
            val knownVersion = MERMAN_REQUIRED_PAYLOAD_SCHEMA_VERSIONS[id]
            if (knownVersion != null && version != knownVersion) {
                throw MermanException("Merman `$id` payload schema version mismatch")
            }
        }
        for (id in requiredIds) {
            val expected = MERMAN_REQUIRED_PAYLOAD_SCHEMA_VERSIONS[id]
                ?: throw MermanException("Merman generated payload schema `$id` is missing")
            if (versions[id] != expected) {
                throw MermanException(
                    "Merman runtime catalog does not advertise `$id` schema $expected",
                )
            }
        }
    }

    private fun validateTextMeasurementProtocol(value: Any?): Set<String> {
        return when (value) {
            null, JSONObject.NULL -> emptySet()
            is JSONObject -> {
                val protocolVersion = requiredInt(value, "protocol_version")
                if (
                    protocolVersion != MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION ||
                    protocolVersion != MermanTextMeasurementOperation.PROTOCOL_VERSION
                ) {
                    throw MermanException("Merman text-measurement protocol version mismatch")
                }
                requiredSortedStringList(
                    value.opt("provider_ids"),
                    "capabilities.text_measurement.provider_ids",
                ).toSet()
            }
            else -> throw MermanException("Merman runtime text-measurement contract is malformed")
        }
    }

    private fun validateOptionalOptionGroups(value: Any?, capabilityIds: Set<String>) {
        if (value == null || value === JSONObject.NULL) return
        val actual = requiredSortedFieldIdentifierList(value, "option_group_ids")
        val hasSvgPipeline = "svg" in capabilityIds
        val expected = MERMAN_BINDING_OPTION_GROUP_SPECS.values
            .filter { spec ->
                spec.alwaysAvailable ||
                    (spec.requiresSvgPipeline && hasSvgPipeline) ||
                    spec.anyCapabilityIds.any(capabilityIds::contains)
            }
            .map(MermanBindingOptionGroupSpec::id)
            .sorted()
        val actualKnown = actual.filter {
            MERMAN_BINDING_OPTION_GROUP_SPECS.containsKey(it)
        }
        if (actualKnown != expected) {
            throw MermanException(
                "Merman runtime known option_group_ids do not match the generated artifact contract",
            )
        }
    }

    private fun validateOptionalConstructorServices(
        catalog: JSONObject,
        capabilityIds: Set<String>,
        candidateIds: Set<String>,
        availableProviderIds: Set<String>,
    ) {
        val rawIds = catalog.opt("constructor_service_ids")
        val rawContracts = catalog.opt("constructor_service_contracts")
        if (
            (rawIds == null || rawIds === JSONObject.NULL) &&
            (rawContracts == null || rawContracts === JSONObject.NULL)
        ) {
            return
        }
        if (rawIds == null || rawIds === JSONObject.NULL) {
            throw MermanException(
                "Merman runtime constructor_service_contracts require constructor_service_ids",
            )
        }
        val actualIds = requiredSortedStringList(rawIds, "constructor_service_ids")
        val hasSvgPipeline = "svg" in capabilityIds
        val expectedIds = candidateIds.filterTo(linkedSetOf()) { id ->
            val spec = MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS[id]
                ?: throw MermanException("Merman generated constructor service `$id` is missing")
            !spec.requiresSvgPipeline || hasSvgPipeline
        }
        val expectedKnownIds = expectedIds.filter {
            MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS.containsKey(it)
        }.sorted()
        val actualKnownIds = actualIds.filter {
            MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS.containsKey(it)
        }
        if (actualKnownIds != expectedKnownIds) {
            throw MermanException(
                "Merman runtime known constructor_service_ids do not match the generated Android contract",
            )
        }
        if (rawContracts == null || rawContracts === JSONObject.NULL) {
            throw MermanException("Merman runtime constructor service contracts are missing")
        }
        validateConstructorServiceContracts(rawContracts, actualIds.toSet(), availableProviderIds)
    }

    private fun validateConstructorServiceContracts(
        value: Any?,
        expectedIds: Set<String>,
        availableProviderIds: Set<String>,
    ) {
        val contracts = value as? JSONArray
            ?: throw MermanException("Merman runtime constructor service contracts are malformed")
        val actualIds = mutableListOf<String>()
        var previousId: String? = null
        val generatedProviderOwners = buildMap<String, String> {
            MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS.values.forEach { spec ->
                spec.providedTextMeasurementProviderIds.forEach { providerId ->
                    put(providerId, spec.id)
                }
            }
        }
        val actualProviderOwners = mutableMapOf<String, String>()
        val providersByServiceId = mutableMapOf<String, Set<String>>()
        for (index in 0 until contracts.length()) {
            val contract = contracts.opt(index) as? JSONObject
                ?: throw MermanException(
                    "Merman runtime constructor service contract entries must be objects",
                )
            val id = requiredRuntimeIdentifier(
                contract,
                "id",
                "constructor_service_contracts[$index]",
            )
            if (previousId != null && previousId >= id) {
                throw MermanException(
                    "Merman runtime constructor service contracts must be sorted and unique by ID",
                )
            }
            if (id in actualIds) {
                throw MermanException("Merman runtime constructor service `$id` is duplicated")
            }
            val providerIds = requiredSortedStringList(
                contract.opt("provided_text_measurement_provider_ids"),
                "constructor_service_contracts[$index].provided_text_measurement_provider_ids",
            )
            providersByServiceId[id] = providerIds.toSet()
            val spec = MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS[id]
            val actualKnownProviderIds = providerIds.filter {
                generatedProviderOwners.containsKey(it)
            }.toSet()
            if (spec != null && actualKnownProviderIds != spec.providedTextMeasurementProviderIds.toSet()) {
                throw MermanException(
                    "Merman runtime constructor service `$id` known provider contract mismatch",
                )
            }
            for (providerId in providerIds) {
                if (providerId !in availableProviderIds) {
                    throw MermanException(
                        "Merman runtime constructor service `$id` names unavailable " +
                            "text-measurement provider `$providerId`",
                    )
                }
                val previousOwner = actualProviderOwners.put(providerId, id)
                if (previousOwner != null) {
                    throw MermanException(
                        "Merman runtime text-measurement provider `$providerId` has multiple " +
                            "constructor service owners",
                    )
                }
                val generatedOwner = generatedProviderOwners[providerId]
                if (generatedOwner != null && generatedOwner != id) {
                    throw MermanException(
                        "Merman runtime text-measurement provider `$providerId` belongs to " +
                            "constructor service `$generatedOwner`, not `$id`",
                    )
                }
            }
            validateConstructorResourceLimits(contract.opt("resource_limits"), id)
            actualIds += id
            previousId = id
        }
        if (actualIds.toSet() != expectedIds) {
            throw MermanException(
                "Merman runtime constructor service contracts do not match their IDs",
            )
        }
        for (providerId in availableProviderIds) {
            val ownerId = generatedProviderOwners[providerId] ?: continue
            if (providerId !in providersByServiceId[ownerId].orEmpty()) {
                throw MermanException(
                    "Merman runtime text-measurement provider `$providerId` is missing " +
                        "its constructor service owner `$ownerId`",
                )
            }
        }
    }

    private fun validateConstructorResourceLimits(value: Any?, serviceId: String) {
        val limits = value as? JSONArray
            ?: throw MermanException(
                "Merman runtime constructor service `$serviceId` resource limits are malformed",
            )
        val actual = mutableListOf<MermanBindingConstructorResourceLimitSpec>()
        var previousId: String? = null
        for (index in 0 until limits.length()) {
            val limit = limits.opt(index) as? JSONObject
                ?: throw MermanException(
                    "Merman runtime constructor resource limit entries must be objects",
                )
            val id = requiredFieldIdentifier(limit, "id", "constructor resource limit")
            if (previousId != null && previousId >= id) {
                throw MermanException(
                    "Merman constructor resource limits must be sorted and unique by ID",
                )
            }
            val value = requiredLong(limit, "value")
            if (value < 0L) {
                throw MermanException("Merman constructor resource limit `$id` must be unsigned")
            }
            actual += MermanBindingConstructorResourceLimitSpec(
                id = id,
                phase = requiredString(limit, "phase", "constructor resource limit `$id`"),
                unit = requiredString(limit, "unit", "constructor resource limit `$id`"),
                description = requiredString(
                    limit,
                    "description",
                    "constructor resource limit `$id`",
                ),
                value = value,
            )
            previousId = id
        }
        val expected = MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS[serviceId]?.resourceLimits
            ?: return
        val expectedIds = expected.mapTo(hashSetOf(), MermanBindingConstructorResourceLimitSpec::id)
        val actualKnown = actual.filter { it.id in expectedIds }
        if (actualKnown != expected) {
            throw MermanException(
                "Merman runtime constructor service `$serviceId` known resource limits do not " +
                    "match the generated contract",
            )
        }
    }

    private fun requiredStringSet(value: Any?, field: String): Set<String> {
        val values = value as? JSONArray
            ?: throw MermanException("Merman runtime catalog field `$field` must be an array")
        val result = linkedSetOf<String>()
        for (index in 0 until values.length()) {
            val item = values.opt(index)
            if (item !is String || !MERMAN_RUNTIME_CATALOG_IDENTIFIER_REGEX.matches(item)) {
                throw MermanException(
                    "Merman runtime catalog field `$field` must contain stable runtime identifiers",
                )
            }
            if (!result.add(item)) {
                throw MermanException("Merman runtime catalog field `$field` contains duplicates")
            }
        }
        return result
    }

    private fun requiredSortedStringList(value: Any?, field: String): List<String> {
        val values = requiredStringSet(value, field).toList()
        if (values.zipWithNext().any { (previous, current) -> previous >= current }) {
            throw MermanException("Merman runtime catalog field `$field` must be sorted and unique")
        }
        return values
    }

    private fun requiredSortedFieldIdentifierList(value: Any?, field: String): List<String> {
        val values = value as? JSONArray
            ?: throw MermanException("Merman runtime catalog field `$field` must be an array")
        val result = mutableListOf<String>()
        for (index in 0 until values.length()) {
            val item = values.opt(index)
            if (item !is String || !MERMAN_RUNTIME_CATALOG_FIELD_IDENTIFIER_REGEX.matches(item)) {
                throw MermanException(
                    "Merman runtime catalog field `$field` must contain stable field identifiers",
                )
            }
            if (result.isNotEmpty() && result.last() >= item) {
                throw MermanException(
                    "Merman runtime catalog field `$field` must be sorted and unique",
                )
            }
            result += item
        }
        return result
    }

    private fun requiredString(objectValue: JSONObject, key: String, owner: String): String {
        val value = objectValue.opt(key)
        if (value !is String || value.isEmpty()) {
            throw MermanException("Merman runtime catalog $owner field `$key` must be a string")
        }
        return value
    }

    private fun requiredRuntimeIdentifier(
        objectValue: JSONObject,
        key: String,
        owner: String,
    ): String {
        val value = requiredString(objectValue, key, owner)
        if (!MERMAN_RUNTIME_CATALOG_IDENTIFIER_REGEX.matches(value)) {
            throw MermanException(
                "Merman runtime catalog $owner field `$key` must be a stable runtime identifier",
            )
        }
        return value
    }

    private fun requiredFieldIdentifier(
        objectValue: JSONObject,
        key: String,
        owner: String,
    ): String {
        val value = requiredString(objectValue, key, owner)
        if (!MERMAN_RUNTIME_CATALOG_FIELD_IDENTIFIER_REGEX.matches(value)) {
            throw MermanException(
                "Merman runtime catalog $owner field `$key` must be a stable field identifier",
            )
        }
        return value
    }

    private fun requiredArrayInt(values: JSONArray, index: Int, field: String): Int {
        val value = when (val raw = values.opt(index)) {
            is Int -> raw.toLong()
            is Long -> raw
            else -> throw MermanException(
                "Merman runtime catalog field `$field` must contain JSON integers",
            )
        }
        if (value !in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
            throw MermanException(
                "Merman runtime catalog field `$field` is outside the supported integer range",
            )
        }
        return value.toInt()
    }

    private fun requiredInt(objectValue: JSONObject, key: String): Int {
        val value = requiredLong(objectValue, key)
        if (value !in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
            throw MermanException(
                "Merman runtime catalog field `$key` is outside the supported integer range",
            )
        }
        return value.toInt()
    }

    private fun requiredLong(objectValue: JSONObject, key: String): Long =
        when (val raw = objectValue.opt(key)) {
            is Int -> raw.toLong()
            is Long -> raw
            else -> throw MermanException(
                "Merman runtime catalog field `$key` must be a JSON integer",
            )
        }
}
