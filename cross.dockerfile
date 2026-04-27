FROM rust:alpine AS build

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app
COPY . .
RUN rm -f fireplace
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:3.11 AS final

ENV TERM=xterm-256color

WORKDIR /app
COPY --from=build /app/target/x86_64-unknown-linux-musl/release/fireplace .
ENTRYPOINT [ "/app/fireplace" ]
