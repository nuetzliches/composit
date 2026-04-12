package composit.providers

# Provider approval — controls which external providers agents may use.

import rego.v1

# Approved provider list (maintained by creator)
approved_providers := {
	"nuetzliche",
}

# Only approved providers
deny contains msg if {
	input.action == "use_provider"
	not input.provider in approved_providers
	msg := sprintf("provider '%s' is not in the approved list", [input.provider])
}

# Require EU region for all providers handling personal data
deny contains msg if {
	input.action == "use_provider"
	input.data_classification in {"personal", "confidential", "restricted"}
	not startswith(input.provider_region, "eu-")
	msg := sprintf("provider '%s' in region '%s' cannot handle %s data (EU region required)", [input.provider, input.provider_region, input.data_classification])
}

# Require compliance attestation for regulated data
deny contains msg if {
	input.action == "use_provider"
	input.data_classification in {"confidential", "restricted"}
	not "gdpr" in input.provider_compliance
	msg := sprintf("provider '%s' lacks GDPR compliance for %s data", [input.provider, input.data_classification])
}
