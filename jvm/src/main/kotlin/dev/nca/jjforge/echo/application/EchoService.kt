package dev.nca.jjforge.echo.application

import dev.nca.jjforge.echo.client.VcsClient
import org.springframework.stereotype.Service

/** Passes a message through vcs, so one request exercises REST and gRPC. */
@Service
class EchoService(
    private val vcs: VcsClient,
) {
    fun echo(message: String): String = vcs.echo(message)
}
