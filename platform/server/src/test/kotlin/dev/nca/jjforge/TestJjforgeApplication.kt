package dev.nca.jjforge

import org.springframework.boot.fromApplication
import org.springframework.boot.with

/** Starts the server with Postgres in a container, through `gradle bootTestRun`. */
fun main(args: Array<String>) {
    fromApplication<JjforgeApplication>().with(TestcontainersConfiguration::class).run(*args)
}
