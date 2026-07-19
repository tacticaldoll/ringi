#!/usr/bin/env bash
# Naming-worldview guard (see docs/naming.md).
#
# Fails if a queue-runtime / CQRS word names a ringi domain type, module, or trait — the
# semantic drift that precedes re-monolithing. It matches DECLARATIONS only
# (struct/enum/trait/type/mod), so it is high-precision: it does not flag prose, method
# names, or standard CLI vocabulary such as `Command`. The softer cases in docs/naming.md
# (e.g. runner, stage) stay review-governed.
set -euo pipefail
cd "$(dirname "$0")/.."

banned='Workflow|Job|Queue|Worker|Broker|Dispatcher|Pipeline|Scheduler|Tenant|MessageBus|DeadLetter'

if grep -rnE "\b(struct|enum|trait|type|mod)[[:space:]]+[A-Za-z0-9_]*(${banned})[A-Za-z0-9_]*" --include='*.rs' crates/; then
    echo "" >&2
    echo "naming-guard: a queue-runtime/CQRS word names a ringi type/module — see docs/naming.md." >&2
    echo "the hard mechanics belong to the bricks (lifecycle=pacta, convergence=suunta," >&2
    echo "idempotency=shaahid); naming one here is the monolith returning." >&2
    exit 1
fi

echo "naming-guard: clean"
