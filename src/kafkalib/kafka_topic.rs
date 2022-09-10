////// ! ## kafka_topic
////// !Command for comunicating with Zookeerper running inside docker container
////// !
////// ! - `kafka_topic::kafka_new_topic` - for creating new topic to Kafka
////// !
////// ! ### Examples
////// !
////// ! Basic usage:
////// !
////// ! ```ignore
////// ! mod kafkalib::kafka_topic;
////// ! fn main() {
////// !
////// !     // kafka_topic::kafka_new_topic("new_topic_name");
////// !     kafka_topic::kafka_new_topic("BinanceMiniTickerPayload");
////// !
////// ! }
////// ! ```

#![allow(dead_code)]

use std::process::Command;

/// ## kafka_new_topic
///
/// ### Examples
///   
/// Basic usage:
///
/// ```rust,no_run
///use crate::twilight_relayer_rust::kafkalib::kafka_topic;
/// fn main() {
///     kafka_topic::kafka_new_topic("new_topic_name");
/// }
///
/// ```
///
pub fn kafka_new_topic(new_topic: &str) {
    let mut output5 = Command::new("docker");
    output5.arg("exec");
    output5.arg("-it");
    //container image name // zookeeper
    output5.arg("zookeeper");
    output5.arg("sh");
    output5.arg("-c");
    //docker exec -it <Container-name> sh -c "cd usr/bin && kafka-topics --topic <New Topic> --create --zookeeper <localhost:2181> --partitions 1 --replication-factor 1"
    output5.arg(format!("cd usr/bin && kafka-topics --topic {} --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1 --config retention.ms=-1 --config cleanup.policy=compact",new_topic));
    // output5.arg(format!("cd usr/bin && kafka-topics --topic {} --create --zookeeper zookeeper:2181 --partitions 1 --replication-factor 1",new_topic));
    // output5.arg(format!("cd usr/bin && kafka-topics --topic {} --create --zookeeper zookeeper:2181 --partitions 2 --replication-factor 1 --config retention.ms=-1",new_topic));
    output5.arg("exit");
    output5.spawn().expect("process failed to execute");
}
