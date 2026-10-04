package dev.nca.jjforge.echo

import dev.nca.jjforge.echo.v1.EchoRequest
import dev.nca.jjforge.echo.v1.EchoServiceGrpc
import io.grpc.ManagedChannelBuilder
import jakarta.annotation.PreDestroy
import org.springframework.stereotype.Component

/** Calls vcs over `echo/v1`. Plaintext, because vcs is only reachable in-cluster. */
@Component
class VcsClient(
    properties: VcsProperties,
) {
    private val channel = ManagedChannelBuilder.forTarget(properties.target).usePlaintext().build()

    private val stub = EchoServiceGrpc.newBlockingStub(channel)

    fun echo(message: String): String = stub.echo(EchoRequest.newBuilder().setMessage(message).build()).message

    @PreDestroy
    fun shutdown() {
        channel.shutdown()
    }
}
