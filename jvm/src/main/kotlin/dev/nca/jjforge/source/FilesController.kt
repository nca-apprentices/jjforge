package dev.nca.jjforge.source

import dev.nca.jjforge.api.FilesApi
import org.springframework.web.bind.annotation.RestController

/** Serves the `files` operations. Each answers 501 until it is built, as ADR 0006 decides. */
@RestController
class FilesController : FilesApi
