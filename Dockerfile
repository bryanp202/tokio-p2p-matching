FROM rust:latest AS builder
WORKDIR /code/rustp2p_matchmaking
COPY . .
RUN cargo build --release

FROM alpine:latest
COPY --from=builder /code/rustp2p_matchmaking/target/release/rustp2p_matchmaking /rustp2p_matchmaking
CMD [ "/rustp2p_matchmaking" ]