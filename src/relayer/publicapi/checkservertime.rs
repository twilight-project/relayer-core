use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
pub fn iso8601(st: &std::time::SystemTime) -> String {
    let dt: DateTime<Utc> = st.clone().into();
    format!("{}", dt.format("%+"))
}

impl ServerTime {
    pub fn new(st: std::time::SystemTime) -> Self {
        return ServerTime {
            iso: iso8601(&st),
            epoch: st
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_micros()
                .to_string(),
        };
    }

    pub fn epoch_to_system_time(self) -> SystemTime {
        let system_time: std::time::SystemTime =
            UNIX_EPOCH + Duration::from_micros(self.epoch.parse::<u64>().unwrap());
        return system_time;
    }
    pub fn now() -> ServerTime {
        let st = SystemTime::now();
        return ServerTime {
            iso: iso8601(&st),
            epoch: st
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_micros()
                .to_string(),
        };
    }
}
