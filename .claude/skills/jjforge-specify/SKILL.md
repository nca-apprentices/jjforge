---
name: jjforge-specify
description: Write the spec PR for one requirement, before anyone builds it. Use when asked to specify, spec, or write scenarios or a design for an issue such as R012 or #25.
---

# Specify a requirement

The spec PR of ADR 0017: scenarios, the contract change they need, and the
design section if the requirement changes state. Someone else builds it.

1. **Read.** `gh issue view <n> --json title,body,type`: a Requirement, and
   its epic for the surfaces it promises. Then the tag's `.tsp` file, its
   design, and its Hurl files.
2. **Scenarios first.** A Hurl file under `shared/http/pending/`, starting
   with `# satisfies #<n>`: one comment per scenario, concrete values, every
   refusal with its problem code, created names as `<word>-{{run}}`. New
   fixtures go in `shared/http/pending/vars.env`. A CLI or sync scenario is a
   Rust test marked `#[ignore = "pending #<n>"]`. Check: `hurlfmt --check`.
3. **Contract against the scenarios.** Every assert must be expressible with
   `shared/api/` and `shared/proto/`. If not, change the contract, never the
   scenario, and add `#<n>` to `x-requirements`. Check: `mise run spec:lint`
   and `mise run spec:breaking`.
4. **Design the write path.** Skip for a read. Add a section to the epic's
   file in `shared/docs/design/` in the format its README gives, and the
   event schema as `shared/proto/<module>/v1/events.proto`. Name each
   external system the path calls and its twin, as ADR 0018 decides. A new
   system's twin joins `shared/deploy/compose.yaml`, with its seed in
   `shared/deploy/twins/`. Check:
   `buf lint shared/proto`.
5. **Trace.** `mise run spec:trace` lists the operation, the scenario, and
   the design under `R<id>` in `build/trace.yaml`. Then `mise run lint`.
6. **Review.** Run the `jjforge-spec-reviewer` agent and close every gap it
   reports.
7. **PR.** Title `feat(spec): <what a person can do>`. Template:
   `Satisfies #<n>`, spec changed, design changed.

Next, once the PR merges: `/jjforge-implement <n>`, by someone else.
