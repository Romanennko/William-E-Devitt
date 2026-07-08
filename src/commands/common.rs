use sqlx::SqlitePool;
use teloxide::prelude::*;

use super::AppError;
use crate::db;

pub async fn resolve_user_id(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
) -> Result<Option<i64>, AppError> {
    let tg_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    match db::find_user_id(pool, tg_id).await? {
        Some(id) => Ok(Some(id)),
        None => {
            bot.send_message(
                msg.chat.id,
                "No data yet. Send a receipt with /photo first!",
            )
            .await?;
            Ok(None)
        }
    }
}
