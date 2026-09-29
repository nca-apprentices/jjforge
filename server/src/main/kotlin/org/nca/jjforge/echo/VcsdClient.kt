package org.nca.jjforge.echo

import io.grpc.ManagedChannelBuilder
import jakarta.annotation.PreDestroy
import org.nca.jjforge.echo.v1.EchoRequest
import org.nca.jjforge.echo.v1.EchoServiceGrpc
import org.springframework.beans.factory.annotation.Value
import org.springframework.stereotype.Component

/** Calls vcsd over `echo/v1`. Plaintext, because vcsd is only reachable in-cluster. */
@Component
class VcsdClient(
    @Value("\${jjforge.vcsd.target}") target: String,
) {
    private val channel = ManagedChannelBuilder.forTarget(target).usePlaintext().build()

    private val stub = EchoServiceGrpc.newBlockingStub(channel)

    fun echo(message: String): String = stub.echo(EchoRequest.newBuilder().setMessage(message).build()).message

    @PreDestroy
    fun shutdown() {
        channel.shutdown()
    }
}
