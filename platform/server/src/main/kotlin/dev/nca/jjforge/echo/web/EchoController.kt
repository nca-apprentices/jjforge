package dev.nca.jjforge.echo.web

import dev.nca.jjforge.api.EchoApi
import dev.nca.jjforge.api.model.Echo
import dev.nca.jjforge.echo.application.EchoService
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.RestController

/** Serves the echo of the contract. */
@RestController
class EchoController(
    private val echoes: EchoService,
) : EchoApi {
    override fun echo(echo: Echo): ResponseEntity<Echo> = ResponseEntity.ok(Echo(echoes.echo(echo.message)))
}
