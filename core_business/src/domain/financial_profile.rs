#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

// ── Simulated Open Finance profile ──────────────────────────────────────────

/// A financial profile generated synthetically per user,
/// simulating data that would come from Open Finance (bank + card APIs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialProfile {
    /// Estimated credit card bill closing in N days (cents).
    pub card_bill_cents: u64,
    /// Days until the card bill closes.
    pub card_bill_days: u8,
    /// Pending boletos/bills due within 15 days (cents).
    pub pending_bills_cents: u64,
    /// Estimated recurring variable spend until end of month
    /// (e.g. transport, groceries, subscriptions) (cents).
    pub estimated_variable_spend_cents: u64,
    /// User's configured monthly savings goal (cents).
    pub monthly_savings_goal_cents: u64,
    /// Estimated days until next salary deposit.
    pub days_until_salary: u8,
    /// Estimated monthly net income (cents).
    pub monthly_income_cents: u64,
}

impl FinancialProfile {
    /// Computes the "true free balance" — what's really available
    /// after all known and estimated obligations are deducted.
    ///
    /// # Example
    /// ```
    /// use core_business::domain::financial_profile::FinancialProfile;
    /// let profile = FinancialProfile {
    ///     card_bill_cents: 80_000,
    ///     card_bill_days: 4,
    ///     pending_bills_cents: 45_000,
    ///     estimated_variable_spend_cents: 15_000,
    ///     monthly_savings_goal_cents: 30_000,
    ///     days_until_salary: 10,
    ///     monthly_income_cents: 500_000,
    /// };
    /// let free = profile.true_free_balance(0);
    /// assert_eq!(free, 330_000_i64); // R$ 3.300,00
    /// ```
    /// Returns what is left from the salary after all obligations.
    pub fn true_free_balance(&self, _current_balance_cents: u64) -> i64 {
        self.monthly_income_cents as i64
            - self.card_bill_cents as i64
            - self.pending_bills_cents as i64
            - self.estimated_variable_spend_cents as i64
            - self.monthly_savings_goal_cents as i64
    }
}

// ── Alert engine output ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertLevel {
    Green,
    Yellow,
    Red,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseAnalysis {
    pub level: AlertLevel,
    pub true_free_balance_cents: i64,
    pub free_balance_after_purchase_cents: i64,
    pub headline: String,
    pub detail_lines: Vec<String>,
    pub suggestion: Option<String>,
    pub installment_suggestion: Option<InstallmentSuggestion>,
    /// Breakdown of deductions used in the calculation.
    pub breakdown: SpendingBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallmentSuggestion {
    pub installments: u8,
    pub installment_cents: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingBreakdown {
    pub monthly_income_cents: u64,
    pub card_bill_cents: u64,
    pub card_bill_days: u8,
    pub pending_bills_cents: u64,
    pub estimated_variable_spend_cents: u64,
    pub monthly_savings_goal_cents: u64,
}

// ── Pure analysis engine ─────────────────────────────────────────────────────

/// Analyzes whether `purchase_cents` is safe given the user's financial profile
/// and current wallet balance.
///
/// This is a pure function — no I/O, fully testable.
pub fn analyze_purchase(
    profile: &FinancialProfile,
    current_balance_cents: u64,
    purchase_cents: u64,
) -> PurchaseAnalysis {
    let true_free = profile.true_free_balance(current_balance_cents);
    let after_purchase = true_free - purchase_cents as i64;

    let breakdown = SpendingBreakdown {
        monthly_income_cents: profile.monthly_income_cents,
        card_bill_cents: profile.card_bill_cents,
        card_bill_days: profile.card_bill_days,
        pending_bills_cents: profile.pending_bills_cents,
        estimated_variable_spend_cents: profile.estimated_variable_spend_cents,
        monthly_savings_goal_cents: profile.monthly_savings_goal_cents,
    };

    // ── Green: purchase fits comfortably ────────────────────────────────────
    if after_purchase >= 0 {
        return PurchaseAnalysis {
            level: AlertLevel::Green,
            true_free_balance_cents: true_free,
            free_balance_after_purchase_cents: after_purchase,
            headline: "✅ Good to go! Your Free Balance covers this purchase.".into(),
            detail_lines: vec![
                format!(
                    "Your Real Free Balance is R$ {}, after deducting all projected expenses.",
                    fmt_brl(true_free.unsigned_abs())
                ),
                format!(
                    "After this purchase you will still have R$ {} free.",
                    fmt_brl(after_purchase.unsigned_abs())
                ),
                format!(
                    "Your savings goal of R$ {} this month remains intact. 🎯",
                    fmt_brl(profile.monthly_savings_goal_cents)
                ),
            ],
            suggestion: None,
            installment_suggestion: None,
            breakdown,
        };
    }

    // ── Red: purchase would cause negative free balance ──────────────────────
    let shortfall = after_purchase.unsigned_abs();

    let installment_suggestion = (2u8..=6).find_map(|n| {
        let per_installment = (purchase_cents + n as u64 - 1) / n as u64;
        let projected_free_monthly = (profile.monthly_income_cents as i64
            - profile.card_bill_cents as i64
            - profile.pending_bills_cents as i64
            - profile.estimated_variable_spend_cents as i64
            - profile.monthly_savings_goal_cents as i64)
            .max(0) as u64;
        if per_installment <= projected_free_monthly {
            Some(InstallmentSuggestion {
                installments: n,
                installment_cents: per_installment,
            })
        } else {
            None
        }
    });

    let mut detail_lines = vec![
        format!(
            "Your Real Free Balance is R$ {}, calculated from your salary of R$ {} minus all obligations.",
            fmt_brl(true_free.unsigned_abs()),
            fmt_brl(profile.monthly_income_cents)
        ),
        format!(
            "If you buy now, you will be short R$ {} to cover your obligations.",
            fmt_brl(shortfall)
        ),
    ];

    if profile.card_bill_days <= 7 {
        detail_lines.push(format!(
            "⚠️ Your card bill of R$ {} closes in just {} days.",
            fmt_brl(profile.card_bill_cents),
            profile.card_bill_days
        ));
    }

    if profile.pending_bills_cents > 0 {
        detail_lines.push(format!(
            "📋 You have R$ {} in bills due within the next 15 days.",
            fmt_brl(profile.pending_bills_cents)
        ));
    }

    detail_lines.push(format!(
        "🎯 This purchase also cancels your savings goal of R$ {} this month.",
        fmt_brl(profile.monthly_savings_goal_cents)
    ));

    let suggestion = installment_suggestion.as_ref().map(|s| {
        format!(
            "💡 Splitting into {}x of R$ {} would fit your Free Balance over the next months without compromising your investments.",
            s.installments,
            fmt_brl(s.installment_cents)
        )
    }).or_else(|| Some("💡 Consider postponing this purchase until next payday or revising your savings goal.".into()));

    let level = if true_free >= 0 {
        AlertLevel::Yellow
    } else {
        AlertLevel::Red
    };

    let headline = match level {
        AlertLevel::Yellow => {
            "⚠️ Rethink this Purchase — your Free Balance does not cover it.".into()
        }
        _ => "🔴 Risky Purchase — you will exceed your obligations.".into(),
    };

    PurchaseAnalysis {
        level,
        true_free_balance_cents: true_free,
        free_balance_after_purchase_cents: after_purchase,
        headline,
        detail_lines,
        suggestion,
        installment_suggestion,
        breakdown,
    }
}

// ── Proactive wallet alerts (no purchase amount needed) ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAlert {
    pub level: AlertLevel,
    pub message: String,
}

/// Generates proactive alerts based solely on the user's financial profile
/// and current balance — shown on the wallet dashboard without any purchase query.
pub fn proactive_alerts(
    profile: &FinancialProfile,
    current_balance_cents: u64,
) -> Vec<WalletAlert> {
    let mut alerts = Vec::new();
    let true_free = profile.true_free_balance(current_balance_cents);

    let goal_met = current_balance_cents >= profile.monthly_savings_goal_cents;

    if true_free < 0 {
        alerts.push(WalletAlert {
            level: AlertLevel::Red,
            message: format!(
                "🔴 Negative Real Free Balance! Your obligations exceed your salary by R$ {}. Avoid new expenses.",
                fmt_brl(true_free.unsigned_abs())
            ),
        });
    } else if !goal_met {
        alerts.push(WalletAlert {
            level: AlertLevel::Yellow,
            message: format!(
                "🟡 Savings goal (R$ {}) not yet reached. Invested so far: R$ {}.",
                fmt_brl(profile.monthly_savings_goal_cents),
                fmt_brl(current_balance_cents)
            ),
        });
    } else {
        alerts.push(WalletAlert {
            level: AlertLevel::Green,
            message: format!(
                "🟢 Savings goal reached! R$ {} invested (goal: R$ {}).",
                fmt_brl(current_balance_cents),
                fmt_brl(profile.monthly_savings_goal_cents)
            ),
        });
    }

    if profile.card_bill_days <= 5 {
        alerts.push(WalletAlert {
            level: AlertLevel::Red,
            message: format!(
                "🔴 Card bill of R$ {} closes in {} days. Make sure your balance covers it.",
                fmt_brl(profile.card_bill_cents),
                profile.card_bill_days
            ),
        });
    } else if profile.card_bill_days <= 10 {
        alerts.push(WalletAlert {
            level: AlertLevel::Yellow,
            message: format!(
                "🟡 Card bill of R$ {} closes in {} days.",
                fmt_brl(profile.card_bill_cents),
                profile.card_bill_days
            ),
        });
    }

    if profile.days_until_salary <= 5 {
        alerts.push(WalletAlert {
            level: AlertLevel::Green,
            message: format!(
                "🟢 Salary expected in {} days (R$ {}).",
                profile.days_until_salary,
                fmt_brl(profile.monthly_income_cents)
            ),
        });
    }

    alerts
}

// ── Formatting helper ────────────────────────────────────────────────────────

pub fn fmt_brl(cents: u64) -> String {
    let reais = cents / 100;
    let centavos = cents % 100;
    let reais_str = reais.to_string();
    let with_sep = reais_str
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(".");
    format!("{},{:02}", with_sep, centavos)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    fn profile() -> FinancialProfile {
        FinancialProfile {
            card_bill_cents: 80_000,
            card_bill_days: 4,
            pending_bills_cents: 45_000,
            estimated_variable_spend_cents: 15_000,
            monthly_savings_goal_cents: 30_000,
            days_until_salary: 10,
            monthly_income_cents: 500_000,
        }
    }

    #[test]
    fn true_free_balance_scenario() {
        // 500_000 - 80_000 - 45_000 - 15_000 - 30_000 = 330_000
        assert_eq!(profile().true_free_balance(0), 330_000);
    }

    #[test]
    fn analyze_green_when_purchase_fits() {
        let p = FinancialProfile {
            card_bill_cents: 10_000,
            card_bill_days: 20,
            pending_bills_cents: 5_000,
            estimated_variable_spend_cents: 5_000,
            monthly_savings_goal_cents: 10_000,
            days_until_salary: 15,
            // free = 500_000 - 30_000 = 470_000; purchase = 20_000; after = 450_000
            monthly_income_cents: 500_000,
        };
        let result = analyze_purchase(&p, 0, 20_000);
        assert_eq!(result.level, AlertLevel::Green);
        assert_eq!(result.free_balance_after_purchase_cents, 450_000);
    }

    #[test]
    fn analyze_red_when_purchase_causes_shortfall() {
        // free = 330_000; purchase = 400_000 -> after = -70_000
        // free was positive before purchase -> Yellow
        // To get Red, need free already negative: income too low
        let mut p = profile();
        p.monthly_income_cents = 100_000; // free = 100_000 - 170_000 = -70_000 (already negative)
        let result = analyze_purchase(&p, 0, 1_000);
        assert_eq!(result.level, AlertLevel::Red);
        assert!(result.free_balance_after_purchase_cents < 0);
    }

    #[test]
    fn analyze_yellow_when_free_positive_but_purchase_exceeds() {
        let p = FinancialProfile {
            card_bill_cents: 10_000,
            card_bill_days: 20,
            pending_bills_cents: 5_000,
            estimated_variable_spend_cents: 5_000,
            monthly_savings_goal_cents: 10_000,
            days_until_salary: 15,
            // free = 70_000 - 30_000 = 40_000; purchase = 50_000 -> yellow
            monthly_income_cents: 70_000,
        };
        let result = analyze_purchase(&p, 0, 50_000);
        assert_eq!(result.level, AlertLevel::Yellow);
    }

    #[test]
    fn fmt_brl_formats_correctly() {
        assert_eq!(fmt_brl(150_000), "1.500,00");
        assert_eq!(fmt_brl(35_000), "350,00");
        assert_eq!(fmt_brl(100), "1,00");
    }

    #[test]
    fn proactive_alerts_healthy_when_income_covers_expenses() {
        // invested (80_000) > goal (30_000) and true_free > 0 -> green
        let alerts = proactive_alerts(&profile(), 80_000);
        assert!(alerts.iter().any(|a| a.level == AlertLevel::Green));
    }

    #[test]
    fn proactive_alerts_yellow_when_goal_not_met() {
        // invested (10_000) < goal (30_000) -> yellow
        let alerts = proactive_alerts(&profile(), 10_000);
        assert!(alerts.iter().any(|a| a.level == AlertLevel::Yellow));
    }

    // ── Property-based tests ─────────────────────────────────────────────────

    proptest! {
        /// Green analysis must always have non-negative after_purchase balance.
        #[test]
        fn prop_green_result_is_never_negative(
            income in 100_000u64..10_000_000u64,
            obligations in 0u64..50_000u64,
        ) {
            let p = FinancialProfile {
                monthly_income_cents: income,
                card_bill_cents: obligations,
                card_bill_days: 15,
                pending_bills_cents: 0,
                estimated_variable_spend_cents: 0,
                monthly_savings_goal_cents: 0,
                days_until_salary: 10,
            };
            let free = p.true_free_balance(0);
            // purchase fits exactly
            if free >= 0 {
                let result = analyze_purchase(&p, 0, free.unsigned_abs());
                prop_assert_eq!(result.level, AlertLevel::Green);
                prop_assert!(result.free_balance_after_purchase_cents >= 0);
            }
        }

        /// true_free_balance must equal income minus the sum of all obligations.
        #[test]
        fn prop_true_free_balance_arithmetic(
            income in 0u64..50_000_000u64,
            card   in 0u64..10_000_000u64,
            bills  in 0u64..10_000_000u64,
            var_sp in 0u64..10_000_000u64,
            goal   in 0u64..10_000_000u64,
        ) {
            let p = FinancialProfile {
                monthly_income_cents: income,
                card_bill_cents: card,
                card_bill_days: 5,
                pending_bills_cents: bills,
                estimated_variable_spend_cents: var_sp,
                monthly_savings_goal_cents: goal,
                days_until_salary: 10,
            };
            let expected = income as i64 - card as i64 - bills as i64 - var_sp as i64 - goal as i64;
            prop_assert_eq!(p.true_free_balance(0), expected);
        }

        /// fmt_brl must never panic and must always contain a comma separator.
        #[test]
        fn prop_fmt_brl_never_panics_and_contains_comma(cents in 0u64..1_000_000_000u64) {
            let result = fmt_brl(cents);
            prop_assert!(result.contains(','));
        }

        /// A zero-value purchase never flips a positive free balance to negative.
        #[test]
        fn prop_zero_purchase_does_not_change_free_balance(
            income in 50_000u64..5_000_000u64,
        ) {
            let p = FinancialProfile {
                monthly_income_cents: income,
                card_bill_cents: 0,
                card_bill_days: 20,
                pending_bills_cents: 0,
                estimated_variable_spend_cents: 0,
                monthly_savings_goal_cents: 0,
                days_until_salary: 10,
            };
            let result = analyze_purchase(&p, 0, 0);
            prop_assert_eq!(result.free_balance_after_purchase_cents, p.true_free_balance(0));
        }
    }
}
