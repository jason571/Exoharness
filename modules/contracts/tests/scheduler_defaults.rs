use exoharness_contracts::scheduler::SchedulerConfiguration;

#[test]
fn global_default_does_not_change_user_or_resource_pool_limits() {
    let configuration = SchedulerConfiguration::default();
    assert_eq!(configuration.max_global_agent_runs, 1000);
    assert_eq!(configuration.max_active_runs_per_user, 2);
    assert!(configuration.resource_pools.is_empty());
    assert!(configuration.mcp_resource_assignments.is_empty());
    assert!(configuration.validate().is_ok());
}
