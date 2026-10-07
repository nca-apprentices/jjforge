package dev.nca.jjforge.telemetry

import io.micrometer.tracing.Tracer
import jakarta.servlet.FilterChain
import jakarta.servlet.http.HttpServletRequest
import jakarta.servlet.http.HttpServletResponse
import org.springframework.stereotype.Component
import org.springframework.web.filter.OncePerRequestFilter

/**
 * Names the trace of every request in a `traceresponse` header, in the format
 * of W3C Trace Context Level 2, so a person can quote it and no endpoint has
 * to, as ADR 0005 decides. It runs after the filter that starts the request's
 * span, and sets the header before any code can commit the response.
 */
@Component
class TraceResponseFilter(
    private val tracer: Tracer,
) : OncePerRequestFilter() {
    override fun doFilterInternal(
        request: HttpServletRequest,
        response: HttpServletResponse,
        chain: FilterChain,
    ) {
        tracer.currentSpan()?.context()?.let { span ->
            val flags =
                if (span.sampled() == true) {
                    SAMPLED
                } else {
                    UNSAMPLED
                }
            response.setHeader(HEADER, "$VERSION-${span.traceId()}-${span.spanId()}-$flags")
        }
        chain.doFilter(request, response)
    }

    // The header, its version, and its trace flags, from W3C Trace Context.
    private companion object {
        const val HEADER = "traceresponse"
        const val VERSION = "00"
        const val SAMPLED = "01"
        const val UNSAMPLED = "00"
    }
}
