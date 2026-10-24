package dev.nca.jjforge.vcs.client

import io.grpc.CallOptions
import io.grpc.Channel
import io.grpc.ClientCall
import io.grpc.ClientInterceptor
import io.grpc.ForwardingClientCall
import io.grpc.Metadata
import io.grpc.MethodDescriptor
import org.springframework.grpc.client.GlobalClientInterceptor
import org.springframework.stereotype.Component

/**
 * Names the server's principal on every call to vcs, which authenticates no
 * one until sign-in exists, as #6 plans.
 */
@Component
@GlobalClientInterceptor
class VcsPrincipalInterceptor(
    properties: VcsProperties,
) : ClientInterceptor {
    private val principal = properties.principal.takeIf { it.isNotBlank() }

    override fun <Req, Res> interceptCall(
        method: MethodDescriptor<Req, Res>,
        options: CallOptions,
        next: Channel,
    ): ClientCall<Req, Res> {
        val call = next.newCall(method, options)
        val principal = principal ?: return call
        return object : ForwardingClientCall.SimpleForwardingClientCall<Req, Res>(call) {
            override fun start(
                listener: Listener<Res>,
                headers: Metadata,
            ) {
                headers.put(AUTHORIZATION, "Bearer $principal")
                super.start(listener, headers)
            }
        }
    }

    private companion object {
        val AUTHORIZATION: Metadata.Key<String> =
            Metadata.Key.of(
                "authorization",
                Metadata.ASCII_STRING_MARSHALLER,
            )
    }
}
