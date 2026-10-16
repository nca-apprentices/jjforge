package dev.nca.jjforge.telemetry.web

import dev.nca.jjforge.TestcontainersConfiguration
import io.opentelemetry.sdk.testing.exporter.InMemorySpanExporter
import io.opentelemetry.sdk.trace.SdkTracerProvider
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.context.TestConfiguration
import org.springframework.boot.test.system.CapturedOutput
import org.springframework.boot.test.system.OutputCaptureExtension
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Import
import org.springframework.web.client.RestClient
import java.util.concurrent.TimeUnit
import kotlin.test.assertTrue

/**
 * A span and a line name a route's template, never the names in the path a
 * caller sent, because an organization may be named after a person, as ADR
 * 0005 decides.
 */
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@Import(TestcontainersConfiguration::class, SpanNamesTest.Spans::class)
@ExtendWith(OutputCaptureExtension::class)
class SpanNamesTest
    @Autowired
    constructor(
        @param:LocalServerPort private val port: Int,
        private val spans: InMemorySpanExporter,
        private val provider: SdkTracerProvider,
    ) {
        @Test
        fun `no span or line names an organization or repository of the path`(output: CapturedOutput) {
            RestClient
                .create("http://localhost:$port")
                .get()
                .uri("/api/v1/orgs/$ORG/repos/$REPO")
                .exchange { _, response -> response.statusCode }
            provider.forceFlush().join(FLUSH_SECONDS, TimeUnit.SECONDS)

            val recorded =
                spans.finishedSpanItems.flatMap { span ->
                    listOf(span.name) + span.attributes.asMap().values.map(Any::toString)
                }
            assertTrue(recorded.isNotEmpty())
            assertTrue(recorded.none { ORG in it || REPO in it }, recorded.toString())
            assertTrue(ORG !in output.out && REPO !in output.out, output.out)
        }

        /** Keeps every span the server finishes, so the test can read it. */
        @TestConfiguration(proxyBeanMethods = false)
        class Spans {
            @Bean
            fun spans(): InMemorySpanExporter = InMemorySpanExporter.create()
        }

        private companion object {
            const val ORG = "ada-lovelace"
            const val REPO = "private-notes"
            const val FLUSH_SECONDS = 10L
        }
    }
