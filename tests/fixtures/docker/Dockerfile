FROM rust:1.77-alpine AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:3.19
COPY --from=builder /app/target/release/api /usr/local/bin/api
EXPOSE 8080
ENTRYPOINT ["api"]
