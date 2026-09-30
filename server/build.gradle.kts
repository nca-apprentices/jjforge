import org.springframework.boot.gradle.plugin.SpringBootPlugin

plugins {
    kotlin("jvm") version "2.4.10"
    kotlin("plugin.spring") version "2.4.10"
    id("org.springframework.boot") version "4.1.1"
    id("com.google.protobuf") version "0.10.0"
    id("org.openapi.generator") version "7.25.0"
}

group = "org.nca"
version = "0.1.0"
description = "jjforge server, reduced to an echo endpoint"

val protobufVersion = "4.36.0"
val grpcVersion = "1.83.1"

repositories {
    mavenCentral()
}

dependencies {
    implementation(platform(SpringBootPlugin.BOM_COORDINATES))
    implementation("org.springframework.boot:spring-boot-starter-web")
    implementation("org.springframework.boot:spring-boot-starter-actuator")
    implementation("org.jetbrains.kotlin:kotlin-reflect")
    implementation("com.fasterxml.jackson.module:jackson-module-kotlin")

    implementation("org.springframework.boot:spring-boot-starter-grpc-client")
    // The runtime must be at least the protoc that generated the stubs, and the
    // Spring Boot BOM alone pins an older one.
    implementation("com.google.protobuf:protobuf-java:$protobufVersion")

    testImplementation("org.springframework.boot:spring-boot-starter-test")
    testImplementation("org.jetbrains.kotlin:kotlin-test-junit5")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

kotlin {
    jvmToolchain(25)
    compilerOptions {
        freeCompilerArgs.add("-Xjsr305=strict")
    }
}

// The .proto files under /proto are the contract. Stubs are generated, never
// checked in.
sourceSets.main {
    proto {
        srcDir("../proto")
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

// /openapi.yaml is the REST contract. Every controller implements an interface
// generated from it, and ControllerContractTest enforces that.
val openApiOutput = layout.buildDirectory.dir("generated/openapi")

openApiGenerate {
    generatorName = "kotlin-spring"
    inputSpec = layout.projectDirectory.file("../openapi.yaml")
    outputDir = openApiOutput
    apiPackage = "org.nca.jjforge.api"
    modelPackage = "org.nca.jjforge.api.model"
    globalProperties = mapOf("apis" to "", "models" to "")
    configOptions =
        mapOf(
            "interfaceOnly" to "true",
            "skipDefaultInterface" to "true",
            "useSpringBoot3" to "true",
            "useTags" to "true",
            "documentationProvider" to "none",
            "annotationLibrary" to "none",
            "useBeanValidation" to "false",
        )
}

sourceSets.main {
    kotlin.srcDir(openApiOutput.map { it.dir("src/main/kotlin") })
}

tasks.compileKotlin {
    dependsOn(tasks.openApiGenerate)
}

tasks.test {
    useJUnitPlatform()
}

// Only the executable jar ships.
tasks.jar {
    enabled = false
}
