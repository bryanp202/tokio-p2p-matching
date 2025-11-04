# Build Stage
FROM rust:1.88.0-slim AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/home/rustp2p" \
    --shell "/sbin/nologin" \
    --uid 10001 \
    rustp2p_matchmaking

WORKDIR /usr/src/app
COPY ./Cargo.lock .
COPY ./Cargo.toml .

RUN cargo build --target x86_64-unknown-linux-musl --release --frozen --jobs 1 || true

COPY ./src ./src
RUN cargo build --target x86_64-unknown-linux-musl --release

# Final stage
FROM alpine:3.22

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group
RUN mkdir /app
COPY --from=builder --chown=rustp2p_matchmaking:rustp2p_matchmaking \
    /usr/src/app/target/x86_64-unknown-linux-musl/release/rustp2p_matchmaking /app/rustp2p_matchmaking


USER rustp2p_matchmaking:rustp2p_matchmaking

WORKDIR /app
EXPOSE 8000

ENTRYPOINT ["./rustp2p_matchmaking"]