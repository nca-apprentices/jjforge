package dev.nca.jjforge.telemetry.web

import dev.nca.jjforge.TestcontainersConfiguration
import org.junit.jupiter.api.Test
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.ValueSource
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.context.annotation.Import
import org.springframework.web.client.RestClient
import kotlin.test.assertEquals

/**
 * Every response names the trace of its request, whatever answers it, so no
 * endpoint has to remember to, as ADR 0005 decides.
 */
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@Import(TestcontainersConfiguration::class)
class TraceResponseTest
    @Autowired
    constructor(
        @param:LocalServerPort private val port: Int,
    ) {
        @ParameterizedTest
        @ValueSource(strings = ["/api/v1/orgs", "/api/v1/nowhere", "/"])
        fun `a response names the caller's trace`(path: String) {
            val response =
                RestClient
                    .create("http://localhost:$port")
                    .get()
                    .uri(path)
                    .header("traceparent", "00-$TRACE-00f067aa0ba902b7-01")
                    .exchange { _, response -> response.headers.getFirst("traceresponse") }

            assertEquals(TRACE, response?.split("-")?.get(1))
        }

        @Test
        fun `a caller can't turn off its trace`() {
            val response =
                RestClient
                    .create("http://localhost:$port")
                    .get()
                    .uri("/api/v1/orgs")
                    .header("traceparent", "00-$TRACE-00f067aa0ba902b7-00")
                    .exchange { _, response -> response.headers.getFirst("traceresponse") }

            assertEquals(TRACE, response?.split("-")?.get(1))
            assertEquals("01", response?.split("-")?.get(3))
        }

        private companion object {
            const val TRACE = "4bf92f3577b34da6a3ce929d0e0e4736"
        }
    }
