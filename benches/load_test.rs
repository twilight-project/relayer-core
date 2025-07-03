// use std::sync::atomic::{AtomicUsize, Ordering};
// use std::sync::{Arc, Mutex};
// use std::thread;
// use std::time::{Duration, Instant};

// #[tokio::main]
// async fn main() {
//     println!("Starting load test...");

//     let concurrent_users = 100;
//     let test_duration = Duration::from_secs(60);
//     let successful_operations = Arc::new(AtomicUsize::new(0));
//     let failed_operations = Arc::new(AtomicUsize::new(0));

//     let start_time = Instant::now();
//     let mut handles = vec![];

//     for i in 0..concurrent_users {
//         let successful_ops = Arc::clone(&successful_operations);
//         let failed_ops = Arc::clone(&failed_operations);

//         let handle = tokio::spawn(async move {
//             while start_time.elapsed() < test_duration {
//                 match simulate_order_creation(i).await {
//                     Ok(_) => successful_ops.fetch_add(1, Ordering::Relaxed),
//                     Err(_) => failed_ops.fetch_add(1, Ordering::Relaxed),
//                 };

//                 // Small delay between operations
//                 tokio::time::sleep(Duration::from_millis(100)).await;
//             }
//         });

//         handles.push(handle);
//     }

//     // Wait for all tasks to complete
//     for handle in handles {
//         handle.await.unwrap();
//     }

//     let total_time = start_time.elapsed();
//     let successful = successful_operations.load(Ordering::Relaxed);
//     let failed = failed_operations.load(Ordering::Relaxed);
//     let total = successful + failed;

//     println!("Load test completed!");
//     println!("Duration: {:?}", total_time);
//     println!("Total operations: {}", total);
//     println!("Successful operations: {}", successful);
//     println!("Failed operations: {}", failed);
//     println!(
//         "Success rate: {:.2}%",
//         (successful as f64 / total as f64) * 100.0
//     // );
//     println!(
//         "Throughput: {:.2} ops/sec",
//         total as f64 / total_time.as_secs_f64()
//     );
// }

// async fn simulate_order_creation(user_id: usize) -> Result<(), String> {
//     // Simulate creating an order
//     todo!("Implement order creation simulation")
// }
