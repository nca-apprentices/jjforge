package dev.nca.jjforge.telemetry

import org.springframework.modulith.ApplicationModule
import org.springframework.modulith.PackageInfo

/**
 * The `telemetry` module: the trace of every request, as ADR 0005 decides.
 * ModulesTest fails on a dependency on another module until
 * allowedDependencies names it.
 */
@PackageInfo
@ApplicationModule(allowedDependencies = [])
interface ModuleMetadata
