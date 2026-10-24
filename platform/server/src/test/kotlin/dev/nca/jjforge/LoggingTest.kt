package dev.nca.jjforge

import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper
import io.micrometer.tracing.Tracer
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith
import org.slf4j.LoggerFactory
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.system.CapturedOutput
import org.springframework.boot.test.system.OutputCaptureExtension
import org.springframework.context.annotation.Import
import kotlin.test.assertEquals

/** A log line is one JSON object with the trace that wrote it, as ADR 0005 decides. */
@SpringBootTest
@Import(TestcontainersConfiguration::class)
@ExtendWith(OutputCaptureExtension::class)
class LoggingTest
    @Autowired
    constructor(
        private val tracer: Tracer,
    ) {
        @Test
        fun `a line names its trace and span`(output: CapturedOutput) {
            val span = tracer.nextSpan().name("probe").start()
            tracer.withSpan(span).use { LoggerFactory.getLogger(javaClass).info("probe") }
            span.end()

            val line = jacksonObjectMapper().readTree(output.out.lines().last { "\"probe\"" in it })
            assertEquals("probe", line["message"].asText())
            assertEquals(span.context().traceId(), line["trace_id"].asText())
            assertEquals(span.context().spanId(), line["span_id"].asText())
        }
    }
