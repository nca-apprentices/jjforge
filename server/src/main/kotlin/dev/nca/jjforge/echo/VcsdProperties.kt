package dev.nca.jjforge.echo

import jakarta.validation.constraints.NotBlank
import org.springframework.boot.context.properties.ConfigurationProperties
import org.springframework.validation.annotation.Validated

/** Where vcsd listens, as `host:port`. A blank target stops the server at startup. */
@ConfigurationProperties("jjforge.vcsd")
@Validated
data class VcsdProperties(
    @field:NotBlank val target: String,
)
