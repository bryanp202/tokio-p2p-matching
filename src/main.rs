mod matcher;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use redis::{AsyncCommands, PushInfo, PushKind, from_redis_value};
use tokio::{net::UdpSocket, sync::mpsc::UnboundedReceiver};

const MATCHMAKING_SERVER_VERSION: &[u8] = "0.1.0".as_bytes();
const UDP_SOCKET_ADDR: &str = "127.0.0.1:8000";
const MATCHMAKING_QUEUE_NAME: &str = "players:queue";

const TIME_OUT: Duration = std::time::Duration::from_secs(30);

struct Client {
    id: String,
    src_addr: SocketAddr,
    redis_con: redis::aio::MultiplexedConnection,
    pubsub: UnboundedReceiver<PushInfo>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let redis_host = std::env::var("REDIS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let redis_port = std::env::var("REDIS_PORT").unwrap_or_else(|_| "6379".to_string());
    let redis_addr = format!("redis://{redis_host}:{redis_port}/?protocol=resp3");
    let redis_client = redis::Client::open(redis_addr).expect("Failed to connect to redis");

    let udp_socket = tokio::net::UdpSocket::bind(UDP_SOCKET_ADDR)
        .await
        .expect("Failed to bind socket");
    let udp_socket = Arc::new(udp_socket);

    let redis_con = redis_client.get_multiplexed_async_connection().await?;
    tokio::spawn(matcher::matcher(redis_con));

    let mut buf = [0u8; 1024];

    loop {
        let (len, src_addr) = udp_socket.recv_from(&mut buf).await.unwrap();

        let (tx, pubsub) = tokio::sync::mpsc::unbounded_channel();
        let redis_config = redis::AsyncConnectionConfig::new().set_push_sender(tx);
        let redis_con = redis_client
            .get_multiplexed_async_connection_with_config(&redis_config)
            .await?;
        let client = Client {
            id: src_addr.to_string(),
            redis_con,
            src_addr,
            pubsub,
        };

        let data: Box<[u8]> = buf[0..len].into();
        let socket_clone = udp_socket.clone();
        tokio::spawn(handle_client(client, socket_clone, data));
    }
}

async fn handle_client(mut client: Client, socket: Arc<UdpSocket>, data: Box<[u8]>) {
    if &*data != MATCHMAKING_SERVER_VERSION {
        return;
    }

    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let _: () = client
        .redis_con
        .zadd(MATCHMAKING_QUEUE_NAME, &client.id, time.as_nanos())
        .await
        .unwrap();

    client
        .redis_con
        .subscribe(player_match_channel(&client.id))
        .await
        .expect("Failed to subscribe");

    match tokio::time::timeout(TIME_OUT, wait_for_match(&mut client, socket)).await {
        Ok(()) => {}
        Err(_) => {
            // Clean up
            let _: () = client
                .redis_con
                .zrem(MATCHMAKING_QUEUE_NAME, &client.id)
                .await
                .unwrap();
        }
    }
}

async fn wait_for_match(client: &mut Client, socket: Arc<UdpSocket>) {
    loop {
        let Some(msg) = client.pubsub.recv().await else {
            let _: () = client
                .redis_con
                .zrem(MATCHMAKING_QUEUE_NAME, &client.id)
                .await
                .unwrap();
            return;
        };

        if msg.kind != PushKind::Message {
            continue;
        }

        let data: Vec<u8> = from_redis_value(&msg.data[1]).unwrap();
        socket
            .send_to(&data, &client.src_addr)
            .await
            .expect("Failed to send back response");

        return;
    }
}

fn player_match_channel(id: &str) -> String {
    format!("matches:{}", id)
}
