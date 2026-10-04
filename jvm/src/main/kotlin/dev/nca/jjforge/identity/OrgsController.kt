package dev.nca.jjforge.identity

import dev.nca.jjforge.api.OrgsApi
import org.springframework.web.bind.annotation.RestController

/** Serves the `orgs` operations. Each answers 501 until it is built, as ADR 0011 decides. */
@RestController
class OrgsController : OrgsApi
