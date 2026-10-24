package dev.nca.jjforge

import com.tngtech.archunit.core.importer.ImportOption
import com.tngtech.archunit.junit.AnalyzeClasses
import com.tngtech.archunit.junit.ArchTest
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.classes
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.fields
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.methods
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.noClasses
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.noMethods
import com.tngtech.archunit.library.Architectures.layeredArchitecture
import com.tngtech.archunit.library.GeneralCodingRules
import org.springframework.beans.factory.annotation.Value
import org.springframework.boot.context.properties.ConfigurationProperties
import org.springframework.context.event.EventListener
import org.springframework.jdbc.core.JdbcTemplate
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate
import org.springframework.modulith.events.ApplicationModuleListener
import org.springframework.stereotype.Component
import org.springframework.transaction.event.TransactionalEventListener
import org.springframework.validation.annotation.Validated
import org.springframework.web.bind.annotation.RestController
import org.springframework.web.client.RestTemplate

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

    @ArchTest
    val modulesHaveLayers =
        classes()
            .should()
            .resideInAnyPackage(
                "dev.nca.jjforge",
                "dev.nca.jjforge.*",
                "dev.nca.jjforge.*.web..",
                "dev.nca.jjforge.*.application..",
                "dev.nca.jjforge.*.domain..",
                "dev.nca.jjforge.*.persistence..",
                "dev.nca.jjforge.*.client..",
                "dev.nca.jjforge.api..",
                "dev.nca.jjforge.*.v1..",
            ).because("a module is its API and the layers web, application, domain, persistence, and client")

    @ArchTest
    val layersPointInward =
        layeredArchitecture()
            .consideringOnlyDependenciesInLayers()
            .withOptionalLayers(true)
            .layer("API")
            .definedBy("dev.nca.jjforge.*")
            .layer("Web")
            .definedBy("dev.nca.jjforge.*.web..")
            .layer("Application")
            .definedBy("dev.nca.jjforge.*.application..")
            .layer("Domain")
            .definedBy("dev.nca.jjforge.*.domain..")
            .layer("Persistence")
            .definedBy("dev.nca.jjforge.*.persistence..")
            .layer("Client")
            .definedBy("dev.nca.jjforge.*.client..")
            .whereLayer("Web")
            .mayNotBeAccessedByAnyLayer()
            .whereLayer("Application")
            .mayOnlyBeAccessedByLayers("Web")
            .whereLayer("Persistence")
            .mayOnlyBeAccessedByLayers("Application")
            .whereLayer("Client")
            .mayOnlyBeAccessedByLayers("Application")
            .whereLayer("Domain")
            .mayOnlyBeAccessedByLayers("Web", "Application", "Persistence", "Client")
            .because("a layer uses only the layers below it, and the API uses none")

    @ArchTest
    val domainIsPlain =
        noClasses()
            .that()
            .resideInAPackage("dev.nca.jjforge.*.domain..")
            .should()
            .dependOnClassesThat()
            .resideInAnyPackage(
                "org.springframework..",
                "jakarta..",
                "java.sql..",
                "io.grpc..",
                "com.google.protobuf..",
                "dev.nca.jjforge.api..",
                "dev.nca.jjforge.*.v1..",
            ).because("the domain holds the rules, and the other layers hold the frameworks")
            .allowEmptyShould(true)

    @ArchTest
    val apiHoldsNoComponent =
        noClasses()
            .that()
            .resideInAPackage("dev.nca.jjforge.*")
            .and()
            .resideOutsideOfPackage("dev.nca.jjforge.api")
            .should()
            .beMetaAnnotatedWith(Component::class.java)
            .because("a module's top-level package is its API, and its components belong in a layer")

    @ArchTest
    val controllersLiveInWeb =
        classes()
            .that()
            .areAnnotatedWith(RestController::class.java)
            .and()
            .areNotInterfaces()
            .should()
            .resideInAPackage("dev.nca.jjforge.*.web..")
            .because("a controller serves HTTP, and no other layer may call it")

    @ArchTest
    val contractStaysInWeb =
        noClasses()
            .that()
            .resideOutsideOfPackages("dev.nca.jjforge.*.web..", "dev.nca.jjforge.api..")
            .should()
            .dependOnClassesThat()
            .resideInAPackage("dev.nca.jjforge.api..")
            .because("web maps the REST contract's models to the application's types")

    @ArchTest
    val stubsStayInClient =
        noClasses()
            .that()
            .resideOutsideOfPackages("dev.nca.jjforge.*.client..", "dev.nca.jjforge.*.v1..")
            .should()
            .dependOnClassesThat()
            .resideInAPackage("dev.nca.jjforge.*.v1..")
            .because("client maps the gRPC messages to the application's types")

    @ArchTest
    val repositoriesLiveInPersistence =
        classes()
            .that()
            .areAssignableTo("org.springframework.data.repository.Repository")
            .should()
            .resideInAPackage("dev.nca.jjforge.*.persistence..")
            .because("persistence owns the module's tables")
            .allowEmptyShould(true)

    @ArchTest
    val listenersLiveInApplication =
        methods()
            .that()
            .areAnnotatedWith(ApplicationModuleListener::class.java)
            .should()
            .beDeclaredInClassesThat()
            .resideInAPackage("dev.nca.jjforge.*.application..")
            .because("a listener runs a use case in its own transaction")
            .allowEmptyShould(true)

    @ArchTest
    val modulesListenWithModulith =
        noMethods()
            .should()
            .beAnnotatedWith(EventListener::class.java)
            .orShould()
            .beAnnotatedWith(TransactionalEventListener::class.java)
            .because("@ApplicationModuleListener runs after the commit, and the registry retries it")

    @ArchTest
    val modernClients =
        noClasses()
            .should()
            .dependOnClassesThat()
            .areAssignableTo(RestTemplate::class.java)
            .orShould()
            .dependOnClassesThat()
            .areAssignableTo(JdbcTemplate::class.java)
            .orShould()
            .dependOnClassesThat()
            .areAssignableTo(NamedParameterJdbcTemplate::class.java)
            .because("RestClient and JdbcClient replace the template classes")

    @ArchTest
    val timeIsJavaTime =
        noClasses()
            .should()
            .dependOnClassesThat()
            .belongToAnyOf(java.util.Date::class.java, java.util.Calendar::class.java, java.sql.Timestamp::class.java)
            .because("java.time holds every instant and date")

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
