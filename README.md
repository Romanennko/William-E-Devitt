# William - Personal Finance Telegram Bot

A Telegram bot written in Rust that tracks personal expenses by analyzing photos of receipts using the Gemini API.

## Features

- **Receipt Processing**: Upload receipt photos to automatically extract items, prices, total sum, and date.
- **Categorization**: Automatically categorizes items (Groceries, Utilities, Entertainment, etc.) and flags junk food.
- **Expense Tracking**: View expenses for the current month, specific months, or by category.
- **Statistics & Trends**: Access monthly summaries, spending trends, and top categories.
- **Private Access**: Restricted to a single, pre-configured user.

## Tech Stack

- **Rust**
- **teloxide**: Telegram bot framework
- **sqlx & SQLite**: Database operations
- **reqwest**: HTTP client for API requests
- **tokio**: Asynchronous runtime
- **Gemini API**: Receipt data extraction and categorization

## Requirements

- Rust (edition 2024)
- Telegram Bot Token
- Gemini API Key
- SQLite

## Environment Variables

Create a `.env` file in the project root with the following variables:

```env
TELOXIDE_TOKEN=your_telegram_bot_token
ALLOWED_USER_ID=your_telegram_user_id
DATABASE_URL=sqlite:db.sqlite
GEMINI_API_KEY=your_gemini_api_key
```

## Setup and Running

1. Initialize the SQLite database:
   ```sh
   sqlx database create
   sqlx migrate run
   ```

2. Run the bot:
   ```sh
   cargo run --release
   ```

## Development and Architecture

The codebase is organized into logical modules for maintainability:

- `src/models.rs`: Data structures and enums (e.g., `ReceiptData`, `ReceiptCategory`).
- `src/utils.rs`: Helper functions for formatting and category handling.
- `src/commands/`: Modular command handlers for statistics, history, and photo processing.
- `src/db.rs`: Database queries and transactions.

**Linting and Formatting**
The project enforces strict linting and formatting standards. Before committing code, ensure you run:
```sh
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
```

## Available Commands

- `/help` - Display all available commands
- `/photo` - Upload a photo of a receipt
- `/check` - Check expenses for the current month
- `/history` - Show recent receipts
- `/stats` - Show monthly statistics
- `/delete` - Delete the last added receipt
- `/month YYYY-MM` - Show expenses for a specific month
- `/category <name>` - Show items in a specific category
- `/trend` - Show spending trend over the last 6 months
- `/top` - Show top spending insights
