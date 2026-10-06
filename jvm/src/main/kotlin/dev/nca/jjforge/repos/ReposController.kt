package dev.nca.jjforge.repos

import dev.nca.jjforge.api.ReposApi
import org.springframework.web.bind.annotation.RestController

/** Serves the `repos` operations. Each answers 501 until it is built, as ADR 0003 decides. */
@RestController
class ReposController : ReposApi
