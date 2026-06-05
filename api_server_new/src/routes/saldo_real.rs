#![forbid(unsafe_code)]

use askama::Template;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    auth::{validate_token, JWT_COOKIE},
    state::AppState,
};
use core_business::{analyze_purchase, fmt_brl, proactive_alerts, AlertLevel, WalletAlert};

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
    pub monthly_income: String,
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

#[derive(Template)]
#[template(path = "saldo_real.html")]
#[allow(dead_code)]
pub struct SaldoRealPage {
    pub balance_display: String,
    pub proactive_alerts: Vec<AlertView>,
    pub analysis: Option<AnalysisView>,
    pub purchase_amount: Option<String>,
}

// ── Query params ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AnalyzeQuery {
    pub amount: Option<String>,
}

// ── Page handler ─────────────────────────────────────────────────────────────

pub async fn get_saldo_real(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<AnalyzeQuery>,
) -> Response {
    let Some(id) = get_user_id(&headers, &state.jwt_secret) else {
        return Redirect::to("/login").into_response();
    };

    let wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(_) => return Redirect::to("/login").into_response(),
    };

    // Perfil vem do banco — gerado pelo post_simulate na wallet
    let profile = match state.profile_repo.find_only(&id).await {
        Ok(Some(p)) => p,
        _ => return Redirect::to("/wallet").into_response(),
    };

    let balance = wallet.balance_cents;
    let p_alerts = proactive_alerts(&profile, balance);
    let (analysis, purchase_amount) = build_analysis(&profile, balance, &params.amount);

    let page = SaldoRealPage {
        balance_display: fmt_brl(balance),
        proactive_alerts: p_alerts.into_iter().map(alert_to_view).collect(),
        analysis,
        purchase_amount,
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
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "not authenticated"})),
        )
            .into_response();
    };

    let wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let profile = match state.profile_repo.get_or_generate(&id).await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    let balance = wallet.balance_cents;

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

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "balance_cents": balance,
            "true_free_balance_cents": profile.true_free_balance(balance),
            "proactive_alerts": p_alerts,
            "analysis": analysis,
        })),
    )
        .into_response()
}

// ── Shared analysis builder ───────────────────────────────────────────────────

fn build_analysis(
    profile: &core_business::FinancialProfile,
    balance: u64,
    amount_raw: &Option<String>,
) -> (Option<AnalysisView>, Option<String>) {
    let Some(raw) = amount_raw else {
        return (None, None);
    };
    let normalized = raw.trim().replace(',', ".");
    let Ok(reais) = normalized.parse::<f64>() else {
        return (None, None);
    };

    let cents = (reais * 100.0).round() as u64;
    let result = analyze_purchase(profile, balance, cents);

    let is_green = result.level == AlertLevel::Green;
    let is_yellow = result.level == AlertLevel::Yellow;

    let level_emoji = match result.level {
        AlertLevel::Green => "✅",
        AlertLevel::Yellow => "⚠️",
        AlertLevel::Red => "🔴",
    }
    .to_string();

    let free = result.true_free_balance_cents;
    let breakdown = BreakdownView {
        monthly_income: fmt_brl(result.breakdown.monthly_income_cents),
        card_bill: fmt_brl(result.breakdown.card_bill_cents),
        card_bill_days: result.breakdown.card_bill_days,
        pending_bills: fmt_brl(result.breakdown.pending_bills_cents),
        variable_spend: fmt_brl(result.breakdown.estimated_variable_spend_cents),
        savings_goal: fmt_brl(result.breakdown.monthly_savings_goal_cents),
        true_free_balance: if free >= 0 {
            fmt_brl(free.unsigned_abs())
        } else {
            format!("-{}", fmt_brl(free.unsigned_abs()))
        },
        true_free_negative: free < 0,
    };

    let installment_text = result
        .installment_suggestion
        .map(|s| format!("{}x de R$ {}", s.installments, fmt_brl(s.installment_cents)));

    let view = AnalysisView {
        level_emoji,
        headline: result.headline,
        detail_lines: result.detail_lines,
        suggestion: result.suggestion,
        installment_text,
        breakdown,
        is_green,
        is_yellow,
    };

    (Some(view), Some(format!("{:.2}", reais)))
}

fn alert_to_view(a: WalletAlert) -> AlertView {
    AlertView {
        is_green: a.level == AlertLevel::Green,
        is_yellow: a.level == AlertLevel::Yellow,
        message: a.message,
    }
}
