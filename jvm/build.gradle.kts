import org.springframework.boot.gradle.plugin.SpringBootPlugin

plugins {
    kotlin("jvm") version "2.4.20"
    kotlin("plugin.spring") version "2.4.20"
    id("org.springframework.boot") version "4.1.1"
    id("com.google.protobuf") version "0.10.0"
    id("org.openapi.generator") version "7.25.0"
    id("dev.detekt") version "2.0.0-alpha.6"
    id("org.jetbrains.kotlinx.kover") version "0.9.11"
}

group = "dev.nca"
version = "0.1.0"
description = "jjforge server, reduced to an echo endpoint"

val protobufVersion = "4.36.2"
val grpcVersion = "1.84.0"

repositories {
    mavenCentral()
}

dependencies {
    implementation(platform(SpringBootPlugin.BOM_COORDINATES))
    implementation(platform("org.springframework.modulith:spring-modulith-bom:2.1.1"))
    implementation("org.springframework.boot:spring-boot-starter-web")
    implementation("org.springframework.boot:spring-boot-starter-actuator")
    runtimeOnly("io.micrometer:micrometer-registry-prometheus")
    // Traces over OTLP, as ADR 0005 decides. Metrics stay with the scrape.
    implementation("org.springframework.boot:spring-boot-starter-opentelemetry")
    implementation("org.springframework.boot:spring-boot-starter-validation")
    implementation("org.jetbrains.kotlin:kotlin-reflect")
    implementation("com.fasterxml.jackson.module:jackson-module-kotlin")

    implementation("org.springframework.modulith:spring-modulith-starter-core")
    implementation("org.springframework.modulith:spring-modulith-starter-jdbc")
    // Runs each module's migrations from its own folder.
    runtimeOnly("org.springframework.modulith:spring-modulith-runtime")
    implementation("org.springframework.boot:spring-boot-starter-flyway")
    runtimeOnly("org.flywaydb:flyway-database-postgresql")
    runtimeOnly("org.postgresql:postgresql")

    implementation("org.springframework.boot:spring-boot-starter-grpc-client")
    // The runtime must be at least the protoc that generated the stubs, and the
    // Spring Boot BOM alone pins an older one.
    implementation("com.google.protobuf:protobuf-java:$protobufVersion")

    testImplementation("org.springframework.boot:spring-boot-starter-test")
    testImplementation("org.jetbrains.kotlin:kotlin-test-junit5")
    testImplementation("org.springframework.modulith:spring-modulith-starter-test")
    testImplementation("com.tngtech.archunit:archunit-junit5:1.5.1")
    testImplementation("org.springframework.boot:spring-boot-testcontainers")
    testImplementation("org.testcontainers:testcontainers-postgresql")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

kotlin {
    jvmToolchain(25)
    compilerOptions {
        freeCompilerArgs.add("-Xjsr305=strict")
        allWarningsAsErrors = true
        // A controller inherits the generated interfaces' default methods as
        // they are. The compatibility modes copy each one, with its route, into
        // the controller.
        jvmDefault = org.jetbrains.kotlin.gradle.dsl.JvmDefaultMode.NO_COMPATIBILITY
    }
}

// The .proto files under /shared/proto are the contract. Stubs are generated,
// never checked in.
sourceSets.main {
    proto {
        srcDir("../shared/proto")
    }
}

protobuf {
    protoc {
        artifact = "com.google.protobuf:protoc:$protobufVersion"
    }
    plugins {
        create("grpc") {
            artifact = "io.grpc:protoc-gen-grpc-java:$grpcVersion"
        }
    }
    generateProtoTasks {
        all().configureEach {
            plugins {
                maybeCreate("grpc").option("@generated=omit")
            }
        }
    }
}

// /shared/openapi.yaml is the REST contract. Every controller implements an
// interface generated from it, and ControllerContractTest enforces that.
val openApiOutput = layout.buildDirectory.dir("generated/openapi")

openApiGenerate {
    generatorName = "kotlin-spring"
    inputSpec = layout.projectDirectory.file("../shared/openapi.yaml")
    outputDir = openApiOutput
    apiPackage = "dev.nca.jjforge.api"
    modelPackage = "dev.nca.jjforge.api.model"
    globalProperties = mapOf("apis" to "", "models" to "")
    configOptions =
        mapOf(
            "interfaceOnly" to "true",
            "skipDefaultInterface" to "false",
            "useSpringBoot3" to "true",
            "useTags" to "true",
            "documentationProvider" to "none",
            "annotationLibrary" to "none",
            "useBeanValidation" to "false",
            // Kotlin style, and it keeps a value such as `context`, a soft keyword,
            // from being read as Kotlin syntax.
            "enumPropertyNaming" to "UPPERCASE",
        )
}

sourceSets.main {
    kotlin.srcDir(openApiOutput.map { it.dir("src/main/kotlin") })
}

tasks.compileKotlin {
    dependsOn(tasks.openApiGenerate)
}

// /shared/openapi.yaml is compiled from /shared/api and not committed. A build
// that starts without it, such as CodeQL's, compiles it here with the pnpm
// that /shared/api pins. The image copies it in and `mise run api:build`
// keeps a local copy fresh, so this is skipped in both cases.
val spec = layout.projectDirectory.file("../shared/openapi.yaml").asFile
val compileSpec =
    tasks.register<Exec>("compileSpec") {
        onlyIf { !spec.exists() }
        workingDir = layout.projectDirectory.dir("../shared/api").asFile
        commandLine("sh", "-c", "corepack pnpm install --frozen-lockfile && corepack pnpm exec tsp compile .")
    }

tasks.openApiGenerate {
    dependsOn(compileSpec)
}

tasks.test {
    useJUnitPlatform()
}

// Only the executable jar ships.
tasks.jar {
    enabled = false
}

// detekt checks the hand-written code. ktlint formats it, so detekt's own
// formatting rules stay off. Every finding fails the build, with no baseline.
detekt {
    buildUponDefaultConfig = true
    config.setFrom("detekt.yml")
}

// The coverage floor counts hand-written code only: the stubs generated from
// the contracts and the application class with its main function are
// excluded.
kover {
    reports {
        filters {
            excludes {
                packages(
                    "dev.nca.jjforge.api",
                    "dev.nca.jjforge.api.model",
                    "dev.nca.jjforge.echo.v1",
                    "dev.nca.jjforge.store.v1",
                    "dev.nca.jjforge.sync.v1",
                    "dev.nca.jjforge.source.v1",
                )
                classes("dev.nca.jjforge.JjforgeApplication", "dev.nca.jjforge.JjforgeApplicationKt")
            }
        }
        verify {
            rule {
                minBound(80)
            }
        }
    }
}
