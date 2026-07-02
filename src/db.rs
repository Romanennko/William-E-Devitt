use sqlx::SqlitePool;

pub struct CategoryExpense {
    pub category: String,
    pub total: f64,
    pub item_count: i64,
}

pub struct MonthSummary {
    pub total_spent: f64,
    pub receipt_count: i64,
    pub items_total: f64,
    pub junk_total: f64,
}

#[allow(dead_code)]
pub struct ReceiptInfo {
    pub id: i64,
    pub shop_name: String,
    pub total_sum: f64,
    pub receipt_date: String,
}

pub async fn find_user_id(pool: &SqlitePool, telegram_id: i64) -> Result<Option<i64>, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64,)>("SELECT id FROM users WHERE telegram_id = ?")
        .bind(telegram_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

pub async fn get_expenses_by_category(
    pool: &SqlitePool,
    user_id: i64,
    year: i32,
    month: u32,
) -> Result<Vec<CategoryExpense>, sqlx::Error> {
    let start = format!("{:04}-{:02}-01 00:00:00", year, month);
    let (ny, nm) = if month == 12 {
        (year + 1, 1u32)
    } else {
        (year, month + 1)
    };
    let end = format!("{:04}-{:02}-01 00:00:00", ny, nm);

    let rows = sqlx::query_as::<_, (String, f64, i64)>(
        "SELECT i.category, ROUND(SUM(i.price), 2) as total, COUNT(*) as cnt \
         FROM items i \
         JOIN receipts r ON i.receipt_id = r.id \
         WHERE r.user_id = ? AND r.receipt_date >= ? AND r.receipt_date < ? \
         GROUP BY i.category \
         ORDER BY total DESC",
    )
    .bind(user_id)
    .bind(&start)
    .bind(&end)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(category, total, item_count)| CategoryExpense {
            category,
            total,
            item_count,
        })
        .collect())
}

pub async fn get_month_summary(
    pool: &SqlitePool,
    user_id: i64,
    year: i32,
    month: u32,
) -> Result<MonthSummary, sqlx::Error> {
    let start = format!("{:04}-{:02}-01 00:00:00", year, month);
    let (ny, nm) = if month == 12 {
        (year + 1, 1u32)
    } else {
        (year, month + 1)
    };
    let end = format!("{:04}-{:02}-01 00:00:00", ny, nm);

    let (total_spent, receipt_count): (f64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(total_sum), 0.0), COUNT(*) \
         FROM receipts \
         WHERE user_id = ? AND receipt_date >= ? AND receipt_date < ?",
    )
    .bind(user_id)
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await?;

    let (items_total, junk_total): (f64, f64) = sqlx::query_as(
        "SELECT COALESCE(SUM(i.price), 0.0), \
                COALESCE(SUM(CASE WHEN i.is_junk_food = 1 THEN i.price ELSE 0.0 END), 0.0) \
         FROM items i \
         JOIN receipts r ON i.receipt_id = r.id \
         WHERE r.user_id = ? AND r.receipt_date >= ? AND r.receipt_date < ?",
    )
    .bind(user_id)
    .bind(&start)
    .bind(&end)
    .fetch_one(pool)
    .await?;

    Ok(MonthSummary {
        total_spent,
        receipt_count,
        items_total,
        junk_total,
    })
}

pub async fn get_recent_receipts(
    pool: &SqlitePool,
    user_id: i64,
    limit: i64,
) -> Result<Vec<ReceiptInfo>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (i64, String, f64, String)>(
        "SELECT id, COALESCE(shop_name, 'Unknown'), total_sum, receipt_date \
         FROM receipts \
         WHERE user_id = ? \
         ORDER BY receipt_date DESC, created_at DESC \
         LIMIT ?",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, shop_name, total_sum, receipt_date)| ReceiptInfo {
            id,
            shop_name,
            total_sum,
            receipt_date,
        })
        .collect())
}

pub async fn delete_last_receipt(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Option<ReceiptInfo>, sqlx::Error> {
    let receipt = sqlx::query_as::<_, (i64, String, f64, String)>(
        "SELECT id, COALESCE(shop_name, 'Unknown'), total_sum, receipt_date \
         FROM receipts \
         WHERE user_id = ? \
         ORDER BY created_at DESC \
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if let Some((id, shop_name, total_sum, receipt_date)) = receipt {
        let mut tx = pool.begin().await?;

        sqlx::query("DELETE FROM items WHERE receipt_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM receipts WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(Some(ReceiptInfo {
            id,
            shop_name,
            total_sum,
            receipt_date,
        }))
    } else {
        Ok(None)
    }
}

pub struct CategoryDetail {
    pub item_name: String,
    pub price: f64,
    pub shop_name: String,
    pub receipt_date: String,
    pub is_junk_food: bool,
}

pub struct MonthlyTrend {
    pub year: i32,
    pub month: i32,
    pub total: f64,
    pub receipt_count: i64,
}

pub struct ShopTotal {
    pub shop_name: String,
    pub total: f64,
    pub visit_count: i64,
}

pub async fn get_category_items(
    pool: &SqlitePool,
    user_id: i64,
    year: i32,
    month: u32,
    category: &str,
) -> Result<Vec<CategoryDetail>, sqlx::Error> {
    let start = format!("{:04}-{:02}-01 00:00:00", year, month);
    let (ny, nm) = if month == 12 {
        (year + 1, 1u32)
    } else {
        (year, month + 1)
    };
    let end = format!("{:04}-{:02}-01 00:00:00", ny, nm);

    let rows = sqlx::query_as::<_, (String, f64, String, String, bool)>(
        "SELECT i.name, i.price, COALESCE(r.shop_name, 'Unknown'), r.receipt_date, i.is_junk_food \
         FROM items i \
         JOIN receipts r ON i.receipt_id = r.id \
         WHERE r.user_id = ? AND r.receipt_date >= ? AND r.receipt_date < ? \
         AND i.category = ? \
         ORDER BY r.receipt_date DESC, i.price DESC",
    )
    .bind(user_id)
    .bind(&start)
    .bind(&end)
    .bind(category)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(item_name, price, shop_name, receipt_date, is_junk_food)| CategoryDetail {
                item_name,
                price,
                shop_name,
                receipt_date,
                is_junk_food,
            },
        )
        .collect())
}

pub async fn get_monthly_trend(
    pool: &SqlitePool,
    user_id: i64,
    start_date: &str,
) -> Result<Vec<MonthlyTrend>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (i32, i32, f64, i64)>(
        "SELECT CAST(strftime('%Y', receipt_date) AS INTEGER), \
                CAST(strftime('%m', receipt_date) AS INTEGER), \
                SUM(total_sum + 0.0), \
                COUNT(*) \
         FROM receipts \
         WHERE user_id = ? AND receipt_date >= ? \
         GROUP BY 1, 2 \
         ORDER BY 1 ASC, 2 ASC",
    )
    .bind(user_id)
    .bind(start_date)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(year, month, total, receipt_count)| MonthlyTrend {
            year,
            month,
            total,
            receipt_count,
        })
        .collect())
}

pub async fn get_top_shops(
    pool: &SqlitePool,
    user_id: i64,
    limit: i64,
) -> Result<Vec<ShopTotal>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, f64, i64)>(
        "SELECT COALESCE(shop_name, 'Unknown'), SUM(total_sum + 0.0), COUNT(*) \
         FROM receipts \
         WHERE user_id = ? \
         GROUP BY shop_name \
         ORDER BY 2 DESC \
         LIMIT ?",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(shop_name, total, visit_count)| ShopTotal {
            shop_name,
            total,
            visit_count,
        })
        .collect())
}

pub async fn get_biggest_receipt(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Option<(String, f64, String)>, sqlx::Error> {
    sqlx::query_as::<_, (String, f64, String)>(
        "SELECT COALESCE(shop_name, 'Unknown'), total_sum, receipt_date \
         FROM receipts \
         WHERE user_id = ? \
         ORDER BY total_sum DESC \
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

