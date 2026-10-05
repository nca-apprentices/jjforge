package dev.nca.jjforge.identity

import dev.nca.jjforge.api.AuthApi
import org.springframework.web.bind.annotation.RestController

/** Serves the `auth` operations. Each answers 501 until it is built, as ADR 0006 decides. */
@RestController
class AuthController : AuthApi
