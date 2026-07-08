use base64::prelude::*;
use sqlx::SqlitePool;
use teloxide::{net::Download, prelude::*};

use super::{AppError, HandlerResult, MyDialogue};
use crate::models::ReceiptData;

pub async fn handle_photo(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    pool: SqlitePool,
) -> HandlerResult {
    // Allow user to cancel photo mode
    if let Some(text) = msg.text()
        && text.starts_with("/cancel")
    {
        dialogue.exit().await?;
        bot.send_message(msg.chat.id, "Cancelled. Back to normal mode.")
            .await?;
        return Ok(());
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
