package dev.nca.jjforge.vcs.client

import org.springframework.boot.context.properties.ConfigurationProperties
import org.springframework.validation.annotation.Validated

/**
 * The server's SSH key, an unencrypted Ed25519 key in OpenSSH form, with its
 * certificate next to it as `{key}-cert.pub`. A blank key sends every call to
 * vcs without a session, which only `echo/v1` accepts.
 */
@ConfigurationProperties("jjforge.vcs.credential")
@Validated
data class VcsCredentialProperties(
    val key: String = "",
)
