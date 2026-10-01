# 0007. The git bridge comes first

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

jj syncs with remotes through git today. A native protocol would keep change
IDs, conflicts, and the operation log intact, but jj's support for non-git
remotes is still open, and every jj user already has git.

Options: build the native protocol first, or serve git smart HTTP first and
add the native protocol later.

## Decision

- vcsd serves git smart HTTP first, so `jj git push` and `git clone` work
  against jjforge.
- vcsd keeps the change IDs that jj writes into commits.
- The native `jf` sync protocol follows, after a spike on jj-cli and non-git
  remotes. The git bridge stays for interoperability.
