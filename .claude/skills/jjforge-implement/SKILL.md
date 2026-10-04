---
name: jjforge-implement
description: Build one specified requirement until its scenarios pass. Use when asked to implement, build, or code an issue such as R012 or #25 whose spec has merged.
---

# Implement a requirement

The build PR of ADR 0017. The spec has merged, and this PR makes it pass
without changing it.

1. **Gate.** `mise run spec:trace` lists, under `R<id>`, the operation, the
   scenario, and for a write path the design. If one is missing, the
   requirement isn't specified: stop and run `/jjforge-specify <n>` first.
2. **Read.** The issue, the scenarios that start with `satisfies #<n>`, the
   design section that starts with `designs #<n>`, and its proto.
3. **Red.** Move the Hurl file from `shared/e2e/http/pending/` to
   `shared/e2e/http/`, or drop the `skip "pending #<n>"` line. Run
   `mise run up` and `mise run e2e`, and watch the scenarios fail. If the file also covers a
   requirement this PR doesn't build, ask before splitting it.
4. **Build.** The tag's controller overrides the generated method, as
   ADR 0011 decides. A write path follows its design step by step: the
   policy, the claimed names, the events, and the read model.
5. **Spec gaps.** A scenario or a design that can't be built as written is a
   gap in the spec. Stop and fix it in a spec PR through `/jjforge-specify`.
   This PR changes no scenario.
6. **Green.** `mise run test` and `mise run lint` pass.
7. **Run it.** Passing tests aren't enough. Once the change is done, run the
   system built from the working tree and watch it work. Every later change
   repeats this step.
   - Start `mise run up` in the background. It rebuilds both images, and
     needs `podman machine start` first.
   - `mise run e2e` sends every Hurl file and runs every Bats file. The
     moved scenarios pass, and the pending Hurl files fail only on their
     501.
   - Run each `jf` command the requirement adds, with `JJFORGE_ENDPOINT`
     pointing at the server.
   - `mise run compose logs` shows each request from the scenarios, and no
     error or stack trace in any service.
   - The state the design says a write leaves exists: the events in the
     stream, the rows in the read model, and the objects in the store. Read
     each through `mise run compose exec <service>`.
   - Every external system runs as its twin from `shared/deploy/compose.yaml`,
     as ADR 0018 decides. Integration tests start the same image, and talk
     to it, never to a mock.
   - Stop with `mise run compose down`.
8. **Surfaces.** In `shared/docs/surfaces.md`, drop "(planned)" from the
   requirement's row for each surface this PR builds.
9. **PR.** Title `feat(<module>): <what a person can do>`. Template:
   `Satisfies #<n>`, spec changed: no. The body lists what step 7 checked
   and what it showed.
