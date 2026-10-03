# Roadmap

The initiatives follow the [dependency order](architecture.md#dependency-order).
Their status, targets, and requirements live in the
[jjforge Project](https://github.com/orgs/nca-apprentices/projects/1).
[Planning](planning.md) explains the IDs.

I0 to I2 have their requirements written, except the spike E19, whose finding
can add requirements to E11 and E20. The epics of I3 to I7 get their
requirements when their initiative starts. Native sync is part of I2, because
without git it is the only way into the forge, as
[ADR 0017](adr/0017-native-jj-without-git.md) decides.

<!-- The epic titles match their issues. -->
<!-- vale Google.WordListCase = NO -->

- [I0 Foundations](https://github.com/nca-apprentices/jjforge/issues/1)
  - [E01 Planning and docs](https://github.com/nca-apprentices/jjforge/issues/2)
  - [E02 Contract first](https://github.com/nca-apprentices/jjforge/issues/3)
  - [E03 Module boundaries](https://github.com/nca-apprentices/jjforge/issues/4)
  - [E36 Storage kernel](https://github.com/nca-apprentices/jjforge/issues/84)
- [I1 Identity and access](https://github.com/nca-apprentices/jjforge/issues/5)
  - [E04 Human login](https://github.com/nca-apprentices/jjforge/issues/6)
  - [E05 Authorization baseline](https://github.com/nca-apprentices/jjforge/issues/13)
  - [E06 Agent and workload principals](https://github.com/nca-apprentices/jjforge/issues/18)
  - [E07 Scoped tokens](https://github.com/nca-apprentices/jjforge/issues/19)
  - [E08 Secrets](https://github.com/nca-apprentices/jjforge/issues/20)
  - [E09 Audit log](https://github.com/nca-apprentices/jjforge/issues/21)
- [I2 Browse jj repos (MVP)](https://github.com/nca-apprentices/jjforge/issues/22)
  - [E10 Orgs and repos](https://github.com/nca-apprentices/jjforge/issues/23)
  - [E19 Spike: native jj backend and sync](https://github.com/nca-apprentices/jjforge/issues/62)
  - [E20 Object store](https://github.com/nca-apprentices/jjforge/issues/63)
  - [E11 Push and clone](https://github.com/nca-apprentices/jjforge/issues/29)
  - [E12 History](https://github.com/nca-apprentices/jjforge/issues/34)
  - [E13 Files and diff](https://github.com/nca-apprentices/jjforge/issues/41)
  - [E14 Web browsing](https://github.com/nca-apprentices/jjforge/issues/47)
  - [E15 CLI](https://github.com/nca-apprentices/jjforge/issues/51)
- [I3 Review and land](https://github.com/nca-apprentices/jjforge/issues/57)
  - [E16 Review rounds](https://github.com/nca-apprentices/jjforge/issues/58)
  - [E17 Stacks and landing queue](https://github.com/nca-apprentices/jjforge/issues/59)
  - [E18 Conflicts as work items](https://github.com/nca-apprentices/jjforge/issues/60)
- [I4 Sync at scale](https://github.com/nca-apprentices/jjforge/issues/61)
  - [E22 Change ownership and push policy](https://github.com/nca-apprentices/jjforge/issues/65)
- [I5 Events and automation](https://github.com/nca-apprentices/jjforge/issues/66)
  - [E23 Operation log as events](https://github.com/nca-apprentices/jjforge/issues/67)
  - [E24 Gates and observers](https://github.com/nca-apprentices/jjforge/issues/68)
  - [E25 Sandboxes](https://github.com/nca-apprentices/jjforge/issues/69)
  - [E26 Durable workflows](https://github.com/nca-apprentices/jjforge/issues/70)
  - [E27 Quotas and budgets](https://github.com/nca-apprentices/jjforge/issues/71)
- [I6 Components](https://github.com/nca-apprentices/jjforge/issues/72)
  - [E28 Registry and catalog](https://github.com/nca-apprentices/jjforge/issues/73)
  - [E29 Install as grant](https://github.com/nca-apprentices/jjforge/issues/74)
  - [E30 Supply chain](https://github.com/nca-apprentices/jjforge/issues/75)
  - [E31 Configuration as code](https://github.com/nca-apprentices/jjforge/issues/76)
- [I7 Specs and measurement](https://github.com/nca-apprentices/jjforge/issues/77)
  - [E32 Measurement store](https://github.com/nca-apprentices/jjforge/issues/78)
  - [E33 Spec trace](https://github.com/nca-apprentices/jjforge/issues/79)
  - [E34 Experiments and promotion](https://github.com/nca-apprentices/jjforge/issues/80)
  - [E35 Metering](https://github.com/nca-apprentices/jjforge/issues/81)

<!-- vale Google.WordListCase = YES -->

## Not on the backlog yet

These stay here until I2 ships: the build cache and remote execution, the
artifact store, issue tracking inside the forge, and an AI panel in the web
app.
