#![forbid(unsafe_code)]

use std::sync::Arc;
use askama::Template;
use askama_axum::IntoResponse as AskamaIntoResponse;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use core_business::{analyze_purchase, proactive_alerts, fmt_brl, AlertLevel, WalletAlert};
use infra_data::{ProfileKind, generate_for_kind};
use crate::{auth::{validate_token, JWT_COOKIE}, state::AppState};

// ── Auth helper ──────────────────────────────────────────────────────────────

fn get_user_id(headers: &HeaderMap, secret: &str) -> Option<core_business::UserId> {
    let cookies = headers.get("cookie")?.to_str().ok()?;
    let token = cookies
        .split(';')
        .map(str::trim)
        .find(|c| c.starts_with(JWT_COOKIE))?
        .split_once('=')?
        .1;
    let claims = validate_token(token, secret).ok()?;
    let uuid = uuid::Uuid::parse_str(&claims.sub).ok()?;
    Some(core_business::UserId(uuid))
}

// ── Template structs ─────────────────────────────────────────────────────────

pub struct BreakdownView {
    pub current_balance: String,
    pub card_bill: String,
    pub card_bill_days: u8,
    pub pending_bills: String,
    pub variable_spend: String,
    pub savings_goal: String,
    pub true_free_balance: String,
    pub true_free_negative: bool,
}

pub struct AnalysisView {
    pub level_emoji: String,
    pub headline: String,
    pub detail_lines: Vec<String>,
    pub suggestion: Option<String>,
    pub installment_text: Option<String>,
    pub breakdown: BreakdownView,
    pub is_green: bool,
    pub is_yellow: bool,
}

pub struct AlertView {
    pub message: String,
    pub is_green: bool,
    pub is_yellow: bool,
}

pub struct SimProfileView {
    pub label: String,
    pub income: String,
    pub card_bill: String,
    pub pending_bills: String,
    pub variable_spend: String,
    pub savings_goal: String,
    pub days_until_salary: u8,
}

#[derive(Template)]
#[template(path = "saldo_real.html")]
#[allow(dead_code)]
pub struct SaldoRealPage {
    pub balance_display: String,
    pub proactive_alerts: Vec<AlertView>,
    pub analysis: Option<AnalysisView>,
    pub purchase_amount: Option<String>,
    pub sim_profile: Option<SimProfileView>,
    pub active_kind: String,
    pub is_student: bool,
    pub is_young: bool,
    pub is_family: bool,
}

// ── Query params ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AnalyzeQuery {
    pub amount:   Option<String>,
    /// "student" | "young" | "family" — triggers simulation mode
    pub profile:  Option<String>,
}

// ── Page handler ─────────────────────────────────────────────────────────────

pub async fn get_saldo_real(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<AnalyzeQuery>,
) -> Response {
    use axum::response::Redirect;

    let Some(id) = get_user_id(&headers, &state.jwt_secret) else {
        return Redirect::to("/login").into_response();
    };

    let wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(_) => return Redirect::to("/login").into_response(),
    };

    // ── Choose profile: simulation or persistent ─────────────────────────────
    let (profile, sim_profile, active_kind, is_student, is_young, is_family) = if let Some(ref kind_str) = params.profile {
        let kind = ProfileKind::from_str(kind_str);
        let p = generate_for_kind(kind);
        let view = SimProfileView {
            label:             kind.label().to_string(),
            income:            fmt_brl(p.monthly_income_cents),
            card_bill:         fmt_brl(p.card_bill_cents),
            pending_bills:     fmt_brl(p.pending_bills_cents),
            variable_spend:    fmt_brl(p.estimated_variable_spend_cents),
            savings_goal:      fmt_brl(p.monthly_savings_goal_cents),
            days_until_salary: p.days_until_salary,
        };
        let is_student = kind == ProfileKind::Student;
        let is_young   = kind == ProfileKind::YoungProfessional;
        let is_family  = kind == ProfileKind::Family;
        (p, Some(view), kind_str.clone(), is_student, is_young, is_family)
    } else {
        let p = match state.profile_repo.get_or_generate(&id).await {
            Ok(p) => p,
            Err(_) => return Redirect::to("/wallet").into_response(),
        };
        (p, None, String::new(), false, false, false)
    };

    // In simulation mode use a fixed illustrative balance; otherwise use real wallet
    let balance = if params.profile.is_some() {
        // Balance = ~60% of simulated income (realistic "mid-month" scenario)
        (profile.monthly_income_cents as f64 * 0.6).round() as u64
    } else {
        wallet.balance_cents
    };

    let p_alerts = proactive_alerts(&profile, balance);

    let (analysis, purchase_amount) = build_analysis(&profile, balance, &params.amount);

    use askama::Template;
    let page = SaldoRealPage {
        balance_display: fmt_brl(balance),
        proactive_alerts: p_alerts.into_iter().map(alert_to_view).collect(),
        analysis,
        purchase_amount,
        sim_profile,
        active_kind,
        is_student,
        is_young,
        is_family,
    };
    axum::response::Html(page.render().unwrap()).into_response()
}

// ── JSON API ─────────────────────────────────────────────────────────────────

pub async fn api_analyze(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<AnalyzeQuery>,
) -> impl IntoResponse {
    use axum::http::StatusCode;

    let Some(id) = get_user_id(&headers, &state.jwt_secret) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "not authenticated"}))).into_response();
    };

    let wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(e) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    };

    let (profile, balance) = if let Some(ref kind_str) = params.profile {
        let p = generate_for_kind(ProfileKind::from_str(kind_str));
        let b = (p.monthly_income_cents as f64 * 0.6).round() as u64;
        (p, b)
    } else {
        match state.profile_repo.get_or_generate(&id).await {
            Ok(p) => {
                let b = wallet.balance_cents;
                (p, b)
            }
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        }
    };

    let p_alerts: Vec<_> = proactive_alerts(&profile, balance)
        .into_iter()
        .map(|a| serde_json::json!({"level": format!("{:?}", a.level), "message": a.message}))
        .collect();

    let analysis = params.amount.as_ref().and_then(|raw| {
        let normalized = raw.trim().replace(',', ".");
        normalized.parse::<f64>().ok().map(|reais| {
            let cents = (reais * 100.0).round() as u64;
            analyze_purchase(&profile, balance, cents)
        })
    });

    (StatusCode::OK, Json(serde_json::json!({
        "balance_cents": balance,
        "true_free_balance_cents": profile.true_free_balance(balance),
        "proactive_alerts": p_alerts,
        "analysis": analysis,
    }))).into_response()
}

// ── Shared analysis builder ───────────────────────────────────────────────────

fn build_analysis(
    profile: &core_business::FinancialProfile,
    balance: u64,
    amount_raw: &Option<String>,
) -> (Option<AnalysisView>, Option<String>) {
    let Some(raw) = amount_raw else { return (None, None) };
    let normalized = raw.trim().replace(',', ".");
    let Ok(reais) = normalized.parse::<f64>() else { return (None, None) };

    let cents  = (reais * 100.0).round() as u64;
    let result = analyze_purchase(profile, balance, cents);

    let is_green  = result.level == AlertLevel::Green;
    let is_yellow = result.level == AlertLevel::Yellow;

    let level_emoji = match result.level {
        AlertLevel::Green  => "✅",
        AlertLevel::Yellow => "⚠️",
        AlertLevel::Red    => "🔴",
    }.to_string();

    let free = result.true_free_balance_cents;
    let breakdown = BreakdownView {
        current_balance:   fmt_brl(result.breakdown.current_balance_cents),
        card_bill:         fmt_brl(result.breakdown.card_bill_cents),
        card_bill_days:    result.breakdown.card_bill_days,
        pending_bills:     fmt_brl(result.breakdown.pending_bills_cents),
        variable_spend:    fmt_brl(result.breakdown.estimated_variable_spend_cents),
        savings_goal:      fmt_brl(result.breakdown.monthly_savings_goal_cents),
        true_free_balance: if free >= 0 {
            fmt_brl(free.unsigned_abs())
        } else {
            format!("-{}", fmt_brl(free.unsigned_abs()))
        },
        true_free_negative: free < 0,
    };

    let installment_text = result.installment_suggestion
        .map(|s| format!("{}x de R$ {}", s.installments, fmt_brl(s.installment_cents)));

    let view = AnalysisView {
        level_emoji,
        headline:         result.headline,
        detail_lines:     result.detail_lines,
        suggestion:       result.suggestion,
        installment_text,
        breakdown,
        is_green,
        is_yellow,
    };

    (Some(view), Some(format!("{:.2}", reais)))
}

fn alert_to_view(a: WalletAlert) -> AlertView {
    AlertView {
        is_green:  a.level == AlertLevel::Green,
        is_yellow: a.level == AlertLevel::Yellow,
        message:   a.message,
    }
}
