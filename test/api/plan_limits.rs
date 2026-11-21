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
        assert_eq!(
            limits.daily_requests_count(),
            0,
            "plan `{}` must not have a daily request cap",
            plan.id
        );
    }
}

#[test]
fn free_plan_matches_expected_budget() {
    let plans = api::services::plans::seed_entries();
    let free = plans
        .iter()
        .find(|plan| plan.id == api::services::app_db::FREE_PLAN_ID)
        .expect("free plan exists");
    let limits = free.limits.as_ref().expect("free plan limits");
    assert_eq!(limits.monthly_tokens_count(), 8_570_000);
    assert_eq!(limits.weekly_tokens_count(), 3_428_000);
    assert_eq!(free.searches_per_month, 5);
    assert_eq!(free.voice_minutes, 0);
    assert_eq!(free.compute_budget_usd, 0.5);
    assert_eq!(free.allowance_usd, 0.48);
}

#[test]
fn paid_plan_prices_and_allowances_match() {
    let plans = api::services::plans::seed_entries();
    let expected = [
        ("starter", "5", 60, 10, 3.0, 2.53),
        ("max", "10", 150, 25, 8.0, 6.83),
        ("max-plus", "15", 250, 45, 12.0, 9.97),
    ];
    for (id, price, searches, voice, compute, allowance) in expected {
        let plan = plans
            .iter()
            .find(|plan| plan.id == id)
            .unwrap_or_else(|| panic!("plan `{id}` exists"));
        assert_eq!(plan.price, price);
        assert_eq!(plan.searches_per_month, searches);
        assert_eq!(plan.voice_minutes, voice);
        assert_eq!(plan.compute_budget_usd, compute);
        assert_eq!(plan.allowance_usd, allowance);
        assert_eq!(plan.weekly_fraction, 0.4);
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
        daily_requests: String::new(),
    };
    assert_eq!(limits.monthly_tokens_count(), 7);
    assert_eq!(limits.daily_requests_count(), 0);
}
