pub mod common;
pub mod history;
pub mod photo;
pub mod stats;

use sqlx::SqlitePool;
use teloxide::dispatching::dialogue::{Dialogue, InMemStorage};
use teloxide::{prelude::*, utils::command::BotCommands};

use crate::models::State;

pub type MyDialogue = Dialogue<State, InMemStorage<State>>;
pub type AppError = Box<dyn std::error::Error + Send + Sync>;
pub type HandlerResult = Result<(), AppError>;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    #[command(description = "display all available commands.")]
    Help,
    #[command(description = "upload a photo.")]
    Photo,
    #[command(description = "check expenses for the current month.")]
    Check,
    #[command(description = "show recent receipts.")]
    History,
    #[command(description = "show monthly statistics.")]
    Stats,
    #[command(description = "delete last receipt.")]
    Delete,
    #[command(description = "expenses for a specific month (YYYY-MM).")]
    Month(String),
    #[command(description = "items in a category (e.g. /category groceries).")]
    Category(String),
    #[command(description = "spending trend over 6 months.")]
    Trend,
    #[command(description = "top spending insights.")]
    Top,
}

pub async fn answer_command(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    cmd: Command,
    pool: SqlitePool,
) -> HandlerResult {
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Photo => {
            bot.send_message(msg.chat.id, "Please send a photo to upload.")
                .await?;
            dialogue.update(State::ReceivePhoto).await?;
        }
        Command::Check => {
            stats::check_expenses(&bot, &msg, &pool).await?;
        }
        Command::History => {
            history::show_history(&bot, &msg, &pool).await?;
        }
        Command::Stats => {
            stats::show_stats(&bot, &msg, &pool).await?;
        }
        Command::Delete => {
            history::delete_last(&bot, &msg, &pool).await?;
        }
        Command::Month(arg) => {
            stats::show_month(&bot, &msg, &pool, arg).await?;
        }
        Command::Category(arg) => {
            stats::show_category(&bot, &msg, &pool, arg).await?;
        }
        Command::Trend => {
            stats::show_trend(&bot, &msg, &pool).await?;
        }
        Command::Top => {
            stats::show_top(&bot, &msg, &pool).await?;
        }
    }
    Ok(())
}
