package dev.nca.jjforge.echo

import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.springframework.boot.autoconfigure.AutoConfigurations
import org.springframework.boot.context.properties.EnableConfigurationProperties
import org.springframework.boot.test.context.runner.ApplicationContextRunner
import org.springframework.boot.validation.autoconfigure.ValidationAutoConfiguration

/** A missing or blank vcsd target stops the server at startup, not at the first request. */
class VcsdPropertiesTest {
    @EnableConfigurationProperties(VcsdProperties::class)
    private class Config

    private val runner =
        ApplicationContextRunner()
            .withConfiguration(AutoConfigurations.of(ValidationAutoConfiguration::class.java))
            .withUserConfiguration(Config::class.java)

    @Test
    fun `binds a target`() {
        runner.withPropertyValues("jjforge.vcsd.target=vcsd:50052").run { context ->
            assertThat(context.getBean(VcsdProperties::class.java).target).isEqualTo("vcsd:50052")
        }
    }

    @Test
    fun `refuses a blank target`() {
        runner.withPropertyValues("jjforge.vcsd.target= ").run { context ->
            assertThat(context).hasFailed()
        }
    }
}
