use serde::{Deserialize, Serialize};

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceivePhoto,
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
