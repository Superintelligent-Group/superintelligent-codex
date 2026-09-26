use pretty_assertions::assert_eq;

use super::*;

#[test]
fn recent_invocations_refresh_recency_and_evict_old_skills() {
    let history = RecentSkillInvocations::default();
    for index in 0..=MAX_SHADOW_RESULTS {
        history.record(format!("skill-{index}"));
    }
    history.record("skill-1".to_string());

    let recent = history.snapshot();

    assert_eq!(MAX_SHADOW_RESULTS, recent.len());
    assert_eq!(Some("skill-1"), recent.first().map(String::as_str));
    assert_eq!(Some("skill-2"), recent.last().map(String::as_str));
    assert!(!recent.iter().any(|skill| skill == "skill-0"));
}

#[test]
fn rank_buckets_distinguish_results_above_twenty() {
    assert_eq!("11_20", rank_bucket(Some(20)));
    assert_eq!("21_50", rank_bucket(Some(21)));
    assert_eq!("21_50", rank_bucket(Some(50)));
    assert_eq!("miss", rank_bucket(Some(51)));
}

fn shadow_request(turn_id: &str) -> ShadowSelectionRequest {
    ShadowSelectionRequest {
        turn_id: turn_id.to_string(),
        user_input: vec![UserInput::Text {
            text: "fix lint".to_string(),
            text_elements: Vec::new(),
        }],
        catalog: SkillCatalog::default(),
        explicitly_selected: Vec::new(),
        host_snapshot: None,
        recent_skill_invocations: Arc::new(RecentSkillInvocations::default()),
        task_context: Arc::new(ShadowTaskContext::default()),
    }
}

#[tokio::test]
async fn spawned_evaluation_waits_for_previous_run_before_starting() {
    let experiment = Arc::new(ShadowSelectionExperiment::new(/*metrics_client*/ None));
    let (release_previous, previous_released) = tokio::sync::oneshot::channel::<()>();
    let previous: PendingShadowSelection = async move {
        let _ = previous_released.await;
        None
    }
    .boxed()
    .shared();

    // Spawning returns immediately; the evaluation is chained behind `previous`.
    let pending = experiment.spawn(Some(previous), shadow_request("turn-2"));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(pending.clone().now_or_never().is_none());

    let _ = release_previous.send(());
    let state = pending.await.expect("shadow evaluation should complete");
    assert_eq!("turn-2", state.turn_id);
    assert_eq!(
        experiment.selectors.len() + 6,
        state.ranked_selections.len()
    );
}

#[test]
fn spawned_evaluation_without_runtime_runs_inline_when_polled() {
    let experiment = Arc::new(ShadowSelectionExperiment::new(/*metrics_client*/ None));
    let pending = experiment.spawn(/*previous*/ None, shadow_request("turn-1"));
    let state = pending
        .now_or_never()
        .flatten()
        .expect("evaluation should run inline when first polled");
    assert_eq!("turn-1", state.turn_id);
}
