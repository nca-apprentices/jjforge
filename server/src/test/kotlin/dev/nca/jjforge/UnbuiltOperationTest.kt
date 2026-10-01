package dev.nca.jjforge

import dev.nca.jjforge.source.ReposController
import org.junit.jupiter.api.Test
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get
import org.springframework.test.web.servlet.result.MockMvcResultMatchers.status
import org.springframework.test.web.servlet.setup.MockMvcBuilders

/** An operation in the contract that no one has built yet answers 501, as ADR 0011 decides. */
class UnbuiltOperationTest {
    private val mvc = MockMvcBuilders.standaloneSetup(ReposController()).build()

    @Test
    fun `an unbuilt operation answers 501`() {
        mvc.perform(get("/api/orgs/acme/repos")).andExpect(status().isNotImplemented)
    }
}
