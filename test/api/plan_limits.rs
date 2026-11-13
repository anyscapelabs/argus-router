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
        assert!(
            limits.daily_requests_count() > 0,
            "plan `{}` must cap daily requests",
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
    assert_eq!(limits.monthly_tokens_count(), 20_000_000);
    assert_eq!(limits.weekly_tokens_count(), 5_000_000);
    assert_eq!(limits.daily_requests_count(), 50);
}

#[test]
fn paid_plan_request_limits_keep_digit_separators() {
    let plans = api::services::plans::seed_entries();
    for id in ["pro", "business"] {
        let plan = plans
            .iter()
            .find(|plan| plan.id == id)
            .unwrap_or_else(|| panic!("plan `{id}` exists"));
        assert_eq!(
            plan.limits
                .as_ref()
                .expect("paid plan limits")
                .daily_requests_count(),
            3_400
        );
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
