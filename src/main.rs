use dotenv::dotenv;
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*};

mod commands;

use commands::{Command, State, answer_command, handle_photo};

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let bot = Bot::from_env();

    // 1. Build the routing tree
    let handler = Update::filter_message()
        // Inject dialogue storage into the context
        .enter_dialogue::<Message, InMemStorage<State>, State>()
        .branch(
            // If state is Start, look for a command and route to answer_command
            dptree::case![State::Start]
                .filter_command::<Command>()
                .endpoint(answer_command),
        )
        .branch(
            // If state is ReceivePhoto, route everything to handle_photo
            dptree::case![State::ReceivePhoto].endpoint(handle_photo),
        );

    // 2. Build and start the Dispatcher
    Dispatcher::builder(bot, handler)
        // This is where we provide the actual memory storage engine
        .dependencies(dptree::deps![InMemStorage::<State>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
