use chrono::Datelike;
use sqlx::SqlitePool;
use teloxide::prelude::*;

use super::HandlerResult;
use super::common::resolve_user_id;
use crate::db;
use crate::utils::{
    VALID_CATEGORIES, category_emoji, category_label, month_name, month_name_short,
    normalize_category,
};

pub async fn check_expenses(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let now = chrono::Local::now();
    let year = now.year();
    let month = now.month();

    let categories = db::get_expenses_by_category(pool, user_id, year, month).await?;
    let summary = db::get_month_summary(pool, user_id, year, month).await?;

    if categories.is_empty() {
        bot.send_message(
            msg.chat.id,
            format!(
                "\u{1F4CA} No expenses for {} {} yet.",
                month_name(month),
                year
            ),
        )
        .await?;
        return Ok(());
    }

    let mut text = format!(
        "\u{1F4CA} Expenses \u{2014} {} {}\n\n",
        month_name(month),
        year
    );

    for cat in &categories {
        text.push_str(&format!(
            "{} {}: {:.2} PLN ({} items)\n",
            category_emoji(&cat.category),
            category_label(&cat.category),
            cat.total,
            cat.item_count,
        ));
    }

    text.push_str("\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n");
    text.push_str(&format!(
        "\u{1F4B0} Total: {:.2} PLN ({} receipts)\n",
        summary.items_total, summary.receipt_count
    ));

    if summary.items_total > 0.0 {
        let junk_pct = (summary.junk_total / summary.items_total) * 100.0;
        text.push_str(&format!(
            "\u{1F355} Junk food: {:.2} PLN ({:.1}%)",
            summary.junk_total, junk_pct
        ));
    }

    bot.send_message(msg.chat.id, text).await?;
    Ok(())
}

pub async fn show_stats(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let now = chrono::Local::now();
    let cur_year = now.year();
    let cur_month = now.month();
    let (prev_year, prev_month) = if cur_month == 1 {
        (cur_year - 1, 12u32)
    } else {
        (cur_year, cur_month - 1)
    };

    let cur = db::get_month_summary(pool, user_id, cur_year, cur_month).await?;
    let prev = db::get_month_summary(pool, user_id, prev_year, prev_month).await?;

    let mut text = String::from("\u{1F4C8} Statistics\n\n");

    text.push_str(&format!(
        "\u{1F4C5} {} {}: {:.2} PLN ({} receipts)\n",
        month_name(cur_month),
        cur_year,
        cur.total_spent,
        cur.receipt_count
    ));
    text.push_str(&format!(
        "\u{1F4C5} {} {}: {:.2} PLN ({} receipts)\n",
        month_name(prev_month),
        prev_year,
        prev.total_spent,
        prev.receipt_count
    ));

    if prev.total_spent > 0.0 {
        let change = ((cur.total_spent - prev.total_spent) / prev.total_spent) * 100.0;
        let arrow = if change >= 0.0 {
            "\u{1F4C8}"
        } else {
            "\u{1F4C9}"
        };
        text.push_str(&format!("{} Change: {:+.1}%\n", arrow, change));
    }

    text.push('\n');

    if cur.receipt_count > 0 {
        let avg = cur.total_spent / cur.receipt_count as f64;
        text.push_str(&format!("\u{1F4B0} Avg receipt: {:.2} PLN\n", avg));
    }

    if cur.items_total > 0.0 {
        let junk_pct = (cur.junk_total / cur.items_total) * 100.0;
        text.push_str(&format!(
            "\u{1F355} Junk food: {:.2} PLN ({:.1}%)\n",
            cur.junk_total, junk_pct
        ));
    }

    let categories = db::get_expenses_by_category(pool, user_id, cur_year, cur_month).await?;
    if !categories.is_empty() {
        text.push_str("\n\u{1F3C6} Top categories:\n");
        for (i, cat) in categories.iter().take(3).enumerate() {
            text.push_str(&format!(
                "  {}. {} {} \u{2014} {:.2} PLN\n",
                i + 1,
                category_emoji(&cat.category),
                category_label(&cat.category),
                cat.total,
            ));
        }
    }

    bot.send_message(msg.chat.id, text).await?;
    Ok(())
}

pub async fn show_month(bot: &Bot, msg: &Message, pool: &SqlitePool, arg: String) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let arg = arg.trim().to_string();
    if arg.is_empty() {
        bot.send_message(
            msg.chat.id,
            "Usage: /month YYYY-MM\nExample: /month 2026-07",
        )
        .await?;
        return Ok(());
    }

    let parts: Vec<&str> = arg.split('-').collect();
    if parts.len() != 2 {
        bot.send_message(msg.chat.id, "Invalid format. Use: /month YYYY-MM")
            .await?;
        return Ok(());
    }

    let year: i32 = parts[0].parse().unwrap_or(0);
    let month: u32 = parts[1].parse().unwrap_or(0);
    if !(2020..=2100).contains(&year) || !(1..=12).contains(&month) {
        bot.send_message(msg.chat.id, "Invalid date. Use: /month YYYY-MM")
            .await?;
        return Ok(());
    }

    let categories = db::get_expenses_by_category(pool, user_id, year, month).await?;
    let summary = db::get_month_summary(pool, user_id, year, month).await?;

    if categories.is_empty() {
        bot.send_message(
            msg.chat.id,
            format!(
                "\u{1F4CA} No expenses for {} {} yet.",
                month_name(month),
                year
            ),
        )
        .await?;
        return Ok(());
    }

    let mut text = format!(
        "\u{1F4CA} Expenses \u{2014} {} {}\n\n",
        month_name(month),
        year
    );

    for cat in &categories {
        text.push_str(&format!(
            "{} {}: {:.2} PLN ({} items)\n",
            category_emoji(&cat.category),
            category_label(&cat.category),
            cat.total,
            cat.item_count,
        ));
    }

    text.push_str("\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n");
    text.push_str(&format!(
        "\u{1F4B0} Total: {:.2} PLN ({} receipts)\n",
        summary.items_total, summary.receipt_count
    ));

    if summary.items_total > 0.0 {
        let junk_pct = (summary.junk_total / summary.items_total) * 100.0;
        text.push_str(&format!(
            "\u{1F355} Junk food: {:.2} PLN ({:.1}%)",
            summary.junk_total, junk_pct
        ));
    }

    bot.send_message(msg.chat.id, text).await?;
    Ok(())
}

pub async fn show_category(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
    arg: String,
) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let arg = arg.trim().to_string();
    if arg.is_empty() {
        let mut text = String::from("Usage: /category <name>\n\nAvailable categories:\n");
        for &cat in VALID_CATEGORIES {
            text.push_str(&format!(
                "  {} {}\n",
                category_emoji(cat),
                cat.to_lowercase()
            ));
        }
        bot.send_message(msg.chat.id, text).await?;
        return Ok(());
    }

    let category = match normalize_category(&arg) {
        Some(c) => c,
        None => {
            let mut text = format!("Unknown category: \"{}\"\n\nAvailable:\n", arg);
            for &cat in VALID_CATEGORIES {
                text.push_str(&format!(
                    "  {} {}\n",
                    category_emoji(cat),
                    cat.to_lowercase()
                ));
            }
            bot.send_message(msg.chat.id, text).await?;
            return Ok(());
        }
    };

    let now = chrono::Local::now();
    let year = now.year();
    let month = now.month();

    let items = db::get_category_items(pool, user_id, year, month, category).await?;

    if items.is_empty() {
        bot.send_message(
            msg.chat.id,
            format!(
                "{} No {} expenses for {} {}.",
                category_emoji(category),
                category_label(category),
                month_name(month),
                year
            ),
        )
        .await?;
        return Ok(());
    }

    let total: f64 = items.iter().map(|i| i.price).sum();
    let junk_count = items.iter().filter(|i| i.is_junk_food).count();

    let mut text = format!(
        "{} {} \u{2014} {} {}\n\n",
        category_emoji(category),
        category_label(category),
        month_name(month),
        year
    );

    for item in &items {
        let date_display = item.receipt_date.get(..10).unwrap_or(&item.receipt_date);
        let junk_marker = if item.is_junk_food { " \u{1F355}" } else { "" };
        text.push_str(&format!(
            "\u{2022} {} \u{2014} {:.2} PLN{}\n  {} | {}\n",
            item.item_name, item.price, junk_marker, item.shop_name, date_display
        ));
    }

    text.push_str(&format!(
        "\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F4B0} Total: {:.2} PLN ({} items)",
        total,
        items.len()
    ));

    if junk_count > 0 {
        text.push_str(&format!("\n\u{1F355} Junk food items: {}", junk_count));
    }

    bot.send_message(msg.chat.id, text).await?;
    Ok(())
}

pub async fn show_trend(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let now = chrono::Local::now();
    let cur_month = now.month() as i32;
    let cur_year = now.year();

    // Go back 5 months to show 6 months including current
    let mut start_month = cur_month - 5;
    let mut start_year = cur_year;
    while start_month <= 0 {
        start_month += 12;
        start_year -= 1;
    }
    let start_date = format!("{:04}-{:02}-01 00:00:00", start_year, start_month);

    let trend = db::get_monthly_trend(pool, user_id, &start_date).await?;

    if trend.is_empty() {
        bot.send_message(msg.chat.id, "\u{1F4CA} No spending data yet.")
            .await?;
        return Ok(());
    }

    let max_total = trend.iter().map(|t| t.total).fold(0.0f64, f64::max);
    let bar_width: usize = 12;

    let mut text = String::from("\u{1F4CA} Spending trend\n\n");

    for entry in &trend {
        let bar_len = if max_total > 0.0 {
            ((entry.total / max_total) * bar_width as f64).round() as usize
        } else {
            0
        };
        let bar = "\u{2588}".repeat(bar_len);
        let empty = "\u{2591}".repeat(bar_width - bar_len);

        text.push_str(&format!(
            "{} {} {}{} {:>8.0} PLN ({})\n",
            month_name_short(entry.month),
            entry.year,
            bar,
            empty,
            entry.total,
            entry.receipt_count
        ));
    }

    let grand_total: f64 = trend.iter().map(|t| t.total).sum();
    let total_receipts: i64 = trend.iter().map(|t| t.receipt_count).sum();
    let avg = grand_total / trend.len() as f64;
    text.push_str(&format!(
        "\n\u{1F4B0} Avg: {:.2} PLN/month ({} receipts total)",
        avg, total_receipts
    ));

    if trend.len() >= 2 {
        let first = trend.first().unwrap().total;
        let last = trend.last().unwrap().total;
        if first > 0.0 {
            let change = ((last - first) / first) * 100.0;
            let arrow = if change >= 0.0 {
                "\u{1F4C8}"
            } else {
                "\u{1F4C9}"
            };
            text.push_str(&format!("\n{} Overall: {:+.1}%", arrow, change));
        }
    }

    bot.send_message(msg.chat.id, text).await?;
    Ok(())
}

pub async fn show_top(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let shops = db::get_top_shops(pool, user_id, 5).await?;
    let biggest = db::get_biggest_receipt(pool, user_id).await?;

    if shops.is_empty() {
        bot.send_message(msg.chat.id, "\u{1F3C6} No data yet.")
            .await?;
        return Ok(());
    }

    let mut text = String::from("\u{1F3C6} Spending insights\n\n\u{1F3EA} Top shops:\n");

    for (i, shop) in shops.iter().enumerate() {
        let medal = match i {
            0 => "\u{1F947}",
            1 => "\u{1F948}",
            2 => "\u{1F949}",
            _ => "  ",
        };
        text.push_str(&format!(
            "{} {} \u{2014} {:.2} PLN ({} visits)\n",
            medal, shop.shop_name, shop.total, shop.visit_count
        ));
    }

    if let Some((shop, total, date)) = biggest {
        let date_display = date.get(..10).unwrap_or(&date);
        text.push_str(&format!(
            "\n\u{1F4B8} Biggest receipt:\n  {} \u{2014} {:.2} PLN ({})",
            shop, total, date_display
        ));
    }

    // Daily average and projection for current month
    let now = chrono::Local::now();
    let day_of_month = now.day();
    let cur = db::get_month_summary(pool, user_id, now.year(), now.month()).await?;
    if cur.total_spent > 0.0 && day_of_month > 0 {
        let daily_avg = cur.total_spent / day_of_month as f64;
        text.push_str(&format!(
            "\n\n\u{1F4C5} Daily avg this month: {:.2} PLN",
            daily_avg
        ));

        let days_in_month = match now.month() {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                let y = now.year();
                if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        };
        let projected = daily_avg * days_in_month as f64;
        text.push_str(&format!(
            "\n\u{1F52E} Projected: {:.2} PLN by month end",
            projected
        ));
    }

    bot.send_message(msg.chat.id, text).await?;
    Ok(())
}
