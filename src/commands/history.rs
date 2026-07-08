use sqlx::SqlitePool;
use teloxide::prelude::*;

use super::HandlerResult;
use super::common::resolve_user_id;
use crate::db;

pub async fn show_history(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let receipts = db::get_recent_receipts(pool, user_id, 10).await?;

    if receipts.is_empty() {
        bot.send_message(msg.chat.id, "\u{1F4CB} No receipts found.")
            .await?;
        return Ok(());
    }

    let mut text = String::from("\u{1F4CB} Recent receipts:\n\n");

    for (i, r) in receipts.iter().enumerate() {
        let date_display = r.receipt_date.get(..10).unwrap_or(&r.receipt_date);
        text.push_str(&format!(
            "{}. {} \u{2014} {:.2} PLN\n   \u{1F4C5} {}\n\n",
            i + 1,
            r.shop_name,
            r.total_sum,
            date_display,
        ));
    }

    bot.send_message(msg.chat.id, text).await?;
    Ok(())
}

pub async fn delete_last(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    match db::delete_last_receipt(pool, user_id).await? {
        Some(receipt) => {
            let date_display = receipt
                .receipt_date
                .get(..10)
                .unwrap_or(&receipt.receipt_date);
            bot.send_message(
                msg.chat.id,
                format!(
                    "\u{1F5D1}\u{FE0F} Deleted last receipt:\n{} \u{2014} {:.2} PLN ({})",
                    receipt.shop_name, receipt.total_sum, date_display
                ),
            )
            .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Nothing to delete.").await?;
        }
    }

    Ok(())
}
