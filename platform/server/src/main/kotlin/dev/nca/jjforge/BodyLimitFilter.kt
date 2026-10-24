package dev.nca.jjforge

import jakarta.servlet.FilterChain
import jakarta.servlet.ReadListener
import jakarta.servlet.ServletInputStream
import jakarta.servlet.http.HttpServletRequest
import jakarta.servlet.http.HttpServletRequestWrapper
import jakarta.servlet.http.HttpServletResponse
import org.springframework.http.HttpStatus
import org.springframework.stereotype.Component
import org.springframework.web.filter.OncePerRequestFilter
import java.io.BufferedReader
import java.io.IOException
import java.io.InputStreamReader

/**
 * Bounds every request body at MAX_BODY_BYTES, since nothing in front of the
 * server does: Cloudflare passes 100 MB, and Traefik and Tomcat pass any
 * size. A declared length over the bound answers 413 before any code reads
 * the body. A chunked body fails with an IOException once it passes the
 * bound, which the message converters answer as 400.
 */
@Component
class BodyLimitFilter : OncePerRequestFilter() {
    override fun doFilterInternal(
        request: HttpServletRequest,
        response: HttpServletResponse,
        chain: FilterChain,
    ) {
        if (request.contentLengthLong > MAX_BODY_BYTES) {
            response.sendError(HttpStatus.CONTENT_TOO_LARGE.value())
            return
        }

        chain.doFilter(BoundedRequest(request), response)
    }

    private class BoundedRequest(
        request: HttpServletRequest,
    ) : HttpServletRequestWrapper(request) {
        private val body by lazy { BoundedStream(request.inputStream) }

        override fun getInputStream(): ServletInputStream = body

        override fun getReader() = BufferedReader(InputStreamReader(body, characterEncoding ?: Charsets.UTF_8.name()))
    }

    private class BoundedStream(
        private val source: ServletInputStream,
    ) : ServletInputStream() {
        private var count = 0L

        override fun read(): Int {
            val byte = source.read()
            if (byte >= 0) {
                count(1)
            }
            return byte
        }

        override fun read(
            b: ByteArray,
            off: Int,
            len: Int,
        ): Int {
            val bytes = source.read(b, off, len)
            if (bytes > 0) {
                count(bytes)
            }
            return bytes
        }

        override fun isFinished() = source.isFinished

        override fun isReady() = source.isReady

        override fun setReadListener(listener: ReadListener) = source.setReadListener(listener)

        private fun count(bytes: Int) {
            count += bytes
            if (count > MAX_BODY_BYTES) {
                throw IOException("The request body exceeds $MAX_BODY_BYTES bytes.")
            }
        }
    }

    private companion object {
        const val MAX_BODY_BYTES = 1024 * 1024L
    }
}
