package dev.nca.jjforge.vcs.client

import dev.nca.jjforge.echo.v1.EchoRequest
import dev.nca.jjforge.echo.v1.EchoResponse
import dev.nca.jjforge.echo.v1.EchoServiceGrpc
import dev.nca.jjforge.source.v1.GetChallengeResponse
import dev.nca.jjforge.source.v1.OpenSessionResponse
import dev.nca.jjforge.source.v1.SessionServiceGrpc
import com.google.protobuf.ByteString
import io.grpc.CallOptions
import io.grpc.Channel
import io.grpc.ClientCall
import io.grpc.ClientInterceptors
import io.grpc.Metadata
import io.grpc.MethodDescriptor
import io.grpc.Status
import io.grpc.StatusRuntimeException
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Files
import java.nio.file.Path
import java.time.Instant

/** The server opens a new session when vcs refuses the one it holds, such as after vcs restarts. */
class VcsSessionInterceptorTest {
    @TempDir
    lateinit var dir: Path

    /** Answers SessionService, and refuses the first echo as vcs refuses a session it can't open. */
    private class FakeVcs : Channel() {
        var sessions = 0
        var echoes = 0

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
                }

                override fun request(numMessages: Int) = Unit

                override fun cancel(
                    message: String?,
                    cause: Throwable?,
                ) = Unit

                override fun halfClose() {
                    listener.onHeaders(Metadata())
                    when (method.fullMethodName) {
                        SessionServiceGrpc.getGetChallengeMethod().fullMethodName -> {
                            val nonce = ByteString.copyFrom(ByteArray(NONCE))
                            answer(GetChallengeResponse.newBuilder().setNonce(nonce).build())
                        }
                        SessionServiceGrpc.getOpenSessionMethod().fullMethodName -> {
                            sessions++
                            val ends = Instant.now().plusSeconds(SESSION).epochSecond
                            answer(OpenSessionResponse.newBuilder().setToken("s$sessions").setExpiresAt(ends).build())
                        }
                        else ->
                            if (echoes++ == 0) {
                                listener.onClose(Status.UNAUTHENTICATED, Metadata())
                            } else {
                                answer(EchoResponse.newBuilder().setMessage("hi").build())
                            }
                    }
                }

                @Suppress("UNCHECKED_CAST")
                private fun answer(message: Any) {
                    listener.onMessage(message as Res)
                    listener.onClose(Status.OK, Metadata())
                }

                override fun sendMessage(message: Req) = Unit
            }
    }

    private companion object {
        const val NONCE = 56
        const val SESSION = 900L
    }

    @Test
    fun `a refused session is opened again on the next call`() {
        ProcessBuilder("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", "server")
            .directory(dir.toFile())
            .start()
            .waitFor()
        Files.writeString(dir.resolve("server-cert.pub"), "certificate")
        val vcs = FakeVcs()
        val interceptor = VcsSessionInterceptor(VcsCredentialProperties(dir.resolve("server").toString()))
        val stub = EchoServiceGrpc.newBlockingStub(ClientInterceptors.intercept(vcs, interceptor))
        val echo = EchoRequest.newBuilder().setMessage("hi").build()

        assertThrows<StatusRuntimeException> { stub.echo(echo) }
        val answer = stub.echo(echo).message

        assertThat(answer).isEqualTo("hi")
        assertThat(vcs.sessions).isEqualTo(2)
    }
}
