# 0006. Objects live in SeaweedFS, and vcsd stays stateless

Status: superseded by [ADR 0018](0018-object-store-is-the-source-of-truth.md), 2026-10-03. Deciders: jjforge maintainers.

## Context

A repository holds many immutable objects and one small, often-changing
pointer: the operation head. Local disks tie a repository to one node and make
scaling out a migration.

Options: local disks per vcsd node, a shared file system, or an object store
plus a database for the heads.

## Decision

- Objects are stored in git format in SeaweedFS, keyed by content hash.
- Operation heads live in Postgres, so a head moves in a transaction.
- vcsd is stateless. Any vcsd replica serves any repository.
- Calls to vcsd carry the organization and repository IDs for routing.
