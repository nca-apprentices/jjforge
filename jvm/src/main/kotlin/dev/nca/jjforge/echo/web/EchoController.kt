package dev.nca.jjforge.echo.web

import dev.nca.jjforge.api.EchoApi
import dev.nca.jjforge.api.model.Echo
import dev.nca.jjforge.echo.internal.VcsClient
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.RestController

/** Passes the message through vcs, so one request exercises REST and gRPC. */
@RestController
class EchoController(
    private val vcs: VcsClient,
) : EchoApi {
    override fun echo(echo: Echo): ResponseEntity<Echo> = ResponseEntity.ok(Echo(vcs.echo(echo.message)))
}
