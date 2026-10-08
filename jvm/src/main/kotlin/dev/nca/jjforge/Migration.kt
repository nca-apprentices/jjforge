package dev.nca.jjforge

import org.springframework.boot.ApplicationArguments
import org.springframework.boot.ApplicationRunner
import org.springframework.boot.SpringApplication
import org.springframework.context.ApplicationContext
import org.springframework.context.annotation.Profile
import org.springframework.stereotype.Component

/**
 * Flyway migrates while the context starts, so the `migrate` profile closes
 * the context once it has started and the process exits. The chart's
 * migration Job runs this before a rollout, and the server's pods leave
 * Flyway off.
 */
@Component
@Profile("migrate")
class Migration(
    private val context: ApplicationContext,
) : ApplicationRunner {
    override fun run(args: ApplicationArguments) {
        SpringApplication.exit(context)
    }
}
