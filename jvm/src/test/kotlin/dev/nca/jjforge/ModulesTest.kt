package dev.nca.jjforge

import com.tngtech.archunit.core.domain.JavaClass
import org.junit.jupiter.api.Test
import org.springframework.modulith.core.ApplicationModules
import java.nio.file.Files
import java.nio.file.Path
import kotlin.test.fail

/**
 * Each direct subpackage is a module, as ADR 0002 decides. The code generated
 * from the contracts, the REST interfaces and the gRPC stubs, belongs to no
 * module.
 */
val serverModules: ApplicationModules =
    ApplicationModules.of(
        JjforgeApplication::class.java,
        JavaClass.Predicates.resideInAnyPackage("dev.nca.jjforge.api..", "dev.nca.jjforge.*.v1.."),
    )

/**
 * A module uses another only through its top-level package, never its
 * internals, and only when its ModuleMetadata allows it. The module page in
 * the docs shows the graph that results.
 */
class ModulesTest {
    @Test
    fun `modules respect their boundaries`() {
        serverModules.verify()
    }

    /**
     * Writes the page when it differs from the code and fails, so the commit
     * that changes a dependency also changes the page.
     */
    @Test
    fun `the module page shows every dependency`() {
        val page = Path.of("../shared/docs/modules.md")
        val expected = modulePage()

        if (Files.exists(page) && Files.readString(page) == expected) {
            return
        }
        Files.writeString(page, expected)
        fail("$page was stale and is now rewritten. Commit it.")
    }

    private fun modulePage(): String {
        val modules = serverModules.map { it.identifier.toString() }.sorted()
        val edges =
            serverModules
                .flatMap { module ->
                    module.getDirectDependencies(serverModules).uniqueModules().toList().map {
                        "${module.identifier} --> ${it.identifier}"
                    }
                }.sorted()

        return buildString {
            appendLine("# Server modules")
            appendLine()
            appendLine("The modules of the server, as `ModulesTest` reads them from the code. An")
            appendLine("arrow points from a module to a module it depends on. Each module's")
            appendLine("`ModuleMetadata` allows its dependencies, and `ModulesTest` rewrites this page")
            appendLine("when they change.")
            appendLine()
            appendLine("```mermaid")
            appendLine("flowchart LR")
            (modules + edges).forEach { appendLine("    $it") }
            appendLine("```")
        }
    }
}
