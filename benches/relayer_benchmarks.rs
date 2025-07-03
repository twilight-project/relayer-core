// use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
// use std::time::Duration;
// use twilight_relayer_rust::relayer::*;
// use uuid::Uuid;

// fn benchmark_order_creation(c: &mut Criterion) {
//     let mut group = c.benchmark_group("order_creation");

//     // Test different order types
//     let order_types = vec![OrderType::MARKET, OrderType::LIMIT];

//     for order_type in order_types {
//         group.bench_with_input(
//             BenchmarkId::new("create_trader_order", format!("{:?}", order_type)),
//             &order_type,
//             |b, &order_type| {
//                 b.iter(|| {
//                     let rpc_request = create_sample_create_trader_order(order_type.clone());
//                     black_box(TraderOrder::new_order(rpc_request));
//                 });
//             },
//         );
//     }

//     group.finish();
// }

// fn benchmark_order_processing_throughput(c: &mut Criterion) {
//     let mut group = c.benchmark_group("order_throughput");
//     group.sample_size(100);
//     group.measurement_time(Duration::from_secs(30));

//     // Benchmark processing multiple orders concurrently
//     let order_counts = vec![10, 50, 100, 500];

//     for count in order_counts {
//         group.bench_with_input(
//             BenchmarkId::new("process_orders", count),
//             &count,
//             |b, &count| {
//                 b.iter(|| {
//                     let orders = create_sample_orders(count);
//                     black_box(process_orders_batch(orders));
//                 });
//             },
//         );
//     }

//     group.finish();
// }

// fn benchmark_order_serialization(c: &mut Criterion) {
//     c.bench_function("order_serialization", |b| {
//         let order = create_sample_trader_order();
//         b.iter(|| {
//             black_box(order.serialize());
//         });
//     });

//     c.bench_function("order_to_hmset", |b| {
//         let order = create_sample_trader_order();
//         b.iter(|| {
//             black_box(order.to_hmset_arg_array());
//         });
//     });
// }

// // Helper functions
// fn create_sample_create_trader_order(order_type: OrderType) -> CreateTraderOrder {
//     CreateTraderOrder {
//         account_id: "test_account_123".to_string(),
//         position_type: PositionType::LONG,
//         order_type,
//         leverage: 10.0,
//         initial_margin: 100.0,
//         available_margin: 100.0,
//         order_status: OrderStatus::PENDING,
//         entryprice: 50000.0,
//         execution_price: 50000.0,
//     }
// }

// fn create_sample_trader_order() -> TraderOrder {
//     let rpc_request = create_sample_create_trader_order(OrderType::MARKET);
//     TraderOrder::new_order(rpc_request).0
// }

// fn create_sample_orders(count: usize) -> Vec<TraderOrder> {
//     (0..count)
//         .map(|i| {
//             let rpc_request = CreateTraderOrder {
//                 account_id: format!("test_account_{}", i),
//                 position_type: if i % 2 == 0 {
//                     PositionType::LONG
//                 } else {
//                     PositionType::SHORT
//                 },
//                 order_type: if i % 3 == 0 {
//                     OrderType::MARKET
//                 } else {
//                     OrderType::LIMIT
//                 },
//                 leverage: 10.0 + (i as f64),
//                 initial_margin: 100.0 + (i as f64 * 10.0),
//                 available_margin: 100.0 + (i as f64 * 10.0),
//                 order_status: OrderStatus::PENDING,
//                 entryprice: 50000.0 + (i as f64 * 100.0),
//                 execution_price: 50000.0 + (i as f64 * 100.0),
//             };
//             TraderOrder::new_order(rpc_request).0
//         })
//         .collect()
// }

// fn process_orders_batch(orders: Vec<TraderOrder>) -> Vec<String> {
//     // Simple processing simulation - serialize all orders
//     orders.into_iter().map(|order| order.serialize()).collect()
// }

// criterion_group!(
//     benches,
//     benchmark_order_creation,
//     benchmark_order_processing_throughput,
//     benchmark_order_serialization
// );
// criterion_main!(benches);
