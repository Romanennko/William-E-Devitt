use dotenv::dotenv;
use sqlx::sqlite::SqlitePool;
use teloxide::{dispatching::dialogue::InMemStorage, prelude::*, types::UserId};

mod commands;
mod db;
mod models;
mod utils;

use commands::{Command, answer_command, photo::handle_photo};
use models::State;

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();
    log::info!("Starting command bot...");

    let allowed_user_id = std::env::var("ALLOWED_USER_ID")
        .expect("ALLOWED_USER_ID must be set")
        .parse::<u64>()
        .expect("ALLOWED_USER_ID must be a valid u64");
    let allowed_user = UserId(allowed_user_id);

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = SqlitePool::connect(&db_url)
        .await
        .expect("Failed to connect to the database");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    let bot = Bot::from_env();

    // 1. Build the routing tree
    let handler = Update::filter_message()
        // Filter messages to only allow the specified user ID
        .filter(move |msg: Message| msg.from.as_ref().map(|u| u.id) == Some(allowed_user))
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
        .dependencies(dptree::deps![InMemStorage::<State>::new(), pool])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
