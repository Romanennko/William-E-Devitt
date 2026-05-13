use teloxide::{
    dispatching::dialogue::InMemStorage, net::Download, prelude::*, utils::command::BotCommands,
};

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceivePhoto,
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    #[command(description = "display all available commands.")]
    Help,
    #[command(description = "upload a photo.")]
    Upload,
    #[command(description = "check expenses")]
    Check,
}

pub async fn answer_command(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    cmd: Command,
) -> HandlerResult {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Upload => {
            bot.send_message(msg.chat.id, "Please send a photo to upload.")
                .await?;
            dialogue.update(State::ReceivePhoto).await?;
        }
        Command::Check => {
            bot.send_message(msg.chat.id, "Checking expenses...")
                .await?;
        }
    }
    Ok(())
}

pub async fn handle_photo(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    if let Some(photos) = msg.photo() {
        if let Some(best_photo) = photos.last() {
            let telegram_file = bot.get_file(best_photo.file.id.clone()).await?;
            let mut local_file = tokio::fs::File::create("downloaded_photo.jpg").await?;

            bot.download_file(&telegram_file.path, &mut local_file)
                .await?;

            bot.send_message(msg.chat.id, "Got the photo! Downloaded successfully.")
                .await?;

            // Reset the state back to Start so they can use commands again
            dialogue.exit().await?;
        }
    } else {
        // If they send text instead of a photo while in the ReceivePhoto state:
        bot.send_message(
            msg.chat.id,
            "That's not a photo, dipshit. Send an actual photo.",
        )
        .await?;
    }

    Ok(())
}
