FROM rust:alpine3.18 as builder

WORKDIR /app/src
RUN USER=root

RUN apk add pkgconfig openssl-dev libc-dev
COPY ./ ./
RUN cargo build --release

FROM alpine:3.18
WORKDIR /app
RUN apk update \
    && apk add openssl ca-certificates curl bind-tools

EXPOSE 8080

COPY --from=builder /app/src/target/release/whoami /app/whoami

CMD ["/app/whoami"]