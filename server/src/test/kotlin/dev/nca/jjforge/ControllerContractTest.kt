package dev.nca.jjforge

import org.junit.jupiter.api.Test
import org.springframework.context.annotation.ClassPathScanningCandidateComponentProvider
import org.springframework.core.annotation.MergedAnnotations
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
 * a route outside the interfaces is a route outside the contract.
 */
class ControllerContractTest {
    @Test
    fun `every controller implements the OpenAPI contract`() {
        val scanner = ClassPathScanningCandidateComponentProvider(false)
        scanner.addIncludeFilter(AnnotationTypeFilter(RestController::class.java))

        val controllers = scanner.findCandidateComponents(APP_PACKAGE).map { Class.forName(it.beanClassName) }
        val violations = controllers.flatMap(::violations)

        assertEquals(emptyList(), violations)
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
