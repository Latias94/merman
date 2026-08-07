package io.merman

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class MermanOperationMetadataTest {
    @Test
    fun outputPlanVocabularyIsNotSealed() {
        assertFalse(MermanOutputPlan::class.java.isSealed)
    }

    @Test
    fun decodesKnownRasterAndPdfPlans() {
        val raster = decodeMermanOperationMetadata(
            metadataJson(
                operationId = "png",
                mediaType = "image/png",
                byteLength = 128,
                outputPlan = """
                    {
                      "kind":"raster",
                      "requested_width_px":640,
                      "requested_height_px":480,
                      "width_px":320,
                      "height_px":240,
                      "requested_scale":2,
                      "effective_scale":1,
                      "limited":true
                    }
                """.trimIndent(),
            ),
        )
        val rasterPlan = raster.outputPlan as MermanRasterOutputPlan
        assertEquals(320, rasterPlan.widthPx)
        assertEquals(240, rasterPlan.heightPx)
        assertTrue(rasterPlan.limited)
        assertEquals(128L, raster.byteLength)

        val pdf = decodeMermanOperationMetadata(
            metadataJson(
                operationId = "pdf",
                mediaType = "application/pdf",
                byteLength = 256,
                outputPlan = """
                    {
                      "kind":"pdf-filter-images",
                      "filtered_groups":2,
                      "requested_scale":2,
                      "effective_scale":1,
                      "requested_image_pixels":1000,
                      "effective_image_pixels":800,
                      "limited":true
                    }
                """.trimIndent(),
            ),
        )
        val pdfPlan = pdf.outputPlan as MermanPdfFilterImagesOutputPlan
        assertEquals(2L, pdfPlan.filteredGroups)
        assertEquals(800L, pdfPlan.effectiveImagePixels)
        assertTrue(pdfPlan.limited)
    }

    @Test
    fun preservesUnknownOutputPlanAndOriginalMetadataJson() {
        val raw = metadataJson(
            operationId = "future",
            mediaType = "application/x-future",
            byteLength = 7,
            outputPlan = """{"kind":"future-plan","nested":{"answer":42}}""",
        )

        val metadata = decodeMermanOperationMetadata(raw)
        val plan = metadata.outputPlan as MermanUnknownOutputPlan
        assertEquals("future-plan", plan.kind)
        assertEquals(42, plan.jsonObject.getJSONObject("nested").getInt("answer"))
        assertEquals(raw, metadata.rawJson)
        assertEquals(42, metadata.jsonObject.getJSONObject("output_plan").getJSONObject("nested").getInt("answer"))
    }

    @Test
    fun operationResultEnforcesEnvelopeMetadataInvariants() {
        val data = byteArrayOf(1, 2, 3)
        val result = MermanOperationResult(
            "png",
            "image/png",
            data,
            metadataJson("png", "image/png", data.size.toLong(), "null"),
        )
        assertEquals("png", result.metadata.operationId)
        assertTrue(result.data.contentEquals(data))

        assertRejectedResult(
            "svg",
            "image/png",
            data,
            metadataJson("png", "image/png", 3, "null"),
            "Merman result operation ID does not match its metadata",
        )
        assertRejectedResult(
            "png",
            "image/jpeg",
            data,
            metadataJson("png", "image/png", 3, "null"),
            "Merman result media type does not match its metadata",
        )
        assertRejectedResult(
            "png",
            "image/png",
            data,
            metadataJson("png", "image/png", 2, "null"),
            "Merman result byte length does not match its metadata",
        )
    }

    @Test
    fun operationResultDecodesUtf8Strictly() {
        val validData = "SVG 你好".toByteArray(Charsets.UTF_8)
        val validResult = MermanOperationResult(
            "svg",
            "image/svg+xml",
            validData,
            metadataJson("svg", "image/svg+xml", validData.size.toLong(), "null"),
        )
        assertEquals("SVG 你好", validResult.utf8Text)

        val invalidData = byteArrayOf(0xC3.toByte(), 0x28)
        val invalidResult = MermanOperationResult(
            "svg",
            "image/svg+xml",
            invalidData,
            metadataJson("svg", "image/svg+xml", invalidData.size.toLong(), "null"),
        )
        val error = runCatching { invalidResult.utf8Text }.exceptionOrNull()
        assertTrue(error is MermanException)
        error as MermanException
        assertEquals(9, error.code)
        assertEquals("MERMAN_INTERNAL_ERROR", error.codeName)
        assertEquals(MermanErrorKind.GENERIC, error.kind)
        assertEquals("Merman operation result data is not valid UTF-8", error.message)
    }

    @Test
    fun rejectsMalformedKnownPlansButAllowsNoPlan() {
        val noPlan = decodeMermanOperationMetadata(
            metadataJson("svg", "image/svg+xml", 3, "null"),
        )
        assertNull(noPlan.outputPlan)

        val error = runCatching {
            decodeMermanOperationMetadata(
                metadataJson(
                    "png",
                    "image/png",
                    3,
                    """{"kind":"raster"}""",
                ),
            )
        }.exceptionOrNull()
        assertTrue(error is MermanException)
    }

    private fun assertRejectedResult(
        operationId: String,
        mediaType: String,
        data: ByteArray,
        metadataJson: String,
        expectedMessage: String,
    ) {
        try {
            MermanOperationResult(operationId, mediaType, data, metadataJson)
            fail("mismatched result envelope was accepted")
        } catch (error: MermanException) {
            assertEquals(9, error.code)
            assertEquals("MERMAN_INTERNAL_ERROR", error.codeName)
            assertEquals(MermanErrorKind.GENERIC, error.kind)
            assertEquals(expectedMessage, error.message)
        }
    }

    private fun metadataJson(
        operationId: String,
        mediaType: String,
        byteLength: Long,
        outputPlan: String,
    ): String = """
        {
          "version":1,
          "operation_id":"$operationId",
          "media_type":"$mediaType",
          "runtime_policy":"deterministic",
          "byte_length":$byteLength,
          "output_plan":$outputPlan
        }
    """.trimIndent()
}
