package dev.nca.jjforge

import com.tngtech.archunit.core.importer.ImportOption
import com.tngtech.archunit.junit.AnalyzeClasses
import com.tngtech.archunit.junit.ArchTest
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.classes
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.fields
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.noClasses
import com.tngtech.archunit.lang.syntax.ArchRuleDefinition.noMethods
import org.springframework.beans.factory.annotation.Value
import org.springframework.boot.context.properties.ConfigurationProperties
import org.springframework.context.event.EventListener
import org.springframework.jdbc.core.JdbcTemplate
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate
import org.springframework.stereotype.Component
import org.springframework.transaction.event.TransactionalEventListener
import org.springframework.validation.annotation.Validated
import org.springframework.web.bind.annotation.RestController
import org.springframework.web.client.RestTemplate

/**
 * The server's code rules that the compiler doesn't check, as ADR 0003
 * decides. ControllerContractTest holds the rules for controllers.
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
    val modulesHaveWebAndInternalOnly =
        classes()
            .should()
            .resideInAnyPackage(
                "dev.nca.jjforge",
                "dev.nca.jjforge.*",
                "dev.nca.jjforge.*.web",
                "dev.nca.jjforge.*.internal..",
                "dev.nca.jjforge.api..",
                "dev.nca.jjforge.*.v1..",
            ).because("a module is its API, its web package, and its internal package")

    @ArchTest
    val apiHoldsNoComponent =
        noClasses()
            .that()
            .resideInAPackage("dev.nca.jjforge.*")
            .and()
            .resideOutsideOfPackage("dev.nca.jjforge.api")
            .should()
            .beMetaAnnotatedWith(Component::class.java)
            .because("a module's top-level package is its API, and its components belong in web or internal")

    @ArchTest
    val controllersLiveInWeb =
        classes()
            .that()
            .areAnnotatedWith(RestController::class.java)
            .and()
            .areNotInterfaces()
            .should()
            .resideInAPackage("dev.nca.jjforge.*.web")
            .because("a controller serves HTTP, and no other module may call it")

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
}
