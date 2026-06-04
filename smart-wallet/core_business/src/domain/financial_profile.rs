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
    /// let free = profile.true_free_balance(150_000);
    /// assert_eq!(free, -20_000_i64); // R$ -200,00
    /// ```
    pub fn true_free_balance(&self, current_balance_cents: u64) -> i64 {
        current_balance_cents as i64
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
    pub current_balance_cents: u64,
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
        current_balance_cents,
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
            headline: "✅ Pode gastar! Seu Saldo Livre cobre essa compra.".into(),
            detail_lines: vec![
                format!(
                    "Seu Saldo Livre Real é de R$ {}, após descontar todas as contas previstas.",
                    fmt_brl(true_free.unsigned_abs())
                ),
                format!(
                    "Após essa compra ainda sobrarão R$ {} livres.",
                    fmt_brl(after_purchase.unsigned_abs())
                ),
                format!(
                    "Sua meta de guardar R$ {} este mês continua preservada. 🎯",
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

    // Check if installments would help (up to 6x, each fitting in future months)
    let installment_suggestion = (2u8..=6).find_map(|n| {
        let per_installment = (purchase_cents + n as u64 - 1) / n as u64; // ceiling div
        // fits if each installment is covered by projected free monthly income
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
            "Seu saldo no banco é R$ {}, mas seu Saldo Livre Real é de apenas R$ {}.",
            fmt_brl(current_balance_cents),
            if true_free >= 0 {
                fmt_brl(true_free.unsigned_abs())
            } else {
                format!("-R$ {}", fmt_brl(true_free.unsigned_abs()))
            }
        ),
        format!(
            "Se comprar agora, vai faltar R$ {} para cobrir suas obrigações.",
            fmt_brl(shortfall)
        ),
    ];

    if profile.card_bill_days <= 7 {
        detail_lines.push(format!(
            "⚠️ Sua fatura de R$ {} fecha em apenas {} dias.",
            fmt_brl(profile.card_bill_cents),
            profile.card_bill_days
        ));
    }

    if profile.pending_bills_cents > 0 {
        detail_lines.push(format!(
            "📋 Você tem R$ {} em boletos vencendo nos próximos 15 dias.",
            fmt_brl(profile.pending_bills_cents)
        ));
    }

    detail_lines.push(format!(
        "🎯 Esta compra também cancela sua meta de guardar R$ {} este mês.",
        fmt_brl(profile.monthly_savings_goal_cents)
    ));

    let suggestion = installment_suggestion.as_ref().map(|s| {
        format!(
            "💡 Parcelar em {}x de R$ {} caberia no seu Saldo Livre dos próximos meses sem comprometer seus investimentos.",
            s.installments,
            fmt_brl(s.installment_cents)
        )
    }).or_else(|| Some("💡 Considere adiar essa compra até o próximo salário ou rever sua meta de economia.".into()));

    // Yellow: free balance is negative but bank balance technically covers it
    let level = if true_free >= 0 {
        AlertLevel::Yellow
    } else {
        AlertLevel::Red
    };

    let headline = match level {
        AlertLevel::Yellow => "⚠️ Repense a Compra — seu Saldo Livre não cobre isso.".into(),
        _ => "🔴 Compra Arriscada — você vai estourar suas obrigações.".into(),
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

    if true_free < 0 {
        alerts.push(WalletAlert {
            level: AlertLevel::Red,
            message: format!(
                "🔴 Saldo Livre Real negativo! Suas obrigações superam seu saldo em R$ {}. Evite novas despesas.",
                fmt_brl(true_free.unsigned_abs())
            ),
        });
    } else if (true_free as u64) < profile.monthly_savings_goal_cents {
        alerts.push(WalletAlert {
            level: AlertLevel::Yellow,
            message: format!(
                "🟡 Seu Saldo Livre Real (R$ {}) é menor que sua meta de reserva (R$ {}). Cuidado com novos gastos.",
                fmt_brl(true_free.unsigned_abs()),
                fmt_brl(profile.monthly_savings_goal_cents)
            ),
        });
    } else {
        alerts.push(WalletAlert {
            level: AlertLevel::Green,
            message: format!(
                "🟢 Finanças saudáveis! Saldo Livre Real de R$ {} e meta de reserva preservada.",
                fmt_brl(true_free.unsigned_abs())
            ),
        });
    }

    if profile.card_bill_days <= 5 {
        alerts.push(WalletAlert {
            level: AlertLevel::Red,
            message: format!(
                "🔴 Fatura de R$ {} fecha em {} dias. Certifique-se que o saldo cobre.",
                fmt_brl(profile.card_bill_cents),
                profile.card_bill_days
            ),
        });
    } else if profile.card_bill_days <= 10 {
        alerts.push(WalletAlert {
            level: AlertLevel::Yellow,
            message: format!(
                "🟡 Fatura de R$ {} fecha em {} dias.",
                fmt_brl(profile.card_bill_cents),
                profile.card_bill_days
            ),
        });
    }

    if profile.days_until_salary <= 5 {
        alerts.push(WalletAlert {
            level: AlertLevel::Green,
            message: format!(
                "🟢 Salário previsto em {} dias (R$ {}).",
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

    fn profile() -> FinancialProfile {
        FinancialProfile {
            card_bill_cents:               80_000,
            card_bill_days:                4,
            pending_bills_cents:           45_000,
            estimated_variable_spend_cents: 15_000,
            monthly_savings_goal_cents:    30_000,
            days_until_salary:             10,
            monthly_income_cents:         500_000,
        }
    }

    #[test]
    fn true_free_balance_scenario() {
        // 150_000 - 80_000 - 45_000 - 15_000 - 30_000 = -20_000
        assert_eq!(profile().true_free_balance(150_000), -20_000);
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
            monthly_income_cents: 500_000,
        };
        // free = 100_000 - 30_000 = 70_000; purchase = 20_000; after = 50_000
        let result = analyze_purchase(&p, 100_000, 20_000);
        assert_eq!(result.level, AlertLevel::Green);
        assert_eq!(result.free_balance_after_purchase_cents, 50_000);
    }

    #[test]
    fn analyze_red_when_purchase_causes_shortfall() {
        let result = analyze_purchase(&profile(), 150_000, 35_000);
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
            monthly_income_cents: 500_000,
        };
        // free = 40_000; purchase = 50_000 → yellow (free was positive, now negative)
        let result = analyze_purchase(&p, 70_000, 50_000);
        assert_eq!(result.level, AlertLevel::Yellow);
    }

    #[test]
    fn fmt_brl_formats_correctly() {
        assert_eq!(fmt_brl(150_000), "1.500,00");
        assert_eq!(fmt_brl(35_000), "350,00");
        assert_eq!(fmt_brl(100), "1,00");
    }

    #[test]
    fn proactive_alerts_returns_red_when_negative_free_balance() {
        let alerts = proactive_alerts(&profile(), 150_000);
        assert!(alerts.iter().any(|a| a.level == AlertLevel::Red));
    }
}
