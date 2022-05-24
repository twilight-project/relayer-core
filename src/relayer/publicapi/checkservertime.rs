use std::time::SystemTime;
pub fn check_server_time() -> u128 {
    let ttime = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    println!("{:#?}", ttime);
    ttime.as_micros()
}
// {
//     "iso": "2021-02-02T18:35:45Z",
//     "epoch": "1611965998.515",
//   }
