use std::time::Duration;

use bincode::{Encode, config, encode_into_slice};
use rand::Rng;
use redis::{AsyncCommands, RedisError};

use crate::{MATCHMAKING_QUEUE_NAME, player_match_channel};

const MATCHER_SLEEP_TIME: Duration = std::time::Duration::from_secs(2);

#[derive(Encode)]
struct MatchData<'a> {
    local_is_host: bool,
    local: &'a str,
    peer: &'a str,
}

pub async fn matcher(mut redis_con: redis::aio::MultiplexedConnection) -> ! {
    loop {
        match_players(&mut redis_con)
            .await
            .expect("Critical error on player matching");
        tokio::time::sleep(MATCHER_SLEEP_TIME).await;
    }
}

async fn match_players(
    redis_con: &mut redis::aio::MultiplexedConnection,
) -> Result<(), RedisError> {
    let players_in_queue: usize = redis_con.zcard(MATCHMAKING_QUEUE_NAME).await?;
    let matches = players_in_queue / 2;

    for _ in 0..matches {
        let players: [String; 2] = redis_con.zrange(MATCHMAKING_QUEUE_NAME, 0, 1).await?;
        let _: () = redis_con
            .zremrangebyrank(MATCHMAKING_QUEUE_NAME, 0, 1)
            .await?;
        make_match(redis_con, players).await?;
    }

    Ok(())
}

async fn make_match(
    redis_con: &mut redis::aio::MultiplexedConnection,
    players: [String; 2],
) -> Result<(), RedisError> {
    let player1_host: bool = rand::rng().random();

    let player1_match_data = MatchData {
        local_is_host: player1_host,
        local: &players[0],
        peer: &players[1],
    };

    let player2_match_data = MatchData {
        local_is_host: !player1_host,
        local: &players[1],
        peer: &players[0],
    };

    let mut buf = [0u8; 1024];
    let len = encode_into_slice(player1_match_data, &mut buf, config::standard())
        .expect("failed to encode match data");
    let _: () = redis_con
        .publish(player_match_channel(&players[0]), &buf[..len])
        .await?;
    let len = encode_into_slice(player2_match_data, &mut buf, config::standard())
        .expect("failed to encode match data");
    let _: () = redis_con
        .publish(player_match_channel(&players[1]), &buf[..len])
        .await?;

    Ok(())
}
