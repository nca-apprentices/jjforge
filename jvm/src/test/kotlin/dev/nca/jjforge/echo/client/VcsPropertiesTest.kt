package dev.nca.jjforge.echo.client

import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.springframework.boot.autoconfigure.AutoConfigurations
import org.springframework.boot.context.properties.EnableConfigurationProperties
import org.springframework.boot.test.context.runner.ApplicationContextRunner
import org.springframework.boot.validation.autoconfigure.ValidationAutoConfiguration

/** A missing or blank vcs target stops the server at startup, not at the first request. */
class VcsPropertiesTest {
    @EnableConfigurationProperties(VcsProperties::class)
    private class Config

    private val runner =
        ApplicationContextRunner()
            .withConfiguration(AutoConfigurations.of(ValidationAutoConfiguration::class.java))
            .withUserConfiguration(Config::class.java)

    @Test
    fun `binds a target`() {
        runner.withPropertyValues("jjforge.vcs.target=vcs:50052").run { context ->
            assertThat(context.getBean(VcsProperties::class.java).target).isEqualTo("vcs:50052")
        }
    }

    @Test
    fun `refuses a blank target`() {
        runner.withPropertyValues("jjforge.vcs.target= ").run { context ->
            assertThat(context).hasFailed()
        }
    }
}
