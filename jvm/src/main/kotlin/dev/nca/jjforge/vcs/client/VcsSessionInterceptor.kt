package dev.nca.jjforge.vcs.client

import dev.nca.jjforge.source.v1.GetChallengeRequest
import dev.nca.jjforge.source.v1.OpenSessionRequest
import dev.nca.jjforge.source.v1.SessionServiceGrpc
import io.grpc.CallOptions
import io.grpc.Channel
import io.grpc.ClientCall
import io.grpc.ClientInterceptor
import io.grpc.ForwardingClientCall
import io.grpc.Metadata
import io.grpc.MethodDescriptor
import org.springframework.grpc.client.GlobalClientInterceptor
import org.springframework.stereotype.Component
import java.nio.file.Files
import java.nio.file.Path
import java.time.Instant
import java.util.concurrent.atomic.AtomicReference

/**
 * Adds the server's session to every call to vcs, opened with the server's
 * certificate through `SessionService` and opened again shortly before it
 * ends. The calls of `SessionService` itself go without one.
 */
@Component
@GlobalClientInterceptor
class VcsSessionInterceptor(
    properties: VcsCredentialProperties,
) : ClientInterceptor {
    private val key = properties.key.takeIf { it.isNotBlank() }?.let(Path::of)
    private val signer = key?.let(SshSigner::read)
    private val session = AtomicReference<Session?>(null)

    private data class Session(
        val token: String,
        val endsAt: Instant,
    )

    override fun <Req, Res> interceptCall(
        method: MethodDescriptor<Req, Res>,
        options: CallOptions,
        next: Channel,
    ): ClientCall<Req, Res> {
        val call = next.newCall(method, options)
        if (signer == null || method.serviceName == SessionServiceGrpc.SERVICE_NAME) {
            return call
        }
        val token = current(next)
        return object : ForwardingClientCall.SimpleForwardingClientCall<Req, Res>(call) {
            override fun start(
                listener: Listener<Res>,
                headers: Metadata,
            ) {
                headers.put(AUTHORIZATION, "Bearer $token")
                super.start(listener, headers)
            }
        }
    }

    private fun current(channel: Channel): String {
        val now = Instant.now()
        session.get()?.takeIf { it.endsAt.isAfter(now.plusSeconds(RENEW_BEFORE)) }?.let { return it.token }

        val stub = SessionServiceGrpc.newBlockingStub(channel)
        val nonce = stub.getChallenge(GetChallengeRequest.getDefaultInstance()).nonce
        val certificate = Files.readString(Path.of(key.toString().removeSuffix(".pub") + "-cert.pub"))
        val signed = "$AUDIENCE\n".toByteArray() + nonce.toByteArray()
        val opened =
            stub.openSession(
                OpenSessionRequest
                    .newBuilder()
                    .setCertificate(certificate)
                    .setNonce(nonce)
                    .setSignature(checkNotNull(signer).sign(signed, NAMESPACE))
                    .build(),
            )
        session.set(Session(opened.token, Instant.ofEpochSecond(opened.expiresAt)))
        return opened.token
    }

    private companion object {
        const val NAMESPACE = "jjforge-session"

        // vcs accepts the server's signature for this audience only, so no
        // signature the server makes opens a session at `POST /sync/v1/session`.
        const val AUDIENCE = "grpc:source/v1"
        const val RENEW_BEFORE = 30L
        val AUTHORIZATION: Metadata.Key<String> = Metadata.Key.of("authorization", Metadata.ASCII_STRING_MARSHALLER)
    }
}
