package dev.nca.jjforge.repos

import org.springframework.modulith.ApplicationModule
import org.springframework.modulith.PackageInfo

/**
 * The `repos` module: the repositories of each organization.
 * ModulesTest fails on a dependency on another module until
 * allowedDependencies names it.
 */
@PackageInfo
@ApplicationModule(allowedDependencies = [])
interface ModuleMetadata
