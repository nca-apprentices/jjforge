# 0010. The infra repository runs the backing services

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

jjforge needs Postgres, Redpanda, and SeaweedFS. Each needs backups, upgrades,
and capacity planning, which the infra repository already does for the shared
cluster. Self-hosters want one command to try jjforge.

Options: the chart installs the services as subcharts, or the infra repository
runs them and the chart only connects. A cache such as Redis was also
considered.

## Decision

- In a cluster, the infra repository runs Postgres, Redpanda, and SeaweedFS.
- The chart takes only their connection configuration and installs none of
  them.
- `shared/deploy/compose.yaml` runs them for development and evaluation. Each
  service joins it when the first module uses it.
- No cache until a requirement needs one, measured.
