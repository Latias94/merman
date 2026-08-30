package io.merman

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.fail
import org.junit.Test

class MermanRuntimeCatalogValidatorTest {
    @Test
    fun knownResourceLimitValuesAreRuntimeImmutable() {
        @Suppress("UNCHECKED_CAST")
        val values = MermanResourceLimitId.knownValues as MutableList<MermanResourceLimitId>
        val original = values.first()

        try {
            values[0] = MermanResourceLimitId.MAX_MODEL_ITEMS
            fail("known resource limit values accepted mutation")
        } catch (_: UnsupportedOperationException) {
        }

        assertSame(original, MermanResourceLimitId.knownValues.first())
    }

    @Test
    fun runtimeResourceLimitIdsPreserveUnknownFutureValues() {
        val known = MermanResourceLimitId.fromId("max_source_bytes")
        assertSame(MermanResourceLimitId.MAX_SOURCE_BYTES, known)
        assertEquals("source", known.phase)

        val future = MermanResourceLimitId.fromId("future_limit")
        assertEquals("future_limit", future.id)
        assertFalse(future.isKnown)
        assertNull(future.phase)
        assertNull(future.overridable)
        assertNull(future.minimumValue)
        assertEquals(future, MermanResourceLimitId.fromId("future_limit"))
    }

    @Test
    fun acceptsCurrentHandshakeAndPreservesUnknownAdditions() {
        val catalog = validCatalog()
            .put("future_catalog_field", JSONObject().put("version", 1))
        catalog.getJSONArray("options_schema_versions").put(99)
        catalog.getJSONArray("payload_schemas").put(
            JSONObject().put("id", "future-payload").put("version", 7),
        )

        assertEquals(
            catalog.toString(),
            MermanRuntimeCatalogValidator.validate(catalog.toString()),
        )
    }

    @Test
    fun acceptsLegacyCatalogWithoutAdditiveOptionOrServiceSections() {
        val legacy = validCatalog()
        legacy.remove("option_group_ids")
        legacy.remove("constructor_service_ids")
        legacy.remove("constructor_service_contracts")

        assertEquals(
            legacy.toString(),
            MermanRuntimeCatalogValidator.validate(legacy.toString()),
        )
    }

    @Test
    fun preservesUnknownFutureOptionServiceAndConstructorResourceIds() {
        val catalog = validCatalog()
        catalog.getJSONArray("option_group_ids").put("zz_future-option")
        catalog.getJSONObject("capabilities")
            .getJSONObject("text_measurement")
            .getJSONArray("provider_ids")
            .put("zz-future-provider")

        val existingIds = catalog.getJSONArray("constructor_service_ids")
        catalog.put(
            "constructor_service_ids",
            JSONArray().put("future-service").also { ids ->
                for (index in 0 until existingIds.length()) ids.put(existingIds.getString(index))
            },
        )
        val existingContracts = catalog.getJSONArray("constructor_service_contracts")
        catalog.put(
            "constructor_service_contracts",
            JSONArray()
                .put(
                    JSONObject()
                        .put("id", "future-service")
                        .put(
                            "provided_text_measurement_provider_ids",
                            JSONArray().put("zz-future-provider"),
                        )
                        .put(
                            "resource_limits",
                            JSONArray().put(
                                JSONObject()
                                    .put("id", "max_future_items")
                                    .put("phase", "construction")
                                    .put("unit", "items")
                                    .put("description", "Maximum future items")
                                    .put("value", 16),
                            ),
                        ),
                )
                .also { contracts ->
                    for (index in 0 until existingContracts.length()) {
                        contracts.put(existingContracts.getJSONObject(index))
                    }
                },
        )
        catalog.constructorServiceContract("icon-registry")
            .getJSONArray("resource_limits")
            .put(
                JSONObject()
                    .put("id", "zz_future_limit")
                    .put("phase", "icon_registry_future")
                    .put("unit", "items")
                    .put("description", "Future additive constructor limit")
                    .put("value", 1),
            )

        assertEquals(
            catalog.toString(),
            MermanRuntimeCatalogValidator.validate(catalog.toString()),
        )
    }

    @Test
    fun validatesPresentOptionAndConstructorServiceSectionsStrictly() {
        val invalidCatalogs = listOf(
            validCatalog().mutateStringArray("option_group_ids") { remove(0) },
            validCatalog().mutateStringArray("option_group_ids") { put(getString(0)) },
            validCatalog().also {
                it.getJSONArray("option_group_ids").swapFirstTwo()
            }.toString(),
            validCatalog().removeAndReturn("constructor_service_contracts"),
            validCatalog().mutateStringArray("constructor_service_ids") { remove(0) },
            validCatalog().mutateStringArray("constructor_service_ids") { put("future-service") },
            validCatalog().also {
                it.getJSONArray("constructor_service_ids").swapFirstTwo()
            }.toString(),
            validCatalog().also {
                it.getJSONArray("constructor_service_contracts").swapFirstTwo()
            }.toString(),
            validCatalog().also {
                it.getJSONObject("capabilities")
                    .getJSONObject("text_measurement")
                    .getJSONArray("provider_ids")
                    .swapFirstTwo()
            }.toString(),
            validCatalog().also {
                it.constructorServiceContract("host-text-measurement")
                    .getJSONArray("provided_text_measurement_provider_ids")
                    .put("future-provider")
            }.toString(),
            validCatalog().also {
                it.constructorServiceContract("icon-registry")
                    .put("resource_limits", "invalid")
            }.toString(),
            validCatalog().also {
                it.constructorServiceContract("icon-registry")
                    .getJSONArray("resource_limits")
                    .put(
                        JSONObject()
                            .put("id", "test-limit")
                            .put("phase", "construction")
                            .put("unit", "items")
                            .put("description", "test")
                            .put("value", -1),
                    )
            }.toString(),
            validCatalog().also {
                val limit = it.constructorServiceContract("icon-registry")
                    .getJSONArray("resource_limits")
                    .getJSONObject(0)
                limit.put("value", limit.getLong("value") + 1)
            }.toString(),
        )

        invalidCatalogs.forEach(::assertRejected)

        val capabilityFocusedCatalog = validCatalog().also {
            it.getJSONObject("capabilities")
                .put("capability_ids", JSONArray(listOf("analysis")))
        }
        assertRejected(capabilityFocusedCatalog.toString())
    }

    @Test
    fun rejectsCatalogDriftInCallableRelationsAndResources() {
        val invalidCatalogs = listOf(
            validCatalog().also {
                it.getJSONObject("capabilities")
                    .getJSONArray("operation_ids")
                    .remove(0)
            }.toString(),
            validCatalog().also {
                val ids = it.getJSONObject("capabilities").getJSONArray("operation_ids")
                val first = ids.getString(0)
                ids.put(0, ids.getString(1)).put(1, first)
            }.toString(),
            validCatalog().also {
                it.getJSONObject("capabilities")
                    .getJSONArray("output_ids")
                    .put(0, "svg")
            }.toString(),
            validCatalog().also {
                it.getJSONArray("metadata_ids").remove(0)
            }.toString(),
            validCatalog().also {
                it.getJSONArray("output_contracts")
                    .getJSONObject(0)
                    .put("media_type", "image/png")
            }.toString(),
            validCatalog().also {
                it.getJSONObject("resources")
                    .getJSONArray("limits")
                    .getJSONObject(0)
                    .getJSONArray("operation_ids")
                    .put("future-operation")
            }.toString(),
        )

        invalidCatalogs.forEach(::assertRejected)
    }

    @Test
    fun rejectsInvalidRuntimeIdentifierGrammarWhileKeepingFutureIdsOpen() {
        val invalidCatalogs = listOf(
            validCatalog().also {
                it.getJSONObject("capabilities")
                    .put("capability_ids", JSONArray().put("Future-capability"))
            }.toString(),
            validCatalog().put("metadata_ids", JSONArray().put("future metadata")).toString(),
            validCatalog().put("option_group_ids", JSONArray().put("future.option")).toString(),
            validCatalog().put(
                "payload_schemas",
                JSONArray().put(JSONObject().put("id", "future_schema").put("version", 1)),
            ).toString(),
            validCatalog().also {
                it.getJSONObject("resources")
                    .getJSONArray("limits")
                    .getJSONObject(0)
                    .put("id", "Future_limit")
            }.toString(),
            validCatalog().also {
                it.getJSONObject("resources")
                    .getJSONArray("profiles")
                    .getJSONObject(0)
                    .put("id", "future_profile")
            }.toString(),
        )

        invalidCatalogs.forEach(::assertRejected)
    }

    @Test
    fun rejectsMalformedOrIncompatibleHandshakeAndBothMissingPayloadSchemas() {
        val invalidCatalogs = listOf(
            "not-json",
            "[]",
            JSONObject().toString(),
            validCatalog().put("schema_version", "1").toString(),
            validCatalog().toString().replace("\"schema_version\":1", "\"schema_version\":1.5"),
            validCatalog().put("schema_version", 2).toString(),
            validCatalog().put("transport_api_version", "2").toString(),
            validCatalog().toString().replace(
                "\"transport_api_version\":2",
                "\"transport_api_version\":2.5",
            ),
            validCatalog().put("transport_api_version", 3).toString(),
            validCatalog().put("package_version", "").toString(),
            validCatalog().put("package_version", 1).toString(),
            validCatalog().removeAndReturn("options_schema_versions"),
            validCatalog().put("options_schema_versions", JSONArray().put(1)).toString(),
            validCatalog().put("options_schema_versions", JSONArray().put("2")).toString(),
            validCatalog().put("options_schema_versions", JSONArray().put(2.5)).toString(),
            validCatalog().removeAndReturn("payload_schemas"),
            validCatalog().withPayloadSchemas("binding-result"),
            validCatalog().withPayloadSchemas("operation-metadata"),
            validCatalog().withPayloadSchemaVersion("binding-result", 2),
            validCatalog().withPayloadSchemaVersion("operation-metadata", 2),
            validCatalog().also {
                it.getJSONArray("payload_schemas").getJSONObject(0).put("version", "1")
            }.toString(),
            validCatalog().removeAndReturn("capabilities"),
            validCatalog().also {
                it.getJSONObject("capabilities").remove("capability_ids")
            }.toString(),
            validCatalog().withTextMeasurement(JSONObject().put("protocol_version", 2)),
            validCatalog().withTextMeasurement(JSONObject().put("protocol_version", "1")),
            validCatalog().withTextMeasurement(
                JSONObject()
                    .put("protocol_version", MermanTextMeasurementOperation.PROTOCOL_VERSION)
                    .put("provider_ids", "invalid"),
            ),
            validCatalog().withTextMeasurement(
                JSONObject()
                    .put("protocol_version", MermanTextMeasurementOperation.PROTOCOL_VERSION)
                    .put("provider_ids", JSONArray().put("host-callback")),
            ),
            validCatalog().withTextMeasurement("invalid"),
            validCatalog().withTextMeasurement(JSONObject.NULL),
        )

        invalidCatalogs.forEach(::assertRejected)
    }

    private fun assertRejected(catalog: String) {
        try {
            MermanRuntimeCatalogValidator.validate(catalog)
            fail("malformed runtime catalog was accepted: $catalog")
        } catch (_: MermanException) {
        }
    }

    private fun validCatalog(): JSONObject {
        val artifact = MERMAN_ANDROID_ARTIFACT_EXPECTATION
        val capabilityIds = artifact.capabilityIds
        val hasSvgPipeline = "svg" in capabilityIds
        val optionIds = MERMAN_BINDING_OPTION_GROUP_SPECS.values
            .filter { spec ->
                spec.alwaysAvailable ||
                    (spec.requiresSvgPipeline && hasSvgPipeline) ||
                    spec.anyCapabilityIds.any(capabilityIds::contains)
            }
            .map(MermanBindingOptionGroupSpec::id)
            .sorted()
        val serviceIds = MERMAN_BINDING_TRANSPORT_EXPOSURE_SPECS.getValue("android-jni")
            .constructorServiceCandidateIds
            .sorted()
        val providerIds = buildSet {
            add("deterministic")
            serviceIds.forEach { id ->
                addAll(
                    MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS
                        .getValue(id)
                        .providedTextMeasurementProviderIds,
                )
            }
        }.toList().sorted()
        val operationIds = artifact.operationIds
        val outputIds = artifact.outputIds
        val metadataIds = artifact.metadataIds

        return JSONObject()
            .put("schema_version", 1)
            .put("transport_api_version", ANDROID_TRANSPORT_API_VERSION)
            .put("package_version", "test")
            .put(
                "options_schema_versions",
                JSONArray().put(MERMAN_BINDING_OPTIONS_SCHEMA_VERSION),
            )
                    .put(
                        "payload_schemas",
                JSONArray(
                    MERMAN_BINDING_TRANSPORT_EXPOSURE_SPECS
                        .getValue("android-jni")
                        .payloadSchemaIds
                        .sorted()
                        .map { id ->
                            JSONObject()
                                .put("id", id)
                                .put("version", MERMAN_REQUIRED_PAYLOAD_SCHEMA_VERSIONS.getValue(id))
                        },
                ),
            )
            .put(
                "capabilities",
                JSONObject()
                    .put("capability_ids", JSONArray(capabilityIds))
                    .put("output_ids", JSONArray(outputIds))
                    .put("operation_ids", JSONArray(operationIds))
                    .put("system_adapter_ids", JSONArray(artifact.systemAdapterIds))
                    .put(
                        "text_measurement",
                        JSONObject()
                            .put(
                                "protocol_version",
                                MermanTextMeasurementOperation.PROTOCOL_VERSION,
                            )
                            .put("provider_ids", JSONArray(providerIds)),
                    ),
            )
            .put(
                "metadata_ids",
                JSONArray(metadataIds),
            )
            .put(
                "output_contracts",
                JSONArray(outputIds.map { id ->
                    JSONObject(MERMAN_ANDROID_OUTPUT_CONTRACT_JSON_BY_ID.getValue(id))
                }),
            )
            .put("registry", JSONObject().put("diagram_family_count", 1))
            .put(
                "resources",
                JSONObject()
                    .put("general_binding_default_profile", "interactive")
                    .put("cli_default_profile", "trusted-native")
                    .put(
                        "limits",
                        JSONArray().put(
                            JSONObject()
                                .put("id", "max_source_bytes")
                                .put("phase", "source")
                                .put("description", "Maximum source bytes")
                                .put("overridable", true)
                                .put("hard_cap", false)
                                .put("minimum_value", 0)
                                .put(
                                    "operation_ids",
                                    JSONArray(operationIds),
                                ),
                        ),
                    )
                    .put(
                        "profiles",
                        JSONArray()
                            .put(
                                JSONObject()
                                    .put("id", "interactive")
                                    .put("purpose", "Interactive binding")
                                    .put("trust_assumption", "bounded")
                                    .put("recommended_binding_default", true)
                                .put("limits", JSONObject().put("max_source_bytes", 0)),
                            )
                            .put(
                                JSONObject()
                                    .put("id", "trusted-native")
                                    .put("purpose", "Trusted native")
                                    .put("trust_assumption", "trusted")
                                    .put("recommended_binding_default", false)
                                .put("limits", JSONObject().put("max_source_bytes", 0)),
                            ),
                    ),
            )
            .put("option_group_ids", JSONArray(optionIds))
            .put("constructor_service_ids", JSONArray(serviceIds))
            .put(
                "constructor_service_contracts",
                JSONArray(
                    serviceIds.map { id ->
                        val spec = MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS.getValue(id)
                        JSONObject()
                            .put("id", id)
                            .put(
                                "provided_text_measurement_provider_ids",
                                JSONArray(spec.providedTextMeasurementProviderIds.toList().sorted()),
                            )
                            .put(
                                "resource_limits",
                                JSONArray(
                                    spec.resourceLimits.sortedBy(MermanBindingConstructorResourceLimitSpec::id).map { limit ->
                                        JSONObject()
                                            .put("id", limit.id)
                                            .put("phase", limit.phase)
                                            .put("unit", limit.unit)
                                            .put("description", limit.description)
                                            .put("value", limit.value)
                                    },
                                ),
                            )
                    },
                ),
            )
    }

    private fun JSONObject.removeAndReturn(key: String): String {
        remove(key)
        return toString()
    }

    private fun JSONObject.mutateStringArray(
        key: String,
        mutation: JSONArray.() -> Unit,
    ): String {
        getJSONArray(key).mutation()
        return toString()
    }

    private fun JSONArray.swapFirstTwo() {
        val first = get(0)
        val second = get(1)
        put(0, second)
        put(1, first)
    }

    private fun JSONObject.withPayloadSchemas(vararg ids: String): String {
        val schemas = JSONArray()
        for (id in ids) {
            schemas.put(JSONObject().put("id", id).put("version", 1))
        }
        put("payload_schemas", schemas)
        return toString()
    }

    private fun JSONObject.withPayloadSchemaVersion(id: String, version: Int): String {
        val schemas = getJSONArray("payload_schemas")
        for (index in 0 until schemas.length()) {
            val schema = schemas.getJSONObject(index)
            if (schema.getString("id") == id) schema.put("version", version)
        }
        return toString()
    }

    private fun JSONObject.withTextMeasurement(value: Any): String {
        getJSONObject("capabilities").put("text_measurement", value)
        return toString()
    }

    private fun JSONObject.constructorServiceContract(id: String): JSONObject {
        val contracts = getJSONArray("constructor_service_contracts")
        for (index in 0 until contracts.length()) {
            val contract = contracts.getJSONObject(index)
            if (contract.getString("id") == id) return contract
        }
        error("missing constructor service contract: $id")
    }
}
