package dev.nca.jjforge

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.context.properties.ConfigurationPropertiesScan
import org.springframework.boot.runApplication

@SpringBootApplication
@ConfigurationPropertiesScan
class JjforgeApplication

fun main(args: Array<String>) {
    runApplication<JjforgeApplication>(*args)
}
