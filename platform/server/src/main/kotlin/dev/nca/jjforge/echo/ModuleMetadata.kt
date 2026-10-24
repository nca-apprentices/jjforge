package dev.nca.jjforge.echo

import org.springframework.modulith.ApplicationModule
import org.springframework.modulith.PackageInfo

/**
 * The `echo` module: the echo through vcs.
 * ModulesTest fails on a dependency on another module until
 * allowedDependencies names it.
 */
@PackageInfo
@ApplicationModule(allowedDependencies = [])
interface ModuleMetadata
