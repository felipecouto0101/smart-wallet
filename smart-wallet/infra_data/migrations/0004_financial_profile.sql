-- Simulated Open Finance profile generated automatically per user.
-- Values are synthetic/randomised on first access and refreshed monthly.
CREATE TABLE IF NOT EXISTS financial_profiles (
    user_id                        UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    card_bill_cents                BIGINT NOT NULL DEFAULT 0,
    card_bill_days                 SMALLINT NOT NULL DEFAULT 15,
    pending_bills_cents            BIGINT NOT NULL DEFAULT 0,
    estimated_variable_spend_cents BIGINT NOT NULL DEFAULT 0,
    monthly_savings_goal_cents     BIGINT NOT NULL DEFAULT 0,
    days_until_salary              SMALLINT NOT NULL DEFAULT 15,
    monthly_income_cents           BIGINT NOT NULL DEFAULT 0,
    generated_at                   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
