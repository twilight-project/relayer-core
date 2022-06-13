use std::time::SystemTime;

use serde_derive::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerTime {
    pub iso: String,
    pub epoch: String,
}

pub fn check_server_time() -> ServerTime {
    let ttime = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let system_time = SystemTime::now();
    return ServerTime {
        iso: iso8601(&system_time),
        epoch: ttime.as_micros().to_string(),
    };
}

use chrono::prelude::{DateTime, Utc};
//https://stackoverflow.com/questions/64146345/how-do-i-convert-a-systemtime-to-iso-8601-in-rust
fn iso8601(st: &std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = st.clone().into();
    format!("{}", dt.format("%+"))
}
