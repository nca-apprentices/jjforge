package dev.nca.jjforge.echo

import dev.nca.jjforge.api.EchoApi
import dev.nca.jjforge.api.model.Echo
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.RestController

/** Passes the message through vcsd, so one request exercises REST and gRPC. */
@RestController
class EchoController(
    private val vcsd: VcsdClient,
) : EchoApi {
    override fun echo(echo: Echo): ResponseEntity<Echo> = ResponseEntity.ok(Echo(vcsd.echo(echo.message)))
}
