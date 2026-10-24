package dev.nca.jjforge.vcs

import org.springframework.modulith.ApplicationModule
import org.springframework.modulith.PackageInfo

/**
 * The `vcs` module: the principal that every call to vcs names, until
 * sign-in exists, as #6 plans. Other modules call vcs
 * through their own clients and never depend on this one.
 * ModulesTest fails on a dependency on another module until
 * allowedDependencies names it.
 */
@PackageInfo
@ApplicationModule(allowedDependencies = [])
interface ModuleMetadata
