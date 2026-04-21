package widgetshop.access

import future.keywords.in

default allow = false

allow {
    input.role == "admin"
}

allow {
    input.role == "member"
    input.action == "read"
}

deny[msg] {
    input.user == ""
    msg := "empty user identifier"
}

is_privileged {
    input.role in ["admin", "owner"]
}
