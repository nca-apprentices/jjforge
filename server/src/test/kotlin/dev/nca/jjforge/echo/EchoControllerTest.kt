package dev.nca.jjforge.echo

import org.junit.jupiter.api.Test
import org.mockito.Mockito.mock
import org.mockito.Mockito.`when`
import org.springframework.http.MediaType
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post
import org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath
import org.springframework.test.web.servlet.result.MockMvcResultMatchers.status
import org.springframework.test.web.servlet.setup.MockMvcBuilders

class EchoControllerTest {
    private val vcsd = mock(VcsdClient::class.java)
    private val mvc = MockMvcBuilders.standaloneSetup(EchoController(vcsd)).build()

    @Test
    fun `returns what vcsd answers`() {
        `when`(vcsd.echo("hello")).thenReturn("hello")

        mvc
            .perform(
                post("/api/echo")
                    .contentType(MediaType.APPLICATION_JSON)
                    .content("""{"message":"hello"}"""),
            ).andExpect(status().isOk)
            .andExpect(jsonPath("$.message").value("hello"))
    }
}
