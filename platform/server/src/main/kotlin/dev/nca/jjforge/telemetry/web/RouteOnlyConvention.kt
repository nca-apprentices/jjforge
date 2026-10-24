package dev.nca.jjforge.telemetry.web

import io.micrometer.common.KeyValues
import org.springframework.http.server.observation.DefaultServerRequestObservationConvention
import org.springframework.http.server.observation.ServerRequestObservationContext
import org.springframework.stereotype.Component

/**
 * Leaves the path a caller sent off every request's span, so the names of an
 * organization and a repository in it, which may name a person, never reach a
 * trace, as ADR 0005 decides. The span keeps its route's template in `uri`.
 */
@Component
class RouteOnlyConvention : DefaultServerRequestObservationConvention() {
    override fun getHighCardinalityKeyValues(context: ServerRequestObservationContext): KeyValues =
        KeyValues.empty()
}
