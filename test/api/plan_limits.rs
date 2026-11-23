#[test]
fn parses_embedded_plan_limits() {
    let plans = api::services::plans::seed_entries();
    assert!(
        !plans.is_empty(),
        "plans.json must embed at least the free plan"
    );
    for plan in &plans {
        let limits = plan
            .limits
            .as_ref()
            .unwrap_or_else(|| panic!("plan `{}` has no limits block", plan.id));
        assert!(
            limits.monthly_tokens_count() >= 0,
            "plan `{}` monthly token count",
            plan.id
        );
    }
}

#[test]
fn plans_match_embedded_catalog() {
    let raw = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/plans/plans.json"));
    let catalog: serde_json::Value = serde_json::from_str(raw).expect("plans.json is valid JSON");
    let entries = catalog.as_array().expect("plans.json contains an array");
    let plans = api::services::plans::seed_entries();

    for plan in &plans {
        let entry = entries
            .iter()
            .find(|entry| entry.get("id").and_then(|value| value.as_str()) == Some(plan.id.as_str()))
            .unwrap_or_else(|| panic!("plan `{}` exists in plans.json", plan.id));

        assert_eq!(plan.name, entry.get("name").and_then(|v| v.as_str()).expect("name"));
        assert_eq!(plan.price, entry.get("price").and_then(|v| v.as_str()).expect("price"));
        assert_eq!(
            plan.searches_per_month,
            entry.get("searchesPerMonth").and_then(|v| v.as_i64()).expect("searchesPerMonth")
        );
        assert_eq!(
            plan.voice_minutes,
            entry.get("voiceMinutes").and_then(|v| v.as_i64()).expect("voiceMinutes")
        );
        assert_eq!(
            plan.compute_budget_usd,
            entry.get("computeBudgetUsd").and_then(|v| v.as_f64()).expect("computeBudgetUsd")
        );
        assert_eq!(
            plan.allowance_usd,
            entry.get("allowanceUsd").and_then(|v| v.as_f64()).expect("allowanceUsd")
        );
        assert_eq!(
            plan.weekly_fraction,
            entry.get("weeklyFraction").and_then(|v| v.as_f64()).expect("weeklyFraction")
        );

        if let (Some(limits), Some(raw_limits)) = (
            plan.limits.as_ref(),
            entry.get("limits"),
        ) {
            assert_eq!(
                limits.monthly_tokens,
                raw_limits.get("monthlyTokens").and_then(|v| v.as_str()).expect("monthlyTokens")
            );
            assert_eq!(
                limits.weekly_tokens,
                raw_limits.get("weeklyTokens").and_then(|v| v.as_str()).expect("weeklyTokens")
            );
        }
    }
}

#[test]
fn group_caps_keep_lite_largest_and_ultra_smallest() {
    let plans = api::services::plans::seed_entries();
    for plan in &plans {
        let caps = &plan.group_caps;
        let lite = caps.get("lite").copied().unwrap_or(0.0);
        assert!(lite > 0.0, "plan `{}` has no lite cap", plan.id);
        for (group, cap) in caps {
            assert!(
                lite >= *cap,
                "plan `{}` cap for {} is larger than lite",
                plan.id,
                group
            );
        }
        if let (Some(ultra), Some(premium)) = (caps.get("ultra"), caps.get("premium")) {
            assert!(ultra < premium, "ultra cap must be below premium");
        }
    }
}

#[test]
fn unparseable_limits_read_as_unset() {
    let limits = api::services::plans::PlanLimits {
        monthly_tokens: "7".to_owned(),
        weekly_tokens: "bogus".to_owned(),
    };
    assert_eq!(limits.monthly_tokens_count(), 7);
}
