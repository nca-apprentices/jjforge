# Built from the repository root: the stubs crate in core/ compiles the contract
# in contracts/proto/.
FROM rust:1.99.1-slim@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7 AS chef
RUN cargo install cargo-chef --locked --version 0.1.78
WORKDIR /src/core

# The recipe lists only the manifests, so the cook layer below stays cached
# until a dependency changes, and a source change rebuilds only the workspace.
FROM chef AS plan
COPY core ./
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=plan /src/core/recipe.json recipe.json
RUN cargo chef cook --release --bin vcs --recipe-path recipe.json
COPY contracts/proto /src/contracts/proto
COPY core ./
RUN cargo build --release --bin vcs

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2

COPY --from=build /src/core/target/release/vcs /usr/local/bin/vcs

ENV VCS_ADDR=0.0.0.0:50052
EXPOSE 50052 9090
# distroless's nonroot user, by ID so a runtime can check it without a passwd file.
USER 65532
ENTRYPOINT ["/usr/local/bin/vcs"]
