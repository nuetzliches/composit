package composit.docker

import rego.v1

# Deny any Docker service that pins to :latest — CI should use digests or
# version tags so builds are reproducible and rollbacks are possible.
deny contains msg if {
    some r in input.resources
    r.type == "docker_service"
    endswith(r.image, ":latest")
    msg := sprintf("docker service %v must not use :latest (pin to a version or digest)", [r.name])
}
