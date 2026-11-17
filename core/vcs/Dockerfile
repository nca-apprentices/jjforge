# Built from the repository root: the stubs crate in core/ compiles the contract
# in contracts/proto/.
FROM rust:1.99.1-slim@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7 AS build

WORKDIR /src
COPY contracts/proto contracts/proto
COPY core core

WORKDIR /src/core
RUN cargo build --release --bin vcs

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2

COPY --from=build /src/core/target/release/vcs /usr/local/bin/vcs

ENV VCS_ADDR=0.0.0.0:50052
EXPOSE 50052 9090
# distroless's nonroot user, by ID so a runtime can check it without a passwd file.
USER 65532
ENTRYPOINT ["/usr/local/bin/vcs"]
