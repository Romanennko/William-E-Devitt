use base64::prelude::*;
use teloxide::dispatching::dialogue::{Dialogue, InMemStorage};
use teloxide::{net::Download, prelude::*, utils::command::BotCommands};

pub type MyDialogue = Dialogue<State, InMemStorage<State>>;

pub type AppError = Box<dyn std::error::Error + Send + Sync>;
pub type HandlerResult = Result<(), AppError>;

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

            let mut buffer = Vec::new();
            bot.download_file(&telegram_file.path, &mut buffer).await?;

            let b64_image = BASE64_STANDARD.encode(&buffer);

            let gemini_response = ask_gemini(&b64_image).await;

            match gemini_response {
                Ok(json_str) => {
                    // Here, you must deserialize the `json_str` into your structure
                    bot.send_message(msg.chat.id, format!("Result:\n{}", json_str))
                        .await?;
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("Error: {}", e))
                        .await?;
                }
            }

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

    dialogue.exit().await?;

    Ok(())
}

async fn ask_gemini(b64_image: &str) -> Result<String, AppError> {
    let api_key = std::env::var("GEMINI_API_KEY").expect("Where's the API key, Lebowski?");
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}",
        api_key
    );

    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "contents": [{
            "parts": [
                { "text": "Extract items from receipt. Return raw JSON without markdown. Schema: { total: f64, items: [{name: str, price: f64, category: str}] }" },
                { "inline_data": { "mime_type": "image/jpeg", "data": b64_image } }
            ]
        }],
        "generationConfig": { "responseMimeType": "application/json" }
    });

    let res = client.post(&url).json(&payload).send().await?;

    let res_json: serde_json::Value = res.json().await?;

    let extracted_text = res_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("{}")
        .to_string();

    Ok(extracted_text)
}
