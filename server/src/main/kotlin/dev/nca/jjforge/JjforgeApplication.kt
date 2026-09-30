package dev.nca.jjforge

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication

@SpringBootApplication
class JjforgeApplication

fun main(args: Array<String>) {
    runApplication<JjforgeApplication>(*args)
}
