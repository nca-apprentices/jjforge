package dev.nca.jjforge.echo.client

import dev.nca.jjforge.TestcontainersConfiguration
import dev.nca.jjforge.echo.v1.EchoRequest
import dev.nca.jjforge.echo.v1.EchoResponse
import dev.nca.jjforge.echo.v1.EchoServiceGrpc
import io.grpc.Metadata
import io.grpc.ServerBuilder
import io.grpc.ServerCall
import io.grpc.ServerCallHandler
import io.grpc.ServerInterceptor
import io.grpc.ServerInterceptors
import io.grpc.stub.StreamObserver
import org.junit.jupiter.api.AfterAll
import org.junit.jupiter.api.Test
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.context.annotation.Import
import org.springframework.http.MediaType
import org.springframework.test.context.DynamicPropertyRegistry
import org.springframework.test.context.DynamicPropertySource
import org.springframework.web.client.RestClient
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.test.assertEquals

/**
 * Calls a real gRPC server through the server's own wiring, so a mismatch
 * between the protobuf code generator and the runtime fails here and not in
 * production, and so does a call to vcs that drops the caller's trace.
 */
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@Import(TestcontainersConfiguration::class)
class VcsClientTest
    @Autowired
    constructor(
        private val client: VcsClient,
        @param:LocalServerPort private val port: Int,
    ) {
        @Test
        fun `returns what vcs answers`() {
            assertEquals("hello", client.echo("hello"))
        }

        @Test
        fun `continues the caller's trace`() {
            RestClient
                .create("http://localhost:$port")
                .post()
                .uri("/api/v1/echo")
                .header("traceparent", "00-$TRACE-00f067aa0ba902b7-01")
                .contentType(MediaType.APPLICATION_JSON)
                .body("""{"message":"traced"}""")
                .retrieve()
                .toBodilessEntity()

            assertEquals(TRACE, traceparents.last().split("-")[1])
        }

        private class FakeVcs : EchoServiceGrpc.EchoServiceImplBase() {
            override fun echo(
                request: EchoRequest,
                response: StreamObserver<EchoResponse>,
            ) {
                response.onNext(EchoResponse.newBuilder().setMessage(request.message).build())
                response.onCompleted()
            }
        }

        /** Records the `traceparent` of every call. */
        private class Recorder : ServerInterceptor {
            override fun <Q, A> interceptCall(
                call: ServerCall<Q, A>,
                headers: Metadata,
                next: ServerCallHandler<Q, A>,
            ): ServerCall.Listener<Q> {
                headers.get(TRACEPARENT)?.let(traceparents::add)
                return next.startCall(call, headers)
            }
        }

        companion object {
            private const val TRACE = "4bf92f3577b34da6a3ce929d0e0e4736"

            private val TRACEPARENT = Metadata.Key.of("traceparent", Metadata.ASCII_STRING_MARSHALLER)

            private val traceparents = CopyOnWriteArrayList<String>()

            private val vcs =
                ServerBuilder
                    .forPort(0)
                    .addService(ServerInterceptors.intercept(FakeVcs(), Recorder()))
                    .build()
                    .start()

            @JvmStatic
            @DynamicPropertySource
            fun target(registry: DynamicPropertyRegistry) {
                registry.add("jjforge.vcs.target") { "localhost:${vcs.port}" }
            }

            @JvmStatic
            @AfterAll
            fun stop() {
                vcs.shutdownNow()
            }
        }
    }
