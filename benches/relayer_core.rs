// benches/relayer_core.rs
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use crossbeam_channel as cb;
use dotenv::dotenv;
use once_cell::sync::Lazy;
use twilight_relayer_rust::{db::*, relayer::*, *};
static META: Lazy<Meta> = Lazy::new(|| Meta {
    metadata: Default::default(),
});
// {\"metadata\":{\"CONTENT_TYPE\":\"application/json\",\"request_server_time\":\"1751026225117829\",\"Relayer\":null}
static INIT_ENV: Lazy<()> = Lazy::new(|| {
    // Look for .env in the crate root; ignore "file not found" errors.
    dotenv().ok();
});
fn sample_create_trader_order() -> CreateTraderOrder {
    CreateTraderOrder {
        account_id: "bench-acct".into(),
        position_type: PositionType::LONG,
        order_type: OrderType::MARKET,
        entryprice: 45_000.0,
        execution_price: 0.0,
        leverage: 10.0,
        initial_margin: 100.0,
        available_margin: 100.0, // <- new field
        order_status: OrderStatus::PENDING,
    }
}

fn sample_trader_order() -> TraderOrder {
    TraderOrder::new_order(sample_create_trader_order()).0
}

fn dummy_private_cmd() -> RpcCommand {
    RpcCommand::CreateTraderOrder(
        sample_create_trader_order(),
        META.clone(),
        ZkosHexString::from("010000000100000000000000000000000100000000000000000000000000000001010101000000000000000000000000000000be583b10118274ff83898478803493f8b5c9f7aff74b47803d005aa5b8e5ff34008890cfffd3c69225c899c8f4d38e2bf27d402bb12f68d0e4d2cb5c2fa057a87634e09e739747339e43a3b2a5f16ef382e705129ebd32e5eab33eb487e23d5f0d8a0000000000000030633238643230396331346633643330396332316431646635636666656665386236353763663466636235316465346666336162363162613330353737646336346231343063373464346539313339363266666336326539646632323665656135613761383134333564353636316130386530323962376365643535303337333635643139376138366300010000000000000001000000010000002a000000000000003138323237323664346265336336623333623166333434633734333263626530343230333861663162388a00000000000000306332386432303963313466336433303963323164316466356366666566653862363537636634666362353164653466663361623631626133303537376463363462313430633734643465393133393632666663363265396466323236656561356137613831343335643536363161303865303239623763656435353033373336356431393761383663010000000000000064000000000000000000000000000000e957acb8f69eb51764fac73a360f33e1541f0011313078264a45ef5fea9ce60b010400000000000000030000000100000000000000000000000000000000000000000000000000000000000000000000000200000001000000000000000a000000000000000000000000000000e957acb8f69eb51764fac73a360f33e1541f0011313078264a45ef5fea9ce60b030000000100000000000000000000000000000000000000000000000000000000000000000000000300000001000000ecd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010010000001600000000000000060a0403000000060a0405000000060a0d0e1302020200000000010000000000000003000000000000003d107ff89a1f0387ca226522f82bca1fbf43059a200c49bdae998400bfe89ea52c92d3c5e02c98dc6b6f1f91cfd42e136d1d4f1de6bdb351b12be20f9535960c7c1b7c42e22f98283d98314b914cdbfc380ef2bb1c3e3a91685a05614fc4cf6da10100000000000000081f634c63bf2608f26ad902b0c5b40054c8fd373105bdc52c287a2487346976c4841881453bbd43c4dd48ba90e7c3711d13492ef27be454a7a009fc70a64754faf4c66a735876d23ab6da6a59467022f5d65f55eeca5079876316c6677931379a5c7d02f3ffca1f71381ecdfc0ab3941e6bc3abab05b0722ed1a7dad3556326d8eece4b05ab25266ff65d512b8289befd6f468348bee4ae8e229a134deb425ec2d864931da5366cca6b6255b8085ffe84cc96fbf42fef2f7ecf0ac5f3443703ece221e62ff80272ca9189c60c36cf1fb3e228b656dd71d77a44f97765914d78e4122aa0fccc966ae809cb9eff9dd3bccbe35a1ea6c2b9f3bb5e492584af2f12c8260448264aa871032db89e606b5f9697226086d312cc1803da2939ad6a9e06522e2ca3826c0388b818049c7acffb91f29a3fb0cbd1247fdf5473fede4f480dd7acd672356877def64706d40dee6c2939ae8d9818a0bf39d59917b3889b7a027b8f846799f811620723b3496b825b9d2396d54e5ff264fde74b84c5735427085fd45812244097a4124449b01fa78fbe7f1e629c867a3e59874f31b4ef0b80030100000000000000020000004000000000000000cca3d920e11a001e32ab1cba5578e5692fe3ab4183bd8b53560658ddde4bf036a13b8161f0d1ab853b8d1a4b782f8c43e709ea102ed576ea21b6bd961ecd850a010000000100000000000000337e29a32dbb048c10411454256831a37b812d87113312e6055ca4fc286e850a01000000000000007b72c28475b0f361f137117037c86e1567b2ab31aa9e81c3ae7e919a2c93cb07000000000000000060b7cdf3a0023b48c957a5a6f44054425a0a6e846d6969656ada920e0d841b0f01020000000100000000000000e803000000000000000000000000000073921625e3f5a03544167ed9e80419c09ce8702032546e88a1b2ddf6540c7e0d"),
        "req-123".into(),
    )
}

fn sample_create_zkos_command() -> ZkosTxCommand {
    ZkosTxCommand::CreateTraderOrderTX(sample_trader_order(), dummy_private_cmd())
}
// -------- full core pipeline ----------
fn bench_core_pipeline(c: &mut Criterion) {
    Lazy::force(&INIT_ENV);
    let (tx, _rx) = cb::unbounded();
    let offset = (0, 0); // OffsetCompletion alias

    c.bench_function("core_pipeline_market_order", |b| {
        b.iter(|| rpc_event_handler(black_box(dummy_private_cmd()), tx.clone(), offset))
    });
}

// -------- zkos pipeline ----------
fn bench_zkos_pipeline(c: &mut Criterion) {
    Lazy::force(&INIT_ENV);
    // let (tx, _rx) = cb::unbounded();
    let (sender, zkos_receiver) = std::sync::mpsc::channel();
    let offset = (0, 0); // OffsetCompletion alias

    c.bench_function("zkos_pipeline_market_order", |b| {
        b.iter(|| zkos_order_handler(black_box(sample_create_zkos_command()), sender.clone()))
    });
}

// -------- LocalDB hot path ----------
fn bench_localdb_ops(c: &mut Criterion) {
    c.bench_function("localdb_add_get_remove", |b| {
        b.iter_batched(
            || {
                let mut db: OrderDB<TraderOrder> = OrderDB::new();
                let order = TraderOrder::new_order(sample_create_trader_order()).0;
                (db, order)
            },
            |(mut db, order)| {
                db.add(order.clone(), dummy_private_cmd()); // add
                db.get(order.uuid).unwrap(); // get
                db.remove(order, dummy_private_cmd()).unwrap(); // remove
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_core_pipeline,
    bench_localdb_ops,
    bench_zkos_pipeline
);
criterion_main!(benches);
