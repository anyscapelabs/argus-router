use std::collections::HashSet;

#[test]
fn catalog_ids_are_unique() {
    let entries = api::services::catalog::seed_entries();
    let mut seen = HashSet::new();
    for entry in &entries {
        assert!(
            seen.insert(entry.id.clone()),
            "catalog contains duplicate id `{}`",
            entry.id
        );
    }
}

#[test]
fn every_catalog_entry_has_a_known_tier() {
    for entry in api::services::catalog::seed_entries() {
        assert!(
            !entry.free,
            "model `{}` is tier {} but marked free",
            entry.id,
            entry.tier.as_str()
        );
    }
}

#[test]
fn plan_tiers_expand_to_catalog_models() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_tiers(api::services::plans::seed_entries(), &catalog);
    for plan in &plans {
        assert!(
            !plan.models.is_empty(),
            "plan `{}` expands to no models — check its tiers ({:?}) against the catalog",
            plan.id,
            plan.tiers
        );
        let catalog_ids: HashSet<&str> = catalog.iter().map(|entry| entry.id.as_str()).collect();
        for model in &plan.models {
            assert!(
                catalog_ids.contains(model.as_str()),
                "plan `{}` advertises `{}`, which is not in the catalog seed",
                plan.id,
                model
            );
        }
    }
}

#[test]
fn free_plan_advertises_only_lite_pool_models() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_tiers(api::services::plans::seed_entries(), &catalog);
    let tier_by_id: std::collections::HashMap<&str, &str> = catalog
        .iter()
        .map(|entry| (entry.id.as_str(), entry.tier.as_str()))
        .collect();
    let free = plans
        .iter()
        .find(|plan| plan.id == api::services::app_db::FREE_PLAN_ID)
        .expect("free plan exists");
    for model in &free.models {
        if model == api::services::catalog::AUTO_MODEL_ID {
            continue;
        }
        assert_eq!(
            tier_by_id.get(model.as_str()).copied(),
            Some("lite"),
            "free plan advertises non-lite model `{}`",
            model
        );
    }
}

#[test]
fn auto_alias_is_available_on_every_plan() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_tiers(api::services::plans::seed_entries(), &catalog);
    for plan in &plans {
        assert!(
            plan.models
                .iter()
                .any(|model| model == api::services::catalog::AUTO_MODEL_ID),
            "plan `{}` does not advertise the auto router",
            plan.id
        );
    }
}

#[test]
fn starter_max_max_plus_model_sets_nest() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_tiers(api::services::plans::seed_entries(), &catalog);
    let models = |id: &str| -> HashSet<String> {
        plans
            .iter()
            .find(|plan| plan.id == id)
            .map(|plan| plan.models.clone().into_iter().collect())
            .unwrap_or_default()
    };
    let starter = models("starter");
    let max = models("max");
    let max_plus = models("max-plus");
    assert!(
        starter.is_subset(&max) && !starter.is_empty(),
        "starter models must be a subset of max"
    );
    assert!(
        max.is_subset(&max_plus),
        "max models must be a subset of max-plus"
    );
}

#[test]
fn plan_model_count_matches_models() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_tiers(api::services::plans::seed_entries(), &catalog);
    for plan in &plans {
        let claimed: Option<usize> = plan
            .model_count
            .split_whitespace()
            .next()
            .and_then(|digits| digits.parse().ok());
        if let Some(claimed) = claimed {
            assert_eq!(
                claimed,
                plan.models.len(),
                "plan `{}` claims `{}` models but lists {}",
                plan.id,
                plan.model_count,
                plan.models.len()
            );
        }
    }
}
