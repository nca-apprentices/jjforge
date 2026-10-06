package dev.nca.jjforge.source

import dev.nca.jjforge.api.HistoryApi
import org.springframework.web.bind.annotation.RestController

/** Serves the `history` operations. Each answers 501 until it is built, as ADR 0003 decides. */
@RestController
class HistoryController : HistoryApi
