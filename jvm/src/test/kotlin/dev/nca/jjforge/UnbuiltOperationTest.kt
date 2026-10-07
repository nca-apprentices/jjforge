package dev.nca.jjforge

import dev.nca.jjforge.identity.web.AuthController
import dev.nca.jjforge.identity.web.OrgsController
import dev.nca.jjforge.repos.web.ReposController
import dev.nca.jjforge.source.web.FilesController
import dev.nca.jjforge.source.web.HistoryController
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.ValueSource
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get
import org.springframework.test.web.servlet.result.MockMvcResultMatchers.status
import org.springframework.test.web.servlet.setup.MockMvcBuilders

/** An operation in the contract that no one has built yet answers 501, as ADR 0003 decides. */
class UnbuiltOperationTest {
    private val mvc =
        MockMvcBuilders
            .standaloneSetup(
                AuthController(),
                OrgsController(),
                ReposController(),
                HistoryController(),
                FilesController(),
            ).build()

    @ParameterizedTest
    @ValueSource(
        strings = [
            "/api/v1/me",
            "/api/v1/orgs",
            "/api/v1/orgs/acme/repos",
            "/api/v1/orgs/acme/repos/forge/operations",
            "/api/v1/orgs/acme/repos/forge/tree",
        ],
    )
    fun `an unbuilt operation answers 501`(path: String) {
        mvc.perform(get(path)).andExpect(status().isNotImplemented)
    }
}
