package dev.nca.jjforge.telemetry.web

import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import dev.nca.jjforge.TestcontainersConfiguration
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.system.CapturedOutput
import org.springframework.boot.test.system.OutputCaptureExtension
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.context.annotation.Import
import org.springframework.web.client.RestClient
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Each request writes one `request` line when it ends, its record when its
 * trace is lost, as ADR 0005 decides.
 */
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@Import(TestcontainersConfiguration::class)
@ExtendWith(OutputCaptureExtension::class)
class RequestLineTest
    @Autowired
    constructor(
        @param:LocalServerPort private val port: Int,
    ) {
        @Test
        fun `a request writes one line when it ends`(output: CapturedOutput) {
            RestClient
                .create("http://localhost:$port")
                .get()
                .uri("/api/v1/orgs/acme")
                .header("traceparent", "00-$TRACE-00f067aa0ba902b7-01")
                .exchange { _, response -> response.statusCode }

            val lines =
                output.out
                    .lines()
                    .filter { "\"request\"" in it }
                    .map { jacksonObjectMapper().readTree(it) }
                    .filter { it["message"].asText() == "request" && it["trace_id"]?.asText() == TRACE }
            assertEquals(1, lines.size)
            val line = lines.single()
            assertEquals("GET", line["method"].asText())
            assertEquals("/api/v1/orgs/{org}", line["route"].asText())
            assertEquals(501, line["status"].asInt())
            assertEquals("failed", line["outcome"].asText())
            assertTrue(line["duration_ms"].isNumber)
        }

        private companion object {
            const val TRACE = "4bf92f3577b34da6a3ce929d0e0e4736"
        }
    }
