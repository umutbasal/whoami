FROM rust:alpine3.18 as builder

WORKDIR /app/src
RUN USER=root

RUN apk add pkgconfig openssl-dev libc-dev
COPY ./ ./
RUN cargo build --release

FROM ghcr.io/edera-dev/am-i-isolated@sha256:7fd9bf036beb09f9d1c480f6e793faf9f6a5979efc2711bcd56e5c844eb98595 as am-i-isolated

FROM alpine:3.18
WORKDIR /app
RUN apk update \
    && apk add openssl ca-certificates curl bind-tools

EXPOSE 8080

COPY --from=builder /app/src/target/release/whoami /app/whoami
COPY --from=am-i-isolated /bin/am-i-isolated /app/am-i-isolated

CMD ["/app/whoami"]