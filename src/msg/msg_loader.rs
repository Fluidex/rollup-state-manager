use super::msg_consumer::{Simple, SimpleConsumer, SimpleMessageHandler};
use crate::test_utils::messages::{parse_msg, WrappedMessage};
use rdkafka::consumer::StreamConsumer;
use rdkafka::message::{BorrowedMessage, Message};
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn load_msgs_from_file(
    filepath: &str,
    sender: crossbeam_channel::Sender<WrappedMessage>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    let filepath = filepath.to_string();
    println!("loading from {}", filepath);
    Some(std::thread::spawn(move || {
        let file = File::open(filepath)?;
        // since
        for l in BufReader::new(file).lines() {
            let msg = parse_msg(l?).expect("invalid data");
            sender.try_send(msg)?;
        }
        Ok(())
    }))
}

const UNIFY_TOPIC: &str = "unifyevents";
const MSG_TYPE_BALANCES: &str = "balances";
const MSG_TYPE_USERS: &str = "registeruser";
const MSG_TYPE_ORDERS: &str = "orders";
const MSG_TYPE_TRADES: &str = "trades";

pub fn load_msgs_from_mq(
    brokers: &str,
    sender: crossbeam_channel::Sender<WrappedMessage>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    let consumer: StreamConsumer = rdkafka::config::ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", "rollup_msg_consumer")
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .create()
        .unwrap();
    let writer = MessageWriter { sender };

    Some(std::thread::spawn(move || {
        let rt: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let cr_main = SimpleConsumer::new(&consumer)
                .add_topic(UNIFY_TOPIC, Simple::from(&writer))
                .unwrap();
            tokio::select! {
                err = cr_main.run_stream(|cr|cr.stream()) => {
                    log::error!("Kafka consumer error: {}", err);
                }
            }
        });

        Ok(())
    }))
}

struct MessageWriter {
    sender: crossbeam_channel::Sender<WrappedMessage>,
}

impl SimpleMessageHandler for &MessageWriter {
    fn on_message(&self, msg: &BorrowedMessage<'_>) {
        let msg_type = std::str::from_utf8(msg.key().unwrap()).unwrap();
        let msg_payload = std::str::from_utf8(msg.payload().unwrap()).unwrap();
        let message = match msg_type {
            MSG_TYPE_BALANCES => {
                let data = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::BALANCE(data)
            }
            MSG_TYPE_ORDERS => {
                let data = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::ORDER(data)
            }
            MSG_TYPE_TRADES => {
                let data = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::TRADE(data)
            }
            MSG_TYPE_USERS => {
                let data = serde_json::from_str(msg_payload).unwrap();
                WrappedMessage::USER(data)
            }
            _ => unreachable!(),
        };

        self.sender.try_send(message).unwrap();
    }
}
