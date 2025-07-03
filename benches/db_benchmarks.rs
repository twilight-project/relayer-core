// use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
// use std::collections::HashMap;
// use twilight_relayer_rust::relayer::*;
// use uuid::Uuid;

// fn benchmark_order_storage(c: &mut Criterion) {
//     let mut group = c.benchmark_group("order_storage");

//     // Benchmark inserting orders into HashMap (simulating in-memory storage)
//     let order_counts = vec![10, 50, 100];

//     for count in order_counts {
//         group.bench_with_input(
//             BenchmarkId::new("insert_orders", count),
//             &count,
//             |b, &count| {
//                 b.iter(|| {
//                     let orders = create_sample_orders(count);
//                     black_box(insert_orders_hashmap(orders));
//                 });
//             },
//         );
//     }

//     group.finish();
// }

// fn benchmark_order_retrieval(c: &mut Criterion) {
//     let mut group = c.benchmark_group("order_retrieval");

//     // Pre-populate with orders
//     let orders = create_sample_orders(1000);
//     let order_map = insert_orders_hashmap(orders.clone());
//     let order_ids: Vec<Uuid> = orders.iter().map(|o| o.uuid).collect();

//     group.bench_function("get_order_by_id", |b| {
//         b.iter(|| {
//             let id = &order_ids[black_box(42) % order_ids.len()];
//             black_box(order_map.get(id));
//         });
//     });

//     group.finish();
// }

// fn benchmark_order_serialization_batch(c: &mut Criterion) {
//     let mut group = c.benchmark_group("serialization_batch");

//     let batch_sizes = vec![10, 50, 100];

//     for size in batch_sizes {
//         group.bench_with_input(
//             BenchmarkId::new("serialize_orders", size),
//             &size,
//             |b, &size| {
//                 let orders = create_sample_orders(size);
//                 b.iter(|| {
//                     black_box(serialize_orders_batch(orders.clone()));
//                 });
//             },
//         );
//     }

//     group.finish();
// }

// // Helper functions
// fn create_sample_orders(count: usize) -> Vec<TraderOrder> {
//     (0..count)
//         .map(|i| {
//             let rpc_request = CreateTraderOrder {
//                 account_id: format!("bench_account_{}", i),
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
//                 leverage: 5.0 + (i as f64 % 20.0),
//                 initial_margin: 50.0 + (i as f64 * 5.0),
//                 available_margin: 50.0 + (i as f64 * 5.0),
//                 order_status: OrderStatus::PENDING,
//                 entryprice: 45000.0 + (i as f64 * 50.0),
//                 execution_price: 45000.0 + (i as f64 * 50.0),
//             };
//             TraderOrder::new_order(rpc_request).0
//         })
//         .collect()
// }

// fn insert_orders_hashmap(orders: Vec<TraderOrder>) -> HashMap<Uuid, TraderOrder> {
//     let mut map = HashMap::new();
//     for order in orders {
//         map.insert(order.uuid, order);
//     }
//     map
// }

// fn serialize_orders_batch(orders: Vec<TraderOrder>) -> Vec<String> {
//     orders.into_iter().map(|order| order.serialize()).collect()
// }

// criterion_group!(
//     benches,
//     benchmark_order_storage,
//     benchmark_order_retrieval,
//     benchmark_order_serialization_batch
// );
// criterion_main!(benches);
