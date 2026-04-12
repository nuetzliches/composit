package composit.provision

# Agent provisioning limits — enforced by composit before any provider action.

import rego.v1

# Maximum jobs per business case
default max_jobs_per_case := 5

deny contains msg if {
	input.action == "create_job"
	count(data.state.jobs[input.business_case]) >= max_jobs_per_case
	msg := sprintf("job limit (%d) reached for business case '%s'", [max_jobs_per_case, input.business_case])
}

# Maximum webhook channels per business case
default max_channels_per_case := 10

deny contains msg if {
	input.action == "create_channel"
	count(data.state.channels[input.business_case]) >= max_channels_per_case
	msg := sprintf("channel limit (%d) reached for business case '%s'", [max_channels_per_case, input.business_case])
}

# Monthly cost cap per business case
deny contains msg if {
	input.estimated_cost > 0
	budget := data.budgets[input.business_case]
	current := data.state.current_cost[input.business_case]
	current + input.estimated_cost > budget
	msg := sprintf("budget exceeded for '%s': %d + %d > %d EUR", [input.business_case, current, input.estimated_cost, budget])
}

# Agents cannot provision without a business case
deny contains msg if {
	input.owner_type == "agent"
	not input.business_case
	msg := "agent-created resources must be assigned to a business case"
}
