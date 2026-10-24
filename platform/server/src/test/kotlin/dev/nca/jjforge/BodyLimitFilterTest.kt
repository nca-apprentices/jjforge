package dev.nca.jjforge

import jakarta.servlet.FilterChain
import org.junit.jupiter.api.Test
import org.springframework.mock.web.MockFilterChain
import org.springframework.mock.web.MockHttpServletRequest
import org.springframework.mock.web.MockHttpServletResponse
import java.io.IOException
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

/**
 * No request body over 1 MiB reaches a controller, whether it names its
 * length or arrives in chunks.
 */
class BodyLimitFilterTest {
    private val filter = BodyLimitFilter()

    @Test
    fun `answers 413 to a declared length over the bound`() {
        val chain = MockFilterChain()
        val response = MockHttpServletResponse()

        filter.doFilter(request(OVER), response, chain)

        assertEquals(CONTENT_TOO_LARGE, response.status)
        assertNull(chain.request)
    }

    @Test
    fun `stops reading a chunked body at the bound`() {
        val chunked =
            object : MockHttpServletRequest("POST", "/api/v1/echo") {
                override fun getContentLengthLong() = -1L

                override fun getContentLength() = -1
            }
        chunked.setContent(ByteArray(OVER))
        val chain = FilterChain { request, _ -> request.inputStream.readAllBytes() }

        assertFailsWith<IOException> { filter.doFilter(chunked, MockHttpServletResponse(), chain) }
    }

    @Test
    fun `passes a body at the bound`() {
        val chain =
            FilterChain {
                request,
                _,
                ->
                assertEquals(BOUND, request.inputStream.readAllBytes().size)
            }

        filter.doFilter(request(BOUND), MockHttpServletResponse(), chain)
    }

    private fun request(size: Int) =
        MockHttpServletRequest("POST", "/api/v1/echo").apply {
            setContent(ByteArray(size))
        }

    private companion object {
        const val BOUND = 1024 * 1024
        const val OVER = BOUND + 1
        const val CONTENT_TOO_LARGE = 413
    }
}
