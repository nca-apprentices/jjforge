# 0021. Both binaries run as roles, and the chart picks a mode

Status: accepted, 2026-10-03. Deciders: jjforge maintainers. Supersedes
[ADR 0010](0010-backing-services-in-infra.md).

## Context

jjforge must run on one machine for evaluation and across many nodes in a
cluster, without two code paths. Loki, Mimir, and Tempo solve this with one
binary that runs any subset of its components, and a chart that picks how many
of each.

With the object store as the only source of truth, as
[ADR 0018](0018-object-store-is-the-source-of-truth.md) decides, every
replica of vcsd and the server is interchangeable. ADR 0010 had the infra
repository run Postgres, Redpanda, and SeaweedFS. Redpanda is no longer used,
and Postgres holds only read models that can be rebuilt.

## Decision

- vcsd selects its roles with `--target`: `sync`, `source`, `writer`,
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
- The object store is addressed by a URL. `s3://` is used in a cluster, and
  `file://` runs the evaluation tier without SeaweedFS.
- The infra repository runs SeaweedFS and a Postgres for read models in the
  cluster. That Postgres needs no backups. The chart takes only their connection
  configuration and installs neither.
- `shared/deploy/compose.yaml` runs both binaries with `file://` storage and a
  Postgres for development and evaluation.
- No cache service until a requirement needs one, measured. Every replica
  caches immutable objects in memory and on local disk.
