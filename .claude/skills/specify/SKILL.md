---
name: specify
description: Write the spec PR for one requirement, before anyone builds it. Use when asked to specify, spec, or write scenarios or a design for an issue such as R012 or #25.
---

# Specify a requirement

The spec PR for one requirement, as `shared/docs/planning.md` and ADR 0017
decide. It holds scenarios, the contract change the scenarios need, and the
design section if the requirement changes state. Someone else builds it.

1. **Read the requirement.** `gh issue view <n> --json title,body,type`. It
   must be a Requirement. Read its epic too, for the surfaces it promises.
   Read every earlier spec on the same tag: the `.tsp` file, the design, and
   the Hurl files.
2. **Write the scenarios first.** One Hurl file under `shared/http/pending/`,
   starting with `# satisfies #<n>`, one comment line per scenario, concrete
   values, every refusal with its problem code. Name what the scenario
   creates `<word>-{{run}}`. Add a token or fixture the scenarios need to
   `shared/http/pending/vars.env`. A CLI or sync scenario is a Rust test
   marked `#[ignore = "pending #<n>"]`. Check: `hurlfmt --check <file>`.
3. **Check the contract against the scenarios.** Every assert must be
   expressible with `shared/api/` and `shared/proto/`. If one isn't, change
   the contract, never the scenario, and add `#<n>` to the operation's
   `x-requirements`. Check: `mise run spec:lint` and `mise run spec:breaking`.
4. **Design the write path.** Skip for a read. Otherwise add a section to the
   epic's file in `shared/docs/design/`, starting with `designs #<n>`:
   policy and its refusal, claimed names, stream and events, read model and
   queries, and partial failures with who repairs them. Add the event schema as
   `shared/proto/<module>/v1/events.proto` in the same PR. A sequence diagram
   for the happy path when it crosses the server, vcsd, and the store. Check:
   `buf lint shared/proto`.
5. **Run the checks.** `mise run spec:trace` must list the operation, the
   scenario file, and the design under `R<id>` in `build/trace.yaml`.
   `mise run lint` for formatting, prose, and links.
6. **Open the PR.** Title `feat(spec): <what a person can do>`. Fill the
   template: `Satisfies #<n>`, spec changed, design changed. Link the design
   from the epic's Design field if it is new. Ask the reviewer one question:
   could someone build this without asking the author?
