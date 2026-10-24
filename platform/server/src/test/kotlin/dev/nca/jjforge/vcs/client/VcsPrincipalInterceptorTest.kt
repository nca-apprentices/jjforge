package dev.nca.jjforge.vcs.client

import dev.nca.jjforge.echo.v1.EchoRequest
import dev.nca.jjforge.echo.v1.EchoResponse
import dev.nca.jjforge.echo.v1.EchoServiceGrpc
import io.grpc.CallOptions
import io.grpc.Channel
import io.grpc.ClientCall
import io.grpc.ClientInterceptors
import io.grpc.Metadata
import io.grpc.MethodDescriptor
import io.grpc.Status
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test

/** Every call to vcs names the server's principal, and none without one. */
class VcsPrincipalInterceptorTest {
    /** Answers every echo and keeps the `authorization` of the last call. */
    private class FakeVcs : Channel() {
        var authorization: String? = null

        override fun authority() = "vcs"

        override fun <Req, Res> newCall(
            method: MethodDescriptor<Req, Res>,
            options: CallOptions,
        ): ClientCall<Req, Res> =
            object : ClientCall<Req, Res>() {
                private lateinit var listener: Listener<Res>

                override fun start(
                    listener: Listener<Res>,
                    headers: Metadata,
                ) {
                    this.listener = listener
                    authorization = headers.get(Metadata.Key.of("authorization", Metadata.ASCII_STRING_MARSHALLER))
                }

                override fun request(numMessages: Int) = Unit

                override fun cancel(
                    message: String?,
                    cause: Throwable?,
                ) = Unit

                @Suppress("UNCHECKED_CAST")
                override fun halfClose() {
                    listener.onHeaders(Metadata())
                    listener.onMessage(EchoResponse.newBuilder().setMessage("hi").build() as Res)
                    listener.onClose(Status.OK, Metadata())
                }

                override fun sendMessage(message: Req) = Unit
            }
    }

    private fun echo(principal: String): String? {
        val vcs = FakeVcs()
        val interceptor = VcsPrincipalInterceptor(VcsProperties(principal))
        EchoServiceGrpc
            .newBlockingStub(ClientInterceptors.intercept(vcs, interceptor))
            .echo(EchoRequest.newBuilder().setMessage("hi").build())
        return vcs.authorization
    }

    @Test
    fun `a call names the server's principal`() {
        assertThat(echo("server")).isEqualTo("Bearer server")
    }

    @Test
    fun `a call without a principal carries no authorization`() {
        assertThat(echo("")).isNull()
    }
}
