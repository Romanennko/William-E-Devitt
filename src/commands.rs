use base64::prelude::*;
use serde::Deserialize;
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
    #[command(description = "check expenses")]
    Check,
}

#[derive(Debug, Deserialize, PartialEq)]
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

#[derive(Debug, Deserialize)]
pub struct ReceiptItem {
    pub name: String,
    pub price: f64,
    pub category: ReceiptCategory,
    pub is_junk_food: bool,
}

#[derive(Debug, Deserialize)]
pub struct ReceiptData {
    pub total: f64,
    pub receipt_date: Option<String>,
    pub items: Vec<ReceiptItem>,
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
        Command::Photo => {
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

            match ask_gemini(&b64_image).await {
                Ok(json_str) => {
                    match serde_json::from_str::<ReceiptData>(&json_str) {
                        Ok(receipt) => {
                            let junk_total: f64 = receipt
                                .items
                                .iter()
                                .filter(|item| item.is_junk_food)
                                .map(|item| item.price)
                                .sum();

                            let msg_text = format!(
                                "Total amount: {:.2}\n📅 Date: {}\n🍔 Junk food spent: {:.2}",
                                receipt.total,
                                receipt.receipt_date.as_deref().unwrap_or("Unknown"),
                                junk_total
                            );

                            bot.send_message(msg.chat.id, msg_text).await?;

                            // TODO LOGIC OF STORING DATA IN THE DATABASE
                        }
                        Err(e) => {
                            bot.send_message(msg.chat.id, format!("Error parsing JSON: {}", e))
                                .await?;
                        }
                    }
                }
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
                "required": ["total", "receipt_date", "items"]
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
