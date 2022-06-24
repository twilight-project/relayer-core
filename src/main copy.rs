use questdb::QuestDB;
use serde::{Deserialize, Serialize};
#[tokio::main]
async fn async_main() {
    let connection = QuestDB::new("http://localhost:9000");

    // let res = connection
    //     .exec::<TestData>("select * from recentorders", Some(2), None, None)
    //     .await
    //     .unwrap();
    let res = connection
        .exec::<ResultOk>(
            "INSERT INTO recentorders VALUES (2, 1000.021,14785452.325,now())",
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // println!("{:#?}", res);
}
#[derive(Serialize, Deserialize, Debug)]
struct TestData {
    id: i32,
    ts: f64,
    temp: f64,
    sensor_id: String,
}
