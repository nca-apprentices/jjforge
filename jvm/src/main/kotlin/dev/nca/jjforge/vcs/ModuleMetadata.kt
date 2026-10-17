package dev.nca.jjforge.vcs

import org.springframework.modulith.ApplicationModule
import org.springframework.modulith.PackageInfo

/**
 * The `vcs` module: the server's own certificate, which opens the session
 * that every call to vcs carries, as ADR 0002 decides. Other modules call vcs
 * through their own clients and never depend on this one.
 * ModulesTest fails on a dependency on another module until
 * allowedDependencies names it.
 */
@PackageInfo
@ApplicationModule(allowedDependencies = [])
interface ModuleMetadata
