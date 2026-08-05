package io.merman

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MermanSemanticOperationFixtureTest {
    @Test
    fun consumesSharedSemanticOperationFixtures() {
        val fixtureJson = InstrumentationRegistry
            .getInstrumentation()
            .context
            .assets
            .open("semantic-operations-v1.json")
            .bufferedReader()
            .use { it.readText() }

        for ((index, fixture) in loadFixtures(fixtureJson).withIndex()) {
            val result = runCatching {
                Merman.execute(
                    operationId = fixture.operationId,
                    source = fixture.source,
                    optionsJson = fixture.optionsJson,
                    uri = fixture.uri,
                )
            }
            val label = "fixture $index operation `${fixture.operationId}`"

            if (fixture.expectedMediaType != null) {
                val operationResult = result.getOrElse { error ->
                    throw AssertionError("$label unexpectedly failed", error)
                }
                check(operationResult.operationId == fixture.operationId) { label }
                check(operationResult.mediaType == fixture.expectedMediaType) { label }
                assertSuccessInvariants(fixture, operationResult, label)
            } else {
                val error = result.exceptionOrNull()
                check(error is MermanException) { "$label unexpectedly succeeded" }
                check(error.kind.wireName == fixture.expectedErrorKind) { label }
                check(error.capabilityId == null) { label }
                assertErrorInvariants(fixture, error, label)
            }
        }
    }

    private fun assertSuccessInvariants(
        fixture: SemanticOperationFixture,
        result: MermanOperationResult,
        label: String,
    ) {
        for (invariant in fixture.payloadInvariants) {
            when (invariant) {
                "nonempty" -> check(result.data.isNotEmpty()) { label }
                "utf8" -> result.data.decodeToString(throwOnInvalidSequence = true)
                "json-object" -> JSONObject(
                    result.data.decodeToString(throwOnInvalidSequence = true),
                )
                "svg-root" -> check(
                    result.data
                        .decodeToString(throwOnInvalidSequence = true)
                        .trimStart()
                        .startsWith("<svg"),
                ) {
                    label
                }
                "metadata-operation-id" -> check(
                    result.metadata.operationId == fixture.operationId,
                ) {
                    label
                }
                else -> error("$label has unsupported success invariant `$invariant`")
            }
        }
    }

    private fun assertErrorInvariants(
        fixture: SemanticOperationFixture,
        exception: MermanException,
        label: String,
    ) {
        for (invariant in fixture.payloadInvariants) {
            when (invariant) {
                "error-message-nonempty" -> check(!exception.message.isNullOrEmpty()) { label }
                else -> error("$label has unsupported error invariant `$invariant`")
            }
        }
    }

    @Test
    fun consumesGeneratedThirteenOperationMatrix() {
        val source = "flowchart TD\nA --> B"
        val documentSource = "```mermaid\n$source\n```\n"
        val expectations = MERMAN_BINDING_OPERATION_EXPECTATIONS

        check(expectations.size == 13)
        val runtimeOperationIds = JSONObject(Merman.runtimeCatalogJson())
            .getJSONObject("capabilities")
            .getJSONArray("operation_ids")
            .strings()
            .toSet()
        check(runtimeOperationIds == expectations.map { it.operationId }.toSet())

        for (expectation in expectations) {
            val result = Merman.execute(
                operationId = expectation.operationId,
                source = if (expectation.requiresUri) documentSource else source,
                uri = if (expectation.requiresUri) "file:///fixtures/generated-matrix.md" else null,
            )
            check(result.operationId == expectation.operationId) { expectation.operationId }
            check(result.mediaType == expectation.mediaType) { expectation.operationId }
            check(result.metadata.version == expectation.metadataSchemaVersion) {
                expectation.operationId
            }
            check(result.metadata.operationId == expectation.operationId) { expectation.operationId }
            check(result.metadata.mediaType == expectation.mediaType) { expectation.operationId }
            check(result.metadata.byteLength == result.data.size.toLong()) { expectation.operationId }
        }
    }
}

private data class SemanticOperationFixture(
    val operationId: String,
    val source: String,
    val uri: String?,
    val optionsJson: String?,
    val expectedMediaType: String?,
    val expectedErrorKind: String?,
    val payloadInvariants: List<String>,
)

private fun loadFixtures(json: String): List<SemanticOperationFixture> {
    val root = JSONObject(json)
    check(root.get("schema_version") is Int && root.getInt("schema_version") == 1) {
        "unsupported semantic operation fixture schema"
    }
    val cases = root.getJSONArray("cases")

    return List(cases.length()) { index ->
        val fixture = cases.getJSONObject(index)
        SemanticOperationFixture(
            operationId = fixture.getString("operation_id"),
            source = fixture.getString("source"),
            uri = fixture.optionalString("uri"),
            optionsJson = fixture
                .takeIf { it.has("options") }
                ?.getJSONObject("options")
                ?.toString(),
            expectedMediaType = fixture.optionalString("expected_media_type"),
            expectedErrorKind = fixture.optionalString("expected_error_kind"),
            payloadInvariants = fixture
                .getJSONArray("payload_invariants")
                .strings(),
        )
    }
}

private fun JSONObject.optionalString(key: String): String? =
    if (has(key)) getString(key) else null

private fun JSONArray.strings(): List<String> = List(length()) { getString(it) }
