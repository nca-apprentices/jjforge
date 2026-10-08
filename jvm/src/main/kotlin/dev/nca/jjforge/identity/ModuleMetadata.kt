package dev.nca.jjforge.identity

import org.springframework.modulith.ApplicationModule
import org.springframework.modulith.PackageInfo

/**
 * The `identity` module: organizations, sign-in, and policy.
 * ModulesTest fails on a dependency on another module until
 * allowedDependencies names it.
 */
@PackageInfo
@ApplicationModule(allowedDependencies = [])
interface ModuleMetadata
