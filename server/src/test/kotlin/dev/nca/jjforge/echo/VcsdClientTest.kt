package dev.nca.jjforge.echo

import dev.nca.jjforge.echo.v1.EchoRequest
import dev.nca.jjforge.echo.v1.EchoResponse
import dev.nca.jjforge.echo.v1.EchoServiceGrpc
import io.grpc.ServerBuilder
import io.grpc.stub.StreamObserver
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals

/**
 * Calls a real gRPC server with the generated stubs, so a mismatch between the
 * protobuf code generator and the runtime fails here and not in production.
 */
class VcsdClientTest {
    private class Echo : EchoServiceGrpc.EchoServiceImplBase() {
        override fun echo(
            request: EchoRequest,
            response: StreamObserver<EchoResponse>,
        ) {
            response.onNext(EchoResponse.newBuilder().setMessage(request.message).build())
            response.onCompleted()
        }
    }

    @Test
    fun `returns what vcsd answers`() {
        val server =
            ServerBuilder
                .forPort(0)
                .addService(Echo())
                .build()
                .start()
        val client = VcsdClient(VcsdProperties("localhost:${server.port}"))

        try {
            assertEquals("hello", client.echo("hello"))
        } finally {
            client.shutdown()
            server.shutdownNow()
        }
    }
}
