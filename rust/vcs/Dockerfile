# Built from the repository root: the stubs crate in rust/ compiles the contract
# in shared/proto/.
FROM rust:1.98.1-slim AS build

WORKDIR /src
COPY shared/proto shared/proto
COPY rust rust

WORKDIR /src/rust
RUN cargo build --release --bin vcs

FROM gcr.io/distroless/cc-debian13:nonroot

COPY --from=build /src/rust/target/release/vcs /usr/local/bin/vcs

ENV VCS_ADDR=0.0.0.0:50052
EXPOSE 50052 9090
# distroless's nonroot user, by ID so a runtime can check it without a passwd file.
USER 65532
ENTRYPOINT ["/usr/local/bin/vcs"]
