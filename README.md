# Smart Wallet

A production-grade investment wallet built in Rust, following Hexagonal Architecture and a full DevSecOps pipeline.

Users simulate an Open Finance financial profile, track crypto and dollar positions, execute buy/sell orders and analyze purchases against their real free balance before spending.

---

## Architecture

The project is structured as a **Cargo Workspace** with three crates:

```
.
├── core_business/      # Pure domain logic — no framework dependencies
├── infra_data/         # PostgreSQL persistence via SQLx
└── api_server_new/     # HTTP server via Axum + Askama HTML templates
```

### Hexagonal Architecture

```
┌─────────────────────────────────────────┐
│              api_server                 │  ← Axum routes, JWT auth, HTML templates
│         (adapters / driving)            │
└────────────────────┬────────────────────┘
                     │ uses traits from
┌────────────────────▼────────────────────┐
│             core_business               │  ← Domain entities, business rules, port traits
│           (domain / ports)              │
└────────────────────┬────────────────────┘
                     │ implemented by
┌────────────────────▼────────────────────┐
│              infra_data                 │  ← SQLx repositories, live price provider
│         (adapters / driven)             │
└─────────────────────────────────────────┘
```

`core_business` has **zero dependencies** on web frameworks or database drivers. All infrastructure contracts are defined as Rust traits (`UserRepository`, `WalletRepository`, `AssetRepository`, `FinancialProfileRepository`).

---

## Features

- **User registration and login** with bcrypt password hashing and JWT authentication
- **Investment wallet** — buy and sell BTC, ETH, SOL and USD with fractional quantities
- **Live prices** — fetched from a public API every 2 seconds in the background
- **P&L tracking** — average cost and profit/loss per position updated in real time
- **Order history** — full record of every executed trade
- **Open Finance simulation** — see the dedicated section below
- **Free Balance analysis** — calculates real disposable income and warns before overspending
- **Proactive wallet alerts** — shown on the dashboard without requiring a purchase query

---

## Open Finance Simulator

Real Open Finance integrations require consent flows, bank partnerships and regulatory approval. This project simulates the same concept synthetically — giving users the full experience of seeing their financial life reflected in the wallet without connecting to any real bank.

### How it works

When the user clicks **Start Simulation**, they choose one of three financial archetypes:

| Archetype | Monthly Income | Card bill | Savings goal |
|---|---|---|---|
| Student | R$ 1,200 – R$ 1,800 | 15–35% of income | 5–10% of income |
| Young Professional | R$ 3,500 – R$ 5,000 | 25–55% of income | 10–20% of income |
| Family | R$ 7,000 – R$ 9,500 | 30–60% of income | 15–25% of income |

The system then generates a `FinancialProfile` using a seeded LCG (linear congruential generator) — deterministic, no external dependencies, zero I/O:

```
FinancialProfile {
    monthly_income_cents,           // salary
    card_bill_cents,                // credit card bill closing soon
    card_bill_days,                 // days until the card closes
    pending_bills_cents,            // boletos due within 15 days
    estimated_variable_spend_cents, // transport, groceries, subscriptions
    monthly_savings_goal_cents,     // investment target for the month
    days_until_salary,              // days until next pay deposit
}
```

A safety constraint is enforced: total expenses are always capped at **90% of income**, guaranteeing the profile is financially plausible regardless of the random seed.

The profile is persisted in PostgreSQL and reused for up to 30 days. A new simulation resets it.

### Free Balance — the core insight

The central concept is the **True Free Balance** — what the user actually has available after all obligations are deducted:

```
True Free Balance = Income − Card Bill − Pending Bills − Variable Spend − Savings Goal
```

This number is what the **Free Balance** page exposes, together with a full breakdown of every deduction, so the user understands exactly where their money is going.

### Purchase Analysis

When the user types a purchase amount on the Free Balance page, the engine evaluates it and returns one of three verdicts:

| Verdict | Condition | Action |
|---|---|---|
| **Green** | Free balance after purchase ≥ 0 | Safe to proceed |
| **Yellow** | Free balance was positive but purchase exceeds it | Suggests postponing or splitting |
| **Red** | Free balance was already negative before the purchase | Strongly warns against buying |

For Yellow and Red verdicts the engine also calculates **installment suggestions** — it finds the smallest number of monthly installments (2 to 6) where each installment fits within the free balance, preventing the purchase from compromising investments.

### Proactive alerts

The wallet dashboard displays alerts automatically, without the user needing to query a purchase amount:

- **Red** — obligations exceed income (negative free balance)
- **Yellow** — savings goal not yet reached for the month
- **Green** — savings goal reached, card bill closing soon (informational)

---

## Tech Stack

| Layer | Technology |
|---|---|
| HTTP server | Axum 0.7 |
| HTML templates | Askama 0.12 |
| Database | PostgreSQL via SQLx 0.7 |
| Authentication | JWT (jsonwebtoken 9) + bcrypt 0.15 |
| Async runtime | Tokio 1 |
| Serialization | Serde + serde_json |
| Validation | validator 0.18 |
| Secret protection | secrecy 0.10 |
| Error handling | thiserror + color-eyre |
| HTTP client | reqwest 0.12 |

---

## Security

- `#![forbid(unsafe_code)]` in every crate
- Passwords never stored as plain `String` — wrapped with `secrecy::SecretString`
- Newtype pattern for all IDs (`UserId`, `OrderId`) to prevent parameter confusion
- JWT stored in `HttpOnly; SameSite=Strict` cookies — not accessible from JavaScript
- All input DTOs validated with `validator` before processing
- Centralized auth middleware — unauthenticated requests are blocked at the router level before reaching any handler

---

## Getting Started

### Prerequisites

- Rust stable (`rustup update stable`)
- PostgreSQL running locally
- A `.env` file at the project root

### Environment

```bash
cp .env.example .env
```

Edit `.env`:

```env
DATABASE_URL=postgres://user:password@localhost:5432/smart_wallet
JWT_SECRET=replace_with_a_secret_of_at_least_32_characters
RUST_LOG=info
```

### Run

```bash
cargo run
```

The server starts at `http://localhost:8090`. SQLx migrations run automatically on startup.

---

## Project Structure

```
core_business/
└── src/
    ├── domain/
    │   ├── asset.rs             # AssetType, Asset
    │   ├── financial_profile.rs # FinancialProfile, analyze_purchase, proactive_alerts
    │   ├── order.rs             # Order, OrderType
    │   ├── user.rs              # User, UserId, CreateUserDto
    │   └── wallet.rs            # Wallet, Position — buy/sell logic
    ├── errors.rs                # BusinessError enum
    └── ports.rs                 # Repository and provider traits

infra_data/
└── src/
    ├── repositories/
    │   ├── asset.rs             # AssetRepositoryPg + in-memory price cache
    │   ├── financial_profile.rs # FinancialProfileRepositoryPg + synthetic generation
    │   ├── user.rs              # UserRepositoryPg
    │   └── wallet.rs            # WalletRepositoryPg
    ├── db.rs                    # SQLx connection pool
    └── price_provider.rs        # LivePriceProvider — fetches BTC/ETH/SOL/USD prices

api_server_new/
└── src/
    ├── auth/
    │   ├── mod.rs               # generate_token, validate_token
    │   └── middleware.rs        # require_auth Axum middleware
    ├── routes/
    │   ├── auth.rs              # POST /api/register, /api/login, /api/logout
    │   ├── wallet.rs            # GET /api/wallet, POST /api/wallet/buy|sell|history
    │   ├── assets.rs            # GET /api/assets
    │   ├── saldo_real.rs        # GET /saldo-real, GET /api/saldo-real
    │   └── pages.rs             # HTML page handlers
    └── state/
        └── mod.rs               # AppState — shared repositories + jwt_secret
```

---

## API Routes

### Public (no authentication required)

| Method | Path | Description |
|---|---|---|
| `GET` | `/` | Login page |
| `GET` | `/login` | Login page |
| `POST` | `/login` | Submit login form |
| `GET` | `/register` | Register page |
| `POST` | `/register` | Submit register form |
| `POST` | `/api/login` | JSON login |
| `POST` | `/api/register` | JSON register |

### Protected (JWT cookie required)

| Method | Path | Description |
|---|---|---|
| `GET` | `/wallet` | Wallet dashboard |
| `GET` | `/assets` | Assets and positions |
| `GET` | `/saldo-real` | Free Balance page |
| `POST` | `/wallet/buy` | Buy asset (form) |
| `POST` | `/wallet/sell` | Sell asset (form) |
| `POST` | `/wallet/simulate` | Start financial simulation |
| `GET` | `/logout` | Clear session and redirect |
| `GET` | `/api/wallet` | JSON wallet state |
| `POST` | `/api/wallet/buy` | JSON buy order |
| `POST` | `/api/wallet/sell` | JSON sell order |
| `GET` | `/api/wallet/history` | JSON order history |
| `GET` | `/api/assets` | JSON asset prices |
| `GET` | `/api/saldo-real` | JSON free balance analysis |
| `POST` | `/api/logout` | JSON logout |

---

## Tests

**76 tests — 0 failures**

```bash
# All tests
cargo test --workspace --features api_server/test-helpers

# List only
cargo test --workspace --features api_server/test-helpers -- --list
```

### Test pyramid

| Type | Count | Location |
|---|---|---|
| Unit | 47 | `#[cfg(test)]` inside each module |
| Property-based (proptest) | 7 | `financial_profile.rs`, `wallet.rs` |
| E2E integration with mocks | 15 | `api_server_new/tests/integration.rs` |
| Doc-tests | 3 | `asset.rs`, `financial_profile.rs`, `wallet.rs` |

Integration tests spin up the full Axum router in memory using `mockall` mocks for all repositories — no real database or network required.

---

## DevSecOps Pipeline

Three-level pipeline runs on every push and pull request to `main`:

```
push / PR
    │
    ├── Level 1: cargo fmt --check
    │           cargo clippy --workspace -D warnings
    │
    ├── Level 2: cargo audit (RustSec advisory database)
    │
    └── Level 3: cargo test --workspace (blocked until levels 1+2 pass)
```

---

## Database Migrations

Migrations run automatically via SQLx on server startup. Manual execution:

```bash
sqlx migrate run --database-url $DATABASE_URL
```

| Migration | Description |
|---|---|
| `0001_initial.sql` | users, wallets, positions, orders |
| `0002_fractional_quantity.sql` | fractional quantity support |
| `0003_avg_cost.sql` | average cost per position |
| `0004_financial_profile.sql` | Open Finance profile storage |
