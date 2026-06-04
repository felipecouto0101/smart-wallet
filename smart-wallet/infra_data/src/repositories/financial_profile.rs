#![forbid(unsafe_code)]

use sqlx::PgPool;
use chrono::Utc;
use uuid::Uuid;

use core_business::{BusinessError, UserId, FinancialProfile};

pub struct FinancialProfileRepositoryPg {
    pool: PgPool,
}

impl FinancialProfileRepositoryPg {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns the stored profile or generates a fresh synthetic one.
    /// Regenerates if the profile is older than 30 days.
    pub async fn get_or_generate(&self, user_id: &UserId) -> Result<FinancialProfile, BusinessError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            card_bill_cents:                i64,
            card_bill_days:                 i16,
            pending_bills_cents:            i64,
            estimated_variable_spend_cents: i64,
            monthly_savings_goal_cents:     i64,
            days_until_salary:              i16,
            monthly_income_cents:           i64,
            generated_at:                   chrono::DateTime<Utc>,
        }

        let existing: Option<Row> = sqlx::query_as(
            r#"SELECT card_bill_cents, card_bill_days, pending_bills_cents,
                      estimated_variable_spend_cents, monthly_savings_goal_cents,
                      days_until_salary, monthly_income_cents, generated_at
               FROM financial_profiles WHERE user_id = $1"#,
        )
        .bind(user_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        if let Some(row) = existing {
            let age_days = (Utc::now() - row.generated_at).num_days();
            if age_days < 30 {
                return Ok(FinancialProfile {
                    card_bill_cents:                row.card_bill_cents as u64,
                    card_bill_days:                 row.card_bill_days as u8,
                    pending_bills_cents:            row.pending_bills_cents as u64,
                    estimated_variable_spend_cents: row.estimated_variable_spend_cents as u64,
                    monthly_savings_goal_cents:     row.monthly_savings_goal_cents as u64,
                    days_until_salary:              row.days_until_salary as u8,
                    monthly_income_cents:           row.monthly_income_cents as u64,
                });
            }
        }

        // Generate a new synthetic profile seeded from the user UUID bytes
        // so different users get different — but deterministic — profiles.
        let seed = seed_from_uuid(user_id.0);
        let profile = generate_synthetic(seed);
        self.upsert(user_id, &profile).await?;
        Ok(profile)
    }

    async fn upsert(&self, user_id: &UserId, p: &FinancialProfile) -> Result<(), BusinessError> {
        sqlx::query(
            r#"INSERT INTO financial_profiles
               (user_id, card_bill_cents, card_bill_days, pending_bills_cents,
                estimated_variable_spend_cents, monthly_savings_goal_cents,
                days_until_salary, monthly_income_cents, generated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW())
               ON CONFLICT (user_id) DO UPDATE SET
                 card_bill_cents                = EXCLUDED.card_bill_cents,
                 card_bill_days                 = EXCLUDED.card_bill_days,
                 pending_bills_cents            = EXCLUDED.pending_bills_cents,
                 estimated_variable_spend_cents = EXCLUDED.estimated_variable_spend_cents,
                 monthly_savings_goal_cents     = EXCLUDED.monthly_savings_goal_cents,
                 days_until_salary              = EXCLUDED.days_until_salary,
                 monthly_income_cents           = EXCLUDED.monthly_income_cents,
                 generated_at                   = NOW()"#,
        )
        .bind(user_id.0)
        .bind(p.card_bill_cents as i64)
        .bind(p.card_bill_days as i16)
        .bind(p.pending_bills_cents as i64)
        .bind(p.estimated_variable_spend_cents as i64)
        .bind(p.monthly_savings_goal_cents as i64)
        .bind(p.days_until_salary as i16)
        .bind(p.monthly_income_cents as i64)
        .execute(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;
        Ok(())
    }
}

// ── Synthetic generation ─────────────────────────────────────────────────────

/// Profile kind selected by the user on the simulator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfileKind {
    Student,         // R$ 1.500 income, low costs
    YoungProfessional, // R$ 4.000 income, medium costs
    Family,          // R$ 8.000 income, high costs
}

impl ProfileKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "student" => Self::Student,
            "family"  => Self::Family,
            _         => Self::YoungProfessional,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Student           => "Estudante",
            Self::YoungProfessional => "Jovem Profissional",
            Self::Family            => "Família",
        }
    }

    pub fn income_range(&self) -> (u64, u64) {
        match self {
            Self::Student           => (120_000, 180_000),  // R$ 1.200 – 1.800
            Self::YoungProfessional => (350_000, 500_000),  // R$ 3.500 – 5.000
            Self::Family            => (700_000, 950_000),  // R$ 7.000 – 9.500
        }
    }

    /// Card bill as % range of income.
    fn card_pct_range(&self) -> (u64, u64) {
        match self {
            Self::Student           => (15, 35),
            Self::YoungProfessional => (25, 55),
            Self::Family            => (30, 60),
        }
    }

    /// Pending boletos range (cents).
    fn boleto_range(&self) -> (u64, u64) {
        match self {
            Self::Student           => (10_000,  40_000),  // R$ 100 – 400
            Self::YoungProfessional => (30_000, 120_000),  // R$ 300 – 1.200
            Self::Family            => (80_000, 250_000),  // R$ 800 – 2.500
        }
    }

    /// Variable spend range (cents).
    fn variable_range(&self) -> (u64, u64) {
        match self {
            Self::Student           => (8_000,  25_000),
            Self::YoungProfessional => (20_000,  60_000),
            Self::Family            => (50_000, 130_000),
        }
    }

    /// Savings goal as % range of income.
    fn savings_pct_range(&self) -> (u64, u64) {
        match self {
            Self::Student           => (5, 10),
            Self::YoungProfessional => (10, 20),
            Self::Family            => (15, 25),
        }
    }
}

/// Extracts a u64 seed from the first 8 bytes of a UUID.
fn seed_from_uuid(id: Uuid) -> u64 {
    let b = id.as_bytes();
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// Minimal LCG pseudo-random generator (no external crate needed).
fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state >> 33
}

/// Returns a value in [lo, hi] using the LCG state.
fn rand_range(state: &mut u64, lo: u64, hi: u64) -> u64 {
    lo + lcg_next(state) % (hi - lo + 1)
}

/// Generates a synthetic profile constrained to the given `ProfileKind` ranges.
/// Uses a fresh time-based seed so each "Testar" click produces new numbers.
pub fn generate_for_kind(kind: ProfileKind) -> FinancialProfile {
    // Time-based seed: changes every millisecond → different on every click
    let seed = chrono::Utc::now().timestamp_millis() as u64;
    generate_synthetic_ranged(seed, kind)
}

/// Generates a realistic synthetic financial profile for a Brazilian user
/// using the user UUID as a stable seed (deterministic per user).
fn generate_synthetic(seed: u64) -> FinancialProfile {
    generate_synthetic_ranged(seed, ProfileKind::YoungProfessional)
}

fn generate_synthetic_ranged(seed: u64, kind: ProfileKind) -> FinancialProfile {
    let mut s = seed;

    let (inc_lo, inc_hi) = kind.income_range();
    let monthly_income_cents = rand_range(&mut s, inc_lo, inc_hi);

    let (cp_lo, cp_hi) = kind.card_pct_range();
    let card_pct = rand_range(&mut s, cp_lo, cp_hi);
    let card_bill_cents = monthly_income_cents * card_pct / 100;

    let card_bill_days = rand_range(&mut s, 2, 28) as u8;

    let (bl_lo, bl_hi) = kind.boleto_range();
    let pending_bills_cents = rand_range(&mut s, bl_lo, bl_hi);

    let (vr_lo, vr_hi) = kind.variable_range();
    let estimated_variable_spend_cents = rand_range(&mut s, vr_lo, vr_hi);

    let (sp_lo, sp_hi) = kind.savings_pct_range();
    let savings_pct = rand_range(&mut s, sp_lo, sp_hi);
    let monthly_savings_goal_cents = monthly_income_cents * savings_pct / 100;

    let days_until_salary = rand_range(&mut s, 1, 30) as u8;

    FinancialProfile {
        card_bill_cents,
        card_bill_days,
        pending_bills_cents,
        estimated_variable_spend_cents,
        monthly_savings_goal_cents,
        days_until_salary,
        monthly_income_cents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn seed_is_deterministic() {
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(seed_from_uuid(id), seed_from_uuid(id));
    }

    #[test]
    fn generate_student_profile_within_range() {
        let p = generate_synthetic_ranged(42, ProfileKind::Student);
        assert!(p.monthly_income_cents >= 120_000 && p.monthly_income_cents <= 180_000);
        assert!(p.card_bill_days >= 2 && p.card_bill_days <= 28);
    }

    #[test]
    fn generate_young_professional_within_range() {
        let p = generate_synthetic_ranged(42, ProfileKind::YoungProfessional);
        assert!(p.monthly_income_cents >= 350_000 && p.monthly_income_cents <= 500_000);
    }

    #[test]
    fn generate_family_within_range() {
        let p = generate_synthetic_ranged(42, ProfileKind::Family);
        assert!(p.monthly_income_cents >= 700_000 && p.monthly_income_cents <= 950_000);
    }

    #[test]
    fn different_seeds_produce_different_profiles() {
        let p1 = generate_synthetic_ranged(1, ProfileKind::YoungProfessional);
        let p2 = generate_synthetic_ranged(999_999, ProfileKind::YoungProfessional);
        assert_ne!(p1.card_bill_cents, p2.card_bill_cents);
    }

    #[test]
    fn profile_kind_from_str() {
        assert_eq!(ProfileKind::from_str("student"),  ProfileKind::Student);
        assert_eq!(ProfileKind::from_str("family"),   ProfileKind::Family);
        assert_eq!(ProfileKind::from_str("anything"), ProfileKind::YoungProfessional);
    }
}
