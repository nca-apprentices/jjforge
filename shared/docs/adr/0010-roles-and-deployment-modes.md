# 0010. Both binaries run as roles, and the chart picks a mode

Status: accepted, 2026-10-03. Deciders: jjforge maintainers.

## Context

jjforge must run on one machine for evaluation and across many nodes in a
cluster, without two code paths. Loki, Mimir, and Tempo solve this with one
binary that runs any subset of its components, and a chart that picks how many
of each.

With the object store as the only source of truth, as
[ADR 0006](0006-object-store-is-the-source-of-truth.md) decides, every
replica of vcs and the server is interchangeable. The backing services need
backups, upgrades, and capacity planning, which the infra repository already
does for the shared cluster. Self-hosters want one command to try jjforge.

## Decision

- vcs selects its roles with `--target`: `sync`, `source`, `writer`,
  `indexer`, or `all`.
- The server selects its roles with Spring profiles: `api`, `projector`,
  `worker`, or `all`.
- One chart offers three modes:
  1. `monolithic`: one Deployment of each binary with every role.
  2. `simple-scalable`: read, write, and background Deployments of each.
  3. `microservices`: one Deployment per role, each scaled on its own.
- Work that needs a single owner, such as a projector's partition or a
  repository's landing queue, takes a lease from `kernel/v1`. Losing a node
  costs only a lease takeover and cold caches.
- The object store is addressed by a URL. `s3://` is used wherever jjforge
  runs with its twins, and `file://` runs a binary on its own, without
  containers.
- The infra repository runs SeaweedFS and a Postgres for read models in the
  cluster. That Postgres needs no backups. The chart takes only their connection
  configuration and installs neither.
- For development and evaluation, `shared/deploy/compose.yaml` runs both
  binaries with the twins of [ADR 0018](0018-external-systems-run-as-twins.md),
  such as SeaweedFS and Postgres.
- No cache service until a requirement needs one, measured. Every replica
  caches immutable objects in memory and on local disk.
