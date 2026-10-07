package dev.nca.jjforge.identity.web

import dev.nca.jjforge.api.AuthApi
import org.springframework.web.bind.annotation.RestController

/** Serves the `auth` operations. Each answers 501 until it is built, as ADR 0003 decides. */
@RestController
class AuthController : AuthApi
