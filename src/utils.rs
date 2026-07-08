pub const VALID_CATEGORIES: &[&str] = &[
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

pub fn normalize_category(input: &str) -> Option<&'static str> {
    let normalized = input.trim().to_uppercase().replace([' ', '-'], "_");
    VALID_CATEGORIES.iter().find(|&&c| c == normalized).copied()
}

pub fn category_emoji(cat: &str) -> &str {
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

pub fn category_label(cat: &str) -> &str {
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

pub fn month_name(month: u32) -> &'static str {
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

pub fn month_name_short(month: i32) -> &'static str {
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
