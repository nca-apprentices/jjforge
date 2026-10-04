package dev.nca.jjforge

import org.junit.jupiter.api.Test
import org.springframework.beans.factory.annotation.AnnotatedBeanDefinition
import org.springframework.context.annotation.ClassPathScanningCandidateComponentProvider
import org.springframework.core.annotation.MergedAnnotations
import org.springframework.core.type.AnnotationMetadata
import org.springframework.core.type.filter.AnnotationTypeFilter
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.RestController
import kotlin.test.assertEquals

private const val APP_PACKAGE = "dev.nca.jjforge"

/**
 * The package openapi-generator writes the interfaces from /shared/openapi.yaml
 * to.
 */
private const val API_PACKAGE = "dev.nca.jjforge.api"

/**
 * Keeps the REST surface equal to /shared/openapi.yaml. A controller must
 * implement a generated interface and must not map any route of its own, since
 * a route outside the interfaces is a route outside the contract. Every
 * generated interface needs a controller, or its operations answer 404 instead
 * of 501.
 */
class ControllerContractTest {
    private val controllers = scan(APP_PACKAGE) { !it.isInterface }

    @Test
    fun `every controller implements the OpenAPI contract`() {
        val violations = controllers.flatMap(::violations)

        assertEquals(emptyList(), violations)
    }

    @Test
    fun `every generated interface has a controller`() {
        val implemented = controllers.flatMap { it.interfaces.toList() }.toSet()
        val missing = scan(API_PACKAGE) { it.isInterface }.filterNot { it in implemented }.map { it.name }

        assertEquals(emptyList(), missing)
    }

    /** Finds the classes in [pkg] that carry @RestController, which the generator puts on its interfaces too. */
    private fun scan(
        pkg: String,
        accept: (AnnotationMetadata) -> Boolean,
    ): List<Class<*>> {
        val scanner =
            object : ClassPathScanningCandidateComponentProvider(false) {
                override fun isCandidateComponent(definition: AnnotatedBeanDefinition) = accept(definition.metadata)
            }
        scanner.addIncludeFilter(AnnotationTypeFilter(RestController::class.java))

        return scanner.findCandidateComponents(pkg).map { Class.forName(it.beanClassName) }
    }

    private fun violations(controller: Class<*>): List<String> {
        val implementsContract = controller.interfaces.any { it.packageName == API_PACKAGE }
        val ownRoutes =
            (listOf(controller) + controller.declaredMethods)
                .filter { MergedAnnotations.from(it).isPresent(RequestMapping::class.java) }
                .map { "${controller.name}: $it maps a route outside /shared/openapi.yaml" }

        if (implementsContract) {
            return ownRoutes
        }

        return ownRoutes + "${controller.name} implements no interface from $API_PACKAGE"
    }
}
