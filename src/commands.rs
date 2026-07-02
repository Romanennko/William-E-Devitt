use base64::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::Datelike;
use crate::db;
use sqlx::SqlitePool;
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

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReceiptCategory {
    RentMortgage,
    Utilities,
    Groceries,
    HouseholdChems,
    Obligations,
    RestaurantsCafes,
    Entertainment,
    ClothingShoes,
    PublicTransport,
    TaxiCarsharing,
    Medical,
    PersonalCare,
    Sport,
    EmergencyFund,
    Investments,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReceiptItem {
    pub name: String,
    pub price: f64,
    pub category: ReceiptCategory,
    pub is_junk_food: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReceiptData {
    pub shop_name: Option<String>,
    pub total: f64,
    pub receipt_date: Option<String>,
    pub items: Vec<ReceiptItem>,
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
            check_expenses(&bot, &msg, &pool).await?;
        }
        Command::History => {
            show_history(&bot, &msg, &pool).await?;
        }
        Command::Stats => {
            show_stats(&bot, &msg, &pool).await?;
        }
        Command::Delete => {
            delete_last(&bot, &msg, &pool).await?;
        }
        Command::Month(arg) => {
            show_month(&bot, &msg, &pool, arg).await?;
        }
        Command::Category(arg) => {
            show_category(&bot, &msg, &pool, arg).await?;
        }
        Command::Trend => {
            show_trend(&bot, &msg, &pool).await?;
        }
        Command::Top => {
            show_top(&bot, &msg, &pool).await?;
        }
    }
    Ok(())
}

pub async fn handle_photo(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    pool: SqlitePool,
) -> HandlerResult {
    // Allow user to cancel photo mode
    if let Some(text) = msg.text() {
        if text.starts_with("/cancel") {
            dialogue.exit().await?;
            bot.send_message(msg.chat.id, "Cancelled. Back to normal mode.")
                .await?;
            return Ok(());
        }
    }

    if let Some(photos) = msg.photo() {
        if let Some(best_photo) = photos.last() {
            let telegram_file = bot.get_file(best_photo.file.id.clone()).await?;

            let mut buffer = Vec::new();
            bot.download_file(&telegram_file.path, &mut buffer).await?;

            let b64_image = BASE64_STANDARD.encode(&buffer);

            match ask_gemini(&b64_image).await {
                Ok(json_str) => match serde_json::from_str::<ReceiptData>(&json_str) {
                    Ok(receipt) => {
                        let mut tx = pool.begin().await?;

                        let tg_user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);

                        let user_id_row: (i64,) = sqlx::query_as(
                                "INSERT INTO users (telegram_id) VALUES (?) ON CONFLICT(telegram_id) DO UPDATE SET telegram_id=telegram_id RETURNING id"
                            )
                            .bind(tg_user_id)
                            .fetch_one(&mut *tx)
                            .await?;
                        let user_id = user_id_row.0;

                        let r_date = receipt.receipt_date.as_deref().unwrap_or("UNKNOWN");
                        let final_date = if r_date == "UNKNOWN" {
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
                        } else {
                            format!("{} 00:00:00", r_date)
                        };

                        let shop_name = receipt.shop_name.as_deref().unwrap_or("UNKNOWN");

                        let receipt_id_row: (i64,) = sqlx::query_as(
                                "INSERT INTO receipts (user_id, shop_name, total_sum, currency, receipt_date) VALUES (?, ?, ?, ?, ?) RETURNING id"
                            )
                            .bind(user_id)
                            .bind(shop_name)
                            .bind(receipt.total)
                            .bind("PLN")
                            .bind(&final_date)
                            .fetch_one(&mut *tx)
                            .await?;
                        let receipt_id = receipt_id_row.0;

                        let mut junk_total = 0.0;
                        for item in &receipt.items {
                            if item.is_junk_food {
                                junk_total += item.price;
                            }

                            let cat_str = serde_json::to_string(&item.category)
                                .unwrap_or_else(|_| "\"UNKNOWN\"".to_string())
                                .replace('\"', "");

                            sqlx::query(
                                    "INSERT INTO items (receipt_id, name, price, category, is_junk_food) VALUES (?, ?, ?, ?, ?)"
                                )
                                .bind(receipt_id)
                                .bind(&item.name)
                                .bind(item.price)
                                .bind(cat_str)
                                .bind(item.is_junk_food)
                                .execute(&mut *tx)
                                .await?;
                        }

                        tx.commit().await?;

                        let msg_text = format!(
                            "Shop: {}\nTotal: {:.2} PLN\n Date: {}\nJunk food: {:.2} PLN",
                            shop_name, receipt.total, final_date, junk_total
                        );

                        bot.send_message(msg.chat.id, msg_text).await?;
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Error parsing JSON: {}", e))
                            .await?;
                    }
                },
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("API Error: {}", e))
                        .await?;
                }
            }

            // Successfully handled the photo, reset state to Start
            dialogue.exit().await?;
        }
    } else {
        bot.send_message(
            msg.chat.id,
            "That's not a photo, dipshit. Send an actual photo.",
        )
        .await?;
    }

    Ok(())
}

async fn ask_gemini(b64_image: &str) -> Result<String, AppError> {
    let api_key = std::env::var("GEMINI_API_KEY").expect("Where's the API key, Lebowski?");
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}",
        api_key
    );

    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "contents": [{
            "parts": [
                { "text": "Extract items, total sum, and the date from the receipt. Assign the correct category based on the provided schema definitions." },
                { "inline_data": { "mime_type": "image/jpeg", "data": b64_image } }
            ]
        }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "OBJECT",
                "properties": {
                    "shop_name": { "type": "STRING", "description": "Name of the shop, restaurant, or service (e.g., Biedronka, Żabka, Uber). Return 'UNKNOWN' if missing." },
                    "total": { "type": "NUMBER", "description": "Total sum of the receipt" },
                    "receipt_date": {
                        "type": "STRING",
                        "description": "Date of the receipt in YYYY-MM-DD format. If the date is completely missing, unreadable, or not present on the image, ALWAYS return 'UNKNOWN'."
                    },
                    "items": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "name": { "type": "STRING", "description": "Original item name from the receipt" },
                                "price": { "type": "NUMBER", "description": "Price of the item" },
                                "category": {
                                    "type": "STRING",
                                    "enum": [
                                        "RENT_MORTGAGE",
                                        "UTILITIES",
                                        "GROCERIES",
                                        "HOUSEHOLD_CHEMS",
                                        "OBLIGATIONS",
                                        "RESTAURANTS_CAFES",
                                        "ENTERTAINMENT",
                                        "CLOTHING_SHOES",
                                        "PUBLIC_TRANSPORT",
                                        "TAXI_CARSHARING",
                                        "MEDICAL",
                                        "PERSONAL_CARE",
                                        "SPORT",
                                        "EMERGENCY_FUND",
                                        "INVESTMENTS"
                                    ],
                                    "description": "Strict category mapping: \
                                                    RENT_MORTGAGE: Rent or mortgage. \
                                                    UTILITIES: Electricity, water, heating, internet, mobile. \
                                                    GROCERIES: Food bought at a supermarket/grocery store for home (CRITICAL: separate from restaurants!). \
                                                    HOUSEHOLD_CHEMS: Detergents, toilet paper, sponges, household goods. \
                                                    OBLIGATIONS: Taxes, insurance, alimony. \
                                                    RESTAURANTS_CAFES: Food delivery, fast food, coffee to go, bars, cafes. \
                                                    ENTERTAINMENT: Movies, concerts, parties, video games, paid subscriptions (Netflix, Spotify, etc.). \
                                                    CLOTHING_SHOES: Shopping for clothes and footwear. \
                                                    PUBLIC_TRANSPORT: Subways, buses, trains, travel passes. \
                                                    TAXI_CARSHARING: Taxi rides and carsharing. \
                                                    MEDICAL: Doctors, pharmacies, medicine, medical tests, dentists. \
                                                    PERSONAL_CARE: Haircuts, cosmetics, barbershop, manicure. \
                                                    SPORT: Gym memberships, sports equipment, swimming pool. \
                                                    EMERGENCY_FUND: Savings, emergency fund transfers. \
                                                    INVESTMENTS: Stocks, bonds, crypto purchases."
                                },
                                "is_junk_food": {
                                    "type": "BOOLEAN",
                                    "description": "True if the item is junk food, fast food or snacks (chips, crackers, candy, soda, sweets, pizza, burgers, kebab), regardless of whether it's from GROCERIES or RESTAURANTS_CAFES."
                                }
                            },
                            "required": ["name", "price", "category", "is_junk_food"]
                        }
                    }
                },
                "required": ["shop_name", "total", "receipt_date", "items"]
            }
        }
    });

    let res = client.post(&url).json(&payload).send().await?;

    let res_json: serde_json::Value = res.json().await?;

    let extracted_text = res_json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("{}")
        .to_string();

    Ok(extracted_text)
}

fn category_emoji(cat: &str) -> &str {
    match cat {
        "RENT_MORTGAGE" => "\u{1F3E0}",
        "UTILITIES" => "\u{1F4A1}",
        "GROCERIES" => "\u{1F6D2}",
        "HOUSEHOLD_CHEMS" => "\u{1F9F9}",
        "OBLIGATIONS" => "\u{1F4CB}",
        "RESTAURANTS_CAFES" => "\u{1F37D}",
        "ENTERTAINMENT" => "\u{1F3AE}",
        "CLOTHING_SHOES" => "\u{1F455}",
        "PUBLIC_TRANSPORT" => "\u{1F68C}",
        "TAXI_CARSHARING" => "\u{1F695}",
        "MEDICAL" => "\u{1F3E5}",
        "PERSONAL_CARE" => "\u{1F487}",
        "SPORT" => "\u{1F3CB}",
        "EMERGENCY_FUND" => "\u{1F3E6}",
        "INVESTMENTS" => "\u{1F4C8}",
        _ => "\u{2753}",
    }
}

fn category_label(cat: &str) -> &str {
    match cat {
        "RENT_MORTGAGE" => "Rent/Mortgage",
        "UTILITIES" => "Utilities",
        "GROCERIES" => "Groceries",
        "HOUSEHOLD_CHEMS" => "Household",
        "OBLIGATIONS" => "Obligations",
        "RESTAURANTS_CAFES" => "Restaurants",
        "ENTERTAINMENT" => "Entertainment",
        "CLOTHING_SHOES" => "Clothing",
        "PUBLIC_TRANSPORT" => "Transport",
        "TAXI_CARSHARING" => "Taxi/Carsharing",
        "MEDICAL" => "Medical",
        "PERSONAL_CARE" => "Personal Care",
        "SPORT" => "Sport",
        "EMERGENCY_FUND" => "Emergency Fund",
        "INVESTMENTS" => "Investments",
        _ => "Other",
    }
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "???",
    }
}

async fn resolve_user_id(
    bot: &Bot,
    msg: &Message,
    pool: &SqlitePool,
) -> Result<Option<i64>, AppError> {
    let tg_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    match db::find_user_id(pool, tg_id).await? {
        Some(id) => Ok(Some(id)),
        None => {
            bot.send_message(msg.chat.id, "No data yet. Send a receipt with /photo first!")
                .await?;
            Ok(None)
        }
    }
}

async fn check_expenses(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
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
            format!("\u{1F4CA} No expenses for {} {} yet.", month_name(month), year),
        )
        .await?;
        return Ok(());
    }

    let mut text = format!("\u{1F4CA} Expenses \u{2014} {} {}\n\n", month_name(month), year);

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

async fn show_history(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
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

async fn show_stats(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
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
        let arrow = if change >= 0.0 { "\u{1F4C8}" } else { "\u{1F4C9}" };
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

    let categories =
        db::get_expenses_by_category(pool, user_id, cur_year, cur_month).await?;
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

async fn delete_last(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
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

fn month_name_short(month: i32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

const VALID_CATEGORIES: &[&str] = &[
    "RENT_MORTGAGE",
    "UTILITIES",
    "GROCERIES",
    "HOUSEHOLD_CHEMS",
    "OBLIGATIONS",
    "RESTAURANTS_CAFES",
    "ENTERTAINMENT",
    "CLOTHING_SHOES",
    "PUBLIC_TRANSPORT",
    "TAXI_CARSHARING",
    "MEDICAL",
    "PERSONAL_CARE",
    "SPORT",
    "EMERGENCY_FUND",
    "INVESTMENTS",
];

fn normalize_category(input: &str) -> Option<&'static str> {
    let normalized = input.trim().to_uppercase().replace(' ', "_").replace('-', "_");
    VALID_CATEGORIES.iter().find(|&&c| c == normalized).copied()
}

async fn show_month(bot: &Bot, msg: &Message, pool: &SqlitePool, arg: String) -> HandlerResult {
    let user_id = match resolve_user_id(bot, msg, pool).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let arg = arg.trim().to_string();
    if arg.is_empty() {
        bot.send_message(msg.chat.id, "Usage: /month YYYY-MM\nExample: /month 2026-07")
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
    if year < 2020 || year > 2100 || month < 1 || month > 12 {
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

async fn show_category(
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
            let mut text = format!(
                "Unknown category: \"{}\"\n\nAvailable:\n",
                arg
            );
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

async fn show_trend(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
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

async fn show_top(bot: &Bot, msg: &Message, pool: &SqlitePool) -> HandlerResult {
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
