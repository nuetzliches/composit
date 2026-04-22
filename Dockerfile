# Build composit from source — no pre-built binary required.
# docker run --rm -v "$PWD:/repo" ghcr.io/nuetzliches/composit scan
FROM rust:alpine AS builder
RUN apk add --no-cache git musl-dev
WORKDIR /build
COPY . .
RUN cargo build --release

FROM alpine:3.21
RUN apk add --no-cache ca-certificates
COPY --from=builder /build/target/release/composit /usr/local/bin/composit
WORKDIR /repo
ENTRYPOINT ["composit"]
