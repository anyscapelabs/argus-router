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
fn every_catalog_entry_has_a_known_group_and_status() {
    for entry in api::services::catalog::seed_entries() {
        assert!(
            !entry.free,
            "model `{}` is group {} but marked free",
            entry.id,
            entry.group.as_str()
        );
        assert!(
            !matches!(
                entry.status,
                api::services::catalog::ModelStatus::Blocked
            ) || entry.id.starts_with("bedrock/minimax-")
                || entry.id == "qwen/qwen3.8-max",
            "unexpected blocked model `{}`",
            entry.id
        );
    }
}

#[test]
fn plan_groups_expand_to_catalog_models() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_groups(api::services::plans::seed_entries(), &catalog);
    for plan in &plans {
        assert!(
            !plan.models.is_empty(),
            "plan `{}` expands to no models — check its groups ({:?}) against the catalog",
            plan.id,
            plan.groups
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
fn blocked_models_are_not_advertised() {
    let catalog = api::services::catalog::seed_entries();
    let blocked: HashSet<&str> = catalog
        .iter()
        .filter(|entry| entry.is_blocked())
        .map(|entry| entry.id.as_str())
        .collect();
    let plans = api::services::plans::expand_groups(api::services::plans::seed_entries(), &catalog);
    for plan in &plans {
        for model in &plan.models {
            assert!(
                !blocked.contains(model.as_str()),
                "plan `{}` advertises blocked model `{}`",
                plan.id,
                model
            );
        }
    }
}

#[test]
fn free_plan_advertises_only_lite_group_models() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_groups(api::services::plans::seed_entries(), &catalog);
    let group_by_id: std::collections::HashMap<&str, &str> = catalog
        .iter()
        .map(|entry| (entry.id.as_str(), entry.group.as_str()))
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
            group_by_id.get(model.as_str()).copied(),
            Some("lite"),
            "free plan advertises non-lite model `{}`",
            model
        );
    }
}

#[test]
fn auto_alias_is_available_on_every_plan() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_groups(api::services::plans::seed_entries(), &catalog);
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
    let plans = api::services::plans::expand_groups(api::services::plans::seed_entries(), &catalog);
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
fn plan_model_counts_match_expected_groups() {
    let catalog = api::services::catalog::seed_entries();
    let plans = api::services::plans::expand_groups(api::services::plans::seed_entries(), &catalog);
    let expected = [("free", 8), ("starter", 19), ("max", 24), ("max-plus", 29)];
    for (id, count) in expected {
        let plan = plans.iter().find(|plan| plan.id == id).expect("plan exists");
        assert_eq!(
            plan.models.len(),
            count + 1,
            "plan `{}` expected {} models plus auto, got {}",
            id,
            count,
            plan.models.len()
        );
    }
}
