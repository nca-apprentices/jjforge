package dev.nca.jjforge.vcs.client

import org.springframework.boot.context.properties.ConfigurationProperties
import org.springframework.validation.annotation.Validated

/**
 * The principal the server names to vcs, as a line of vcs's principals file
 * names it. A blank principal sends every call without one, which only
 * `echo/v1` accepts.
 */
@ConfigurationProperties("jjforge.vcs")
@Validated
data class VcsProperties(
    val principal: String = "",
)
