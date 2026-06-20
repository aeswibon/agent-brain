//! Operator-facing product metrics from retrieval_log + index composition.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::adoption::{self, AdoptionMilestones};
use crate::config::Config;
use crate::db::store::{BrainStore, RetrievalStats};

/// Rough USD per 1M input tokens (Sonnet-class ballpark for ROI estimates).
pub const INPUT_TOKEN_USD_PER_MILLION: f64 = 3.0;

#[derive(serde::Deserialize)]
struct StoredProofReport {
    generated_at: String,
    fixture_skills: usize,
    eval: StoredEvalReport,
    latency: StoredLatencyReport,
}

#[derive(serde::Deserialize)]
struct StoredEvalReport {
    recall_at_3: f64,
    cases: usize,
}

#[derive(serde::Deserialize)]
struct StoredLatencyReport {
    warm_route: StoredPercentileMs,
}

#[derive(serde::Deserialize)]
struct StoredPercentileMs {
    p95_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexBreakdown {
    pub total_indexed: usize,
    pub active_memories: usize,
    pub by_type: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProofSnapshot {
    pub generated_at: String,
    pub recall_at_3: f64,
    pub eval_cases: usize,
    pub fixture_skills: usize,
    pub p95_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValueSummary {
    pub combined_tokens_saved: u64,
    pub estimated_api_cost_usd: f64,
    pub memories_committed: usize,
    pub blocked_full_reads: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperatorStats {
    pub period_days: u32,
    pub index: IndexBreakdown,
    pub period: RetrievalStats,
    pub value: ValueSummary,
    pub adoption: AdoptionMilestones,
    pub proof: Option<ProofSnapshot>,
}

pub fn collect(store: &BrainStore, config: &Config, period_days: u32) -> Result<OperatorStats> {
    adoption::ensure_installed_at(store)?;
    let now = chrono::Utc::now().timestamp_millis();
    let since = now - i64::from(period_days) * 24 * 3600 * 1000;
    let _ = crate::tool_events::ingest_hook_events_since(store, &config.home, since);
    let period = store.retrieval_stats_since(since)?;
    let mut by_type = BTreeMap::new();
    for (item_type, count) in store.count_indexed_by_type()? {
        by_type.insert(item_type, count);
    }
    let index = IndexBreakdown {
        total_indexed: store.count_indexed_items()?,
        active_memories: store.count_active_facts()?,
        by_type,
    };
    let adoption = adoption::load_milestones(store)?;
    let proof = load_proof_snapshot(&config.home);
    let combined_tokens_saved = period
        .total_saved_tokens
        .saturating_add(period.tool_tokens_saved);
    let value = ValueSummary {
        combined_tokens_saved,
        estimated_api_cost_usd: estimate_cost_usd(combined_tokens_saved),
        memories_committed: store.count_facts_created_since(since)?,
        blocked_full_reads: period.inefficient_read_events,
    };
    Ok(OperatorStats {
        period_days,
        index,
        period,
        value,
        adoption,
        proof,
    })
}

pub fn estimate_cost_usd(tokens_saved: u64) -> f64 {
    tokens_saved as f64 / 1_000_000.0 * INPUT_TOKEN_USD_PER_MILLION
}

pub fn format_tokens_short(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn load_proof_snapshot(home: &Path) -> Option<ProofSnapshot> {
    let path = home.join("metrics/proof-latest.json");
    let body = fs::read_to_string(&path).ok()?;
    let report: StoredProofReport = serde_json::from_str(&body).ok()?;
    Some(ProofSnapshot {
        generated_at: report.generated_at,
        recall_at_3: report.eval.recall_at_3,
        eval_cases: report.eval.cases,
        fixture_skills: report.fixture_skills,
        p95_latency_ms: report.latency.warm_route.p95_ms,
    })
}

pub fn persist_proof_snapshot(home: &Path, report: &crate::proofs::ProofReport) -> Result<()> {
    let dir = home.join("metrics");
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join("proof-latest.json");
    let json = serde_json::to_string_pretty(report)?;
    fs::write(&path, format!("{json}\n")).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn format_text(stats: &OperatorStats) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# agent-brain stats ({} days)\n\n",
        stats.period_days
    ));

    out.push_str("## Index\n\n");
    out.push_str(&format!(
        "- Indexed items: {}\n- Active memories: {}\n",
        stats.index.total_indexed, stats.index.active_memories
    ));
    if !stats.index.by_type.is_empty() {
        out.push_str("\n");
        for (item_type, count) in &stats.index.by_type {
            out.push_str(&format!("- {item_type}: {count}\n"));
        }
    }

    out.push_str("\n## Routing (period)\n\n");
    out.push_str(&format!(
        "- Route calls: {}\n- Cache hit rate: {:.1}%\n- Avg latency: {:.0}ms · P95: {}ms\n",
        stats.period.route_calls,
        stats.period.cache_hit_rate * 100.0,
        stats.period.avg_latency_ms,
        stats.period.p95_latency_ms
    ));
    if stats.period.routes_with_savings > 0 {
        out.push_str(&format!(
            "- Token savings: ~{:.0}% avg across {} routes · ~{} tok saved (est.)\n- Avg routed tokens: {:.0}\n",
            stats.period.avg_saved_pct,
            stats.period.routes_with_savings,
            stats.period.total_saved_tokens,
            stats.period.avg_routed_tokens
        ));
    } else {
        out.push_str(
            "- Token savings: no routes logged yet (send an agent turn with route_task)\n",
        );
    }
    if stats.period.routes_with_constraints > 0 {
        out.push_str(&format!(
            "- Supervisor: {} routes enforced must_apply ({} constraints total)\n",
            stats.period.routes_with_constraints, stats.period.total_must_apply
        ));
    }
    if stats.period.tool_calls > 0 {
        out.push_str(&format!(
            "- Token tools: {} calls · ~{} tok saved (est.) · {:.0}% avg tool savings\n",
            stats.period.tool_calls,
            stats.period.tool_tokens_saved,
            stats.period.tool_avg_savings_pct
        ));
    }
    if stats.period.inefficient_read_events > 0 {
        out.push_str(&format!(
            "- Inefficient Read steers: {} (use grep_search / read_file_head)\n",
            stats.period.inefficient_read_events
        ));
    }

    out.push_str("\n## Value (period)\n\n");
    out.push_str(&format!(
        "- Combined token savings: ~{} tokens (route + tools)\n- Estimated API cost avoided: ${:.2}\n- Memories committed: {}\n- Full-read steers: {}\n",
        format_tokens_short(stats.value.combined_tokens_saved),
        stats.value.estimated_api_cost_usd,
        stats.value.memories_committed,
        stats.value.blocked_full_reads,
    ));
    if stats.period.route_calls == 0 && stats.value.combined_tokens_saved == 0 {
        out.push_str("- Tip: run Agent mode once so route_task logs savings, then refresh with `agent-brain dashboard --open`\n");
    }

    out.push_str("\n## Adoption (local)\n\n");
    out.push_str(&format!(
        "- Installed: {}\n- First route: {}\n- Starter pack: {}\n- Supervisor pack: {}\n",
        stats.adoption.installed_at.as_deref().unwrap_or("—"),
        stats.adoption.first_route_at.as_deref().unwrap_or("—"),
        stats.adoption.starter_pack_at.as_deref().unwrap_or("—"),
        stats.adoption.supervisor_pack_at.as_deref().unwrap_or("—"),
    ));

    if let Some(proof) = &stats.proof {
        out.push_str("\n## Last proof snapshot\n\n");
        out.push_str(&format!(
            "- Generated: {}\n- Recall@3: {:.1}% ({}/{} cases)\n- Fixture skills: {}\n- P95 warm route: {}ms\n",
            proof.generated_at,
            proof.recall_at_3 * 100.0,
            (proof.recall_at_3 * proof.eval_cases as f64).round() as usize,
            proof.eval_cases,
            proof.fixture_skills,
            proof.p95_latency_ms
        ));
    }

    out
}

/// One-line summary for doctor / onboarding.
pub fn format_summary_line(stats: &OperatorStats) -> String {
    if stats.period.route_calls == 0 {
        return format!(
            "Index: {} items · no routes in last {}d — run Agent mode once, then: agent-brain stats",
            stats.index.total_indexed,
            stats.period_days
        );
    }
    let savings = if stats.period.routes_with_savings > 0 || stats.value.combined_tokens_saved > 0 {
        if stats.value.combined_tokens_saved > 0 {
            format!(
                "~{} tok saved (~${:.2})",
                format_tokens_short(stats.value.combined_tokens_saved),
                stats.value.estimated_api_cost_usd
            )
        } else {
            format!("~{:.0}% token savings", stats.period.avg_saved_pct)
        }
    } else {
        "savings pending".into()
    };
    let supervisor = if stats.period.routes_with_constraints > 0 {
        format!(
            " · must_apply on {} routes",
            stats.period.routes_with_constraints
        )
    } else {
        String::new()
    };
    format!(
        "{} routes ({}d) · {}{} · p95 {}ms · index {}",
        stats.period.route_calls,
        stats.period_days,
        savings,
        supervisor,
        stats.period.p95_latency_ms,
        stats.index.total_indexed
    )
}
