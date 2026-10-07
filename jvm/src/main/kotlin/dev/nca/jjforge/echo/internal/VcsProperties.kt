package dev.nca.jjforge.echo.internal

import jakarta.validation.constraints.NotBlank
import org.springframework.boot.context.properties.ConfigurationProperties
import org.springframework.validation.annotation.Validated

/** Where vcs listens, as `host:port`. A blank target stops the server at startup. */
@ConfigurationProperties("jjforge.vcs")
@Validated
data class VcsProperties(
    @field:NotBlank val target: String,
)
