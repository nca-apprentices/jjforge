# Built from the repository root: the stubs crate in rust/ compiles the contract
# in shared/proto/.
FROM rust:1.98-slim AS build

RUN apt-get update \
    && apt-get install -y --no-install-recommends protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY shared/proto shared/proto
COPY rust rust

WORKDIR /src/rust
RUN cargo build --release --bin vcsd

FROM gcr.io/distroless/cc-debian13:nonroot

COPY --from=build /src/rust/target/release/vcsd /usr/local/bin/vcsd

ENV VCSD_ADDR=0.0.0.0:50052
EXPOSE 50052
# distroless's nonroot user, by ID so a runtime can check it without a passwd file.
USER 65532
ENTRYPOINT ["/usr/local/bin/vcsd"]
