// use std::time::Instant;
// use twilight_relayer_rust::relayer::*;

// fn main() {
//     println!("🚀 Quick Performance Test for Twilight Relayer");

//     // Test order creation performance
//     let start = Instant::now();
//     let iterations = 1000;

//     for i in 0..iterations {
//         let rpc_request = CreateTraderOrder {
//             account_id: format!("perf_test_{}", i),
//             position_type: if i % 2 == 0 {
//                 PositionType::LONG
//             } else {
//                 PositionType::SHORT
//             },
//             order_type: OrderType::MARKET,
//             leverage: 10.0,
//             initial_margin: 100.0,
//             available_margin: 100.0,
//             order_status: OrderStatus::PENDING,
//             entryprice: 50000.0,
//             execution_price: 50000.0,
//         };

//         let _order = TraderOrder::new_order(rpc_request);
//     }

//     let elapsed = start.elapsed();
//     println!("✅ Created {} orders in {:?}", iterations, elapsed);
//     println!("📊 Average time per order: {:?}", elapsed / iterations);
//     println!(
//         "⚡ Orders per second: {:.2}",
//         iterations as f64 / elapsed.as_secs_f64()
//     );
// }
