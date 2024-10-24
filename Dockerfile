FROM --platform=$BUILDPLATFORM messense/rust-musl-cross:aarch64-musl AS build_arm64
FROM --platform=$BUILDPLATFORM messense/rust-musl-cross:x86_64-musl AS build_amd64

FROM --platform=$BUILDPLATFORM build_$TARGETARCH AS build
WORKDIR /app
COPY . .
RUN cargo build -r --target $RUST_MUSL_CROSS_TARGET
RUN cp /app/target/${RUST_MUSL_CROSS_TARGET}/release/rustic-exporter /app/rustic-exporter

FROM --platform=$TARGETPLATFORM gcr.io/distroless/static:nonroot
COPY --from=build --chown=nonroot:nonroot /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build --chown=nonroot:nonroot /app/rustic-exporter /rustic-exporter
