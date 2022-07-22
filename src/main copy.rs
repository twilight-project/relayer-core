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

extern crate rql;
use rql::prelude::*;

schema! {
    #[derive(Debug)]
    MySchema {
        traderorder: TraderOrder,
    }
}

fn main() {
    let db = MySchema::new("my_db", HumanReadable).unwrap();
    let traderorder1=TraderOrder::deserialize(&"{\"uuid\":\"22f940be-79ca-4365-8ec2-d93f3e8d6233\",\"account_id\":\"test order\",\"position_type\":\"LONG\",\"order_status\":\"FILLED\",\"order_type\":\"MARKET\",\"entryprice\":20000.0,\"execution_price\":20000.0,\"positionsize\":200000.0,\"leverage\":10.0,\"initial_margin\":1.0,\"available_margin\":1.0,\"timestamp\":{\"secs_since_epoch\":1657919055,\"nanos_since_epoch\":663796000},\"bankruptcy_price\":18181.81818181818,\"bankruptcy_value\":11.000000000000002,\"maintenance_margin\":4400.040000000001,\"liquidation_price\":-45.568051327853006,\"unrealized_pnl\":0.0,\"settlement_price\":0.0,\"entry_nonce\":3,\"exit_nonce\":0,\"entry_sequence\":1}".to_string());
    thread::sleep(time::Duration::from_millis(5000));
    let sw = Stopwatch::start_new();
    let dbc = MySchema::new("my_db", BinaryStable).unwrap();
    let mut ordertb = Id::new();
    for i in 0..5000 {
        let tc = traderorder1.clone();
        ordertb = dbc.traderorder_mut().insert(tc);
    }
    println!("pool took {:#?}", sw.elapsed());
    println!("{:#?}", db.traderorder().len());
    println!("Im done");
    thread::sleep(time::Duration::from_millis(10000));
    let dbc = MySchema::new("my_db", BinaryStable).unwrap();
    let x: TraderOrder = dbc.traderorder().get(ordertb).unwrap().clone();
    println!("{:#?}", db.traderorder().len());
    println!("{:#?}", x);
}

#[derive(Serialize, Deserialize)]
struct User {
    name: String,
    age: u8,
}

#[derive(Serialize, Deserialize)]
struct Group {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct Member {
    user_id: Id<User>,
    group_id: Id<Group>,
    permission: bool,
}

schema! {
    MySchema {
        user: User,
        group: Group,
        member: Member,
    }
}
fn main() {
    // Create a new database with the previously defined schema
    // We pass a folder name for the database files as well as a representation type
    let sw = Stopwatch::start_new();
    let db = MySchema::new("my_db", HumanReadable).unwrap();

    // Insert values into the database
    // Insertion returns the new row's id
    let dan = db.user_mut().insert(User {
        name: "Dan".into(),
        age: 25,
    });
    let steve = db.user_mut().insert(User {
        name: "Steve".into(),
        age: 39,
    });
    let mary = db.user_mut().insert(User {
        name: "Mary".into(),
        age: 31,
    });

    let admin = db.group_mut().insert(Group {
        name: "Admin".into(),
    });
    let normal = db.group_mut().insert(Group {
        name: "Normal User".into(),
    });

    db.member_mut().insert(Member {
        user_id: dan,
        group_id: admin,
        permission: true,
    });
    db.member_mut().insert(Member {
        user_id: steve,
        group_id: normal,
        permission: true,
    });
    db.member_mut().insert(Member {
        user_id: mary,
        group_id: normal,
        permission: false,
    });

    // Data can easily be looked up by id
    db.user_mut().get_mut(dan).unwrap().age += 1;
    let dan_age = db.user().get(dan).unwrap().age;
    assert_eq!(dan_age, 26);

    // Data can be selected from a table
    let ages: Vec<u8> = db.user().select(|user| user.age).collect();

    // Use `wher` to filter entries
    let can_run_for_president: Vec<String> = db
        .user()
        .wher(|user| user.age >= 35)
        .select(|user| user.name.clone())
        .collect();

    // Table intersections are done using `relate`
    // A function relating the tables is required
    for (user, permission) in db
        .user()
        .relate(&*db.member(), |user, member| {
            user.id == member.user_id && member.group_id == normal
        })
        .select(|(user, member)| (&user.data.name, member.permission))
    {
        println!("{} is a normal user with permission = {}", user, permission);
    }

    // Rows can be updated with `update`
    for mut user in db.user_mut().update() {
        user.age += 1;
    }

    // Rows can be deleted in a few ways

    // By id
    db.user_mut().delete_one(steve);

    // With a where clause
    db.member_mut().delete_where(|member| member.permission);

    // With an iterator over ids
    db.user_mut().delete_iter(|_| vec![dan, mary]);

    // Changes to the database are automatically saved, so they can be loaded again
    let db_copy = MySchema::new("my_db", HumanReadable).unwrap();
    assert_eq!(db.user().len(), db_copy.user().len());
    assert_eq!(db.group().len(), db_copy.group().len());
    assert_eq!(db.member().len(), db_copy.member().len());
    println!("pool took {:#?}", sw.elapsed());
}

// let traderorder=TraderOrder::deserialize(&"{\"uuid\":\"22f940be-79ca-4365-8ec2-d93f3e8d6233\",\"account_id\":\"test order\",\"position_type\":\"LONG\",\"order_status\":\"FILLED\",\"order_type\":\"MARKET\",\"entryprice\":20000.0,\"execution_price\":20000.0,\"positionsize\":200000.0,\"leverage\":10.0,\"initial_margin\":1.0,\"available_margin\":1.0,\"timestamp\":{\"secs_since_epoch\":1657919055,\"nanos_since_epoch\":663796000},\"bankruptcy_price\":18181.81818181818,\"bankruptcy_value\":11.000000000000002,\"maintenance_margin\":4400.040000000001,\"liquidation_price\":-45.568051327853006,\"unrealized_pnl\":0.0,\"settlement_price\":0.0,\"entry_nonce\":3,\"exit_nonce\":0,\"entry_sequence\":1}".to_string());
// }
