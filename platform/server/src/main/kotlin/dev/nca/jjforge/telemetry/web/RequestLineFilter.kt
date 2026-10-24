package dev.nca.jjforge.telemetry.web

import jakarta.servlet.FilterChain
import jakarta.servlet.http.HttpServletRequest
import jakarta.servlet.http.HttpServletResponse
import org.slf4j.LoggerFactory
import org.springframework.stereotype.Component
import org.springframework.web.filter.OncePerRequestFilter
import org.springframework.web.servlet.HandlerMapping

/**
 * Writes one `request` line when each request ends, with its route, status,
 * outcome, and duration, as ADR 0005 decides. It is the request's record when
 * its trace is sampled out, lost, or expired, so it repeats the span on
 * purpose. It runs inside the request's span, so the line carries its trace.
 */
@Component
class RequestLineFilter : OncePerRequestFilter() {
    override fun doFilterInternal(
        request: HttpServletRequest,
        response: HttpServletResponse,
        chain: FilterChain,
    ) {
        val start = System.nanoTime()
        try {
            chain.doFilter(request, response)
        } finally {
            val status = response.status
            log
                .atInfo()
                .addKeyValue("method", request.method)
                .addKeyValue("route", route(request))
                .addKeyValue("status", status)
                .addKeyValue("outcome", outcome(status))
                .addKeyValue("duration_ms", (System.nanoTime() - start) / NANOS_PER_MILLI)
                .log("request")
        }
    }

    /** The route's template, such as `/api/v1/orgs/{org}`, never the path a caller sent. */
    private fun route(request: HttpServletRequest): String =
        request.getAttribute(HandlerMapping.BEST_MATCHING_PATTERN_ATTRIBUTE) as? String ?: UNMATCHED

    private fun outcome(status: Int): String =
        when {
            status >= SERVER_ERROR -> "failed"
            status >= CLIENT_ERROR -> "rejected"
            else -> "ok"
        }

    private companion object {
        val log = LoggerFactory.getLogger(RequestLineFilter::class.java)

        const val NANOS_PER_MILLI = 1_000_000

        // The first status of each class, from RFC 9110.
        const val CLIENT_ERROR = 400
        const val SERVER_ERROR = 500

        /**
         * The route of a request that no handler matched, so no path a caller sent
         * reaches a line.
         */
        const val UNMATCHED = "unmatched"
    }
}
