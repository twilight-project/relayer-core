use amiquip::{Connection, ConsumerMessage, ConsumerOptions, QueueDeclareOptions, Result};

fn consumer_main() -> Result<()> {
    // Open connection.
    let mut connection = Connection::insecure_open("amqp://guest:guest@localhost:5672")?;

    // Open a channel - None says let the library choose the channel ID.
    let channel = connection.open_channel(None)?;

    // Declare the "hello" queue.
    let queue = channel.queue_declare("hello", QueueDeclareOptions::default())?;

    // Start a consumer.
    let consumer = queue.consume(ConsumerOptions::default())?;
    println!("Waiting for messages. Press Ctrl-C to exit.");

    for (i, message) in consumer.receiver().iter().enumerate() {
        match message {
            ConsumerMessage::Delivery(delivery) => {
                let body = String::from_utf8_lossy(&delivery.body);
                println!("({:>3}) Received [{}]", i, body);
                consumer.ack(delivery)?;
            }
            other => {
                println!("Consumer ended: {:?}", other);
                break;
            }
        }
    }

    connection.close()
}

// A Connection is effectively bound to a single thread (it technically implements both Send and Sync, but most relevant methods take &mut self). A connection can open many Channels; a channel can only be used by a single thread (it implements Send but not Sync). There is no tie between a connection and its channels at the type system level; if the connection is closed (either intentionally or because of an error), all the channels that it opened will end up returning errors shortly thereafter. See the discussion on Connection::close for a bit more information about that.

// A channel is able to produce other handles (queues, exchanges, and consumers). These are mostly thin convenience wrappers around the channel, and they do hold a reference back to the channel that created them. This means if you want to use the connection to open a channel on one thread then move it to another thread to do work, you will need to declare queues, exchanges, and consumers from the thread where work will be done; e.g.,

// use amiquip::{Connection, QueueDeclareOptions, ConsumerOptions, Result};
// use std::thread;

// fn run_connection(mut connection: Connection) -> Result<()> {
//     let channel = connection.open_channel(None)?;

//     // Declaring the queue outside the thread spawn will fail, as it cannot
//     // be moved into the thread. Instead, wait to declare until inside the new thread.

//     // Would fail:
//     // let queue = channel.queue_declare("hello", QueueDeclareOptions::default())?;
//     thread::spawn(move || -> Result<()> {
//         // Instead, declare once the channel is moved into this thread.
//         let queue = channel.queue_declare("hello", QueueDeclareOptions::default())?;
//         let consumer = queue.consume(ConsumerOptions::default())?;
//         for message in consumer.receiver().iter() {
//             // do something with message...
//         }
//         Ok(())
//     });

//     // do something to keep the connection open; if we drop the connection here,
//     // it will be closed, killing the channel that we just moved into a new thread
// }
