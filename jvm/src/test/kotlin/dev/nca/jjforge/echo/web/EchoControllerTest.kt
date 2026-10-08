package dev.nca.jjforge.echo.web

import dev.nca.jjforge.echo.application.EchoService
import dev.nca.jjforge.echo.client.VcsClient
import org.junit.jupiter.api.Test
import org.mockito.Mockito.mock
import org.mockito.Mockito.`when`
import org.springframework.http.MediaType
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post
import org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath
import org.springframework.test.web.servlet.result.MockMvcResultMatchers.status
import org.springframework.test.web.servlet.setup.MockMvcBuilders

class EchoControllerTest {
    private val vcs = mock(VcsClient::class.java)
    private val mvc = MockMvcBuilders.standaloneSetup(EchoController(EchoService(vcs))).build()

    @Test
    fun `returns what vcs answers`() {
        `when`(vcs.echo("hello")).thenReturn("hello")

        mvc
            .perform(
                post("/api/v1/echo")
                    .contentType(MediaType.APPLICATION_JSON)
                    .content("""{"message":"hello"}"""),
            ).andExpect(status().isOk)
            .andExpect(jsonPath("$.message").value("hello"))
    }
}
