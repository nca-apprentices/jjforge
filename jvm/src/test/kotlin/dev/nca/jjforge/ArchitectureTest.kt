package dev.nca.jjforge

import com.tngtech.archunit.core.importer.ImportOption
import com.tngtech.archunit.junit.AnalyzeClasses
import com.tngtech.archunit.junit.ArchTest
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.classes
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.fields
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.noClasses
import com.tngtech.archunit.library.GeneralCodingRules
import org.springframework.beans.factory.annotation.Value
import org.springframework.boot.context.properties.ConfigurationProperties
import org.springframework.stereotype.Component
import org.springframework.validation.annotation.Validated

/**
 * The server's code rules that the compiler doesn't check, as ADR 0003 and
 * ADR 0005 decide. ControllerContractTest holds the rules for controllers.
 */
@AnalyzeClasses(packages = ["dev.nca.jjforge"], importOptions = [ImportOption.DoNotIncludeTests::class])
class ArchitectureTest {
    @ArchTest
    val settingsComeFromProperties =
        noClasses()
            .should()
            .dependOnClassesThat()
            .areAssignableTo(Value::class.java)
            .because("a setting is a field of a validated @ConfigurationProperties class, not an @Value")

    @ArchTest
    val propertiesAreValidated =
        classes()
            .that()
            .areAnnotatedWith(ConfigurationProperties::class.java)
            .should()
            .haveSimpleNameEndingWith("Properties")
            .andShould()
            .beAnnotatedWith(Validated::class.java)
            .because("an invalid setting must stop the server at startup")

    @ArchTest
    val componentsUseConstructorInjection =
        fields()
            .that()
            .areDeclaredInClassesThat()
            .areMetaAnnotatedWith(Component::class.java)
            .should()
            .beFinal()
            .because("a component receives its dependencies through its constructor, never through a field or lateinit")

    // A line written past SLF4J reaches no log store as JSON and carries no
    // trace.
    @ArchTest
    val logsGoThroughSlf4j = GeneralCodingRules.NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS

    @ArchTest
    val logsSkipJavaUtilLogging = GeneralCodingRules.NO_CLASSES_SHOULD_USE_JAVA_UTIL_LOGGING

    @ArchTest
    val tracingIsCrossCutting =
        noClasses()
            .that()
            .resideOutsideOfPackage("dev.nca.jjforge.telemetry..")
            .should()
            .dependOnClassesThat()
            .resideInAnyPackage("io.micrometer.tracing..", "io.opentelemetry..")
            .because("every request is traced by the telemetry module, so no other code touches a trace")
}
