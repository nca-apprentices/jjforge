package dev.nca.jjforge.echo

import dev.nca.jjforge.echo.v1.EchoRequest
import dev.nca.jjforge.echo.v1.EchoServiceGrpc
import org.springframework.grpc.client.GrpcChannelFactory
import org.springframework.stereotype.Component

/**
 * Calls vcs over `echo/v1`. Plaintext, because vcs is only reachable in-cluster.
 * The channel comes from Spring gRPC, which traces each call and closes the
 * channel on shutdown.
 */
@Component
class VcsClient(
    properties: VcsProperties,
    channels: GrpcChannelFactory,
) {
    private val stub = EchoServiceGrpc.newBlockingStub(channels.createChannel(properties.target))

    fun echo(message: String): String = stub.echo(EchoRequest.newBuilder().setMessage(message).build()).message
}
