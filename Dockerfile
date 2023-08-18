FROM rust:1.71.0-alpine3.18 as builder
COPY . .
RUN cargo build --release

FROM alpine:3.14.2
COPY --from=builder /target/release/whoami /usr/local/bin/whoami
CMD ["whoami"]