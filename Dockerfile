# --------------------------------------------------------------------------------------------------------------------                                                                                                                                                                                #*eddiere
# Multi-stage Dockerfile for PgShield-rs
FROM rust:1.80-alpine as builder

RUN apk add --no-cache musl-dev

WORKDIR /app
COPY Cargo.toml ./
COPY src ./src

RUN cargo build --release

FROM alpine:latest

WORKDIR /app
COPY --from=builder /app/target/release/pgshield /app/pgshield

EXPOSE 6432

ENTRYPOINT ["/app/pgshield"]
