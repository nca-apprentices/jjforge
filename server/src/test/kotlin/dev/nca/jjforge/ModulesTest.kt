package dev.nca.jjforge

import com.tngtech.archunit.core.domain.JavaClass
import org.junit.jupiter.api.Test
import org.springframework.modulith.core.ApplicationModules

/**
 * Each direct subpackage is a module, as ADR 0005 decides. A module uses
 * another only through its top-level package, never its internals. The code
 * generated from the contracts belongs to no module.
 */
class ModulesTest {
    @Test
    fun `modules respect their boundaries`() {
        ApplicationModules
            .of(JjforgeApplication::class.java, JavaClass.Predicates.resideInAPackage("dev.nca.jjforge.api.."))
            .verify()
    }
}
