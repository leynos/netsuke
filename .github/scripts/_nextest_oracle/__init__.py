"""Ask Nextest which tests a filter selects, and what the configuration says.

The pieces are shared because two questions need the same runner: what the
``filter = '…'`` lines of `.config/nextest.toml` are, and which test instances
Nextest reports for a given selector. The grammar half reads the configuration
without running anything; the listing half runs `cargo nextest list` against
the instrumented build tree the coverage lane already populated.

The package exists so that `.github/scripts/verify_nextest_anchored_filters.py`
can stay a top-level script, which is how the workflow calls it, while the
reading it does is split into units that each fit under the module ceiling.

Usage:
    from _nextest_oracle import all_filters, parameterized_tests, selected
"""

from _nextest_oracle.grammar import (
    ANCHORED_SELECTOR as ANCHORED_SELECTOR,
)
from _nextest_oracle.grammar import (
    LEGACY_SELECTOR as LEGACY_SELECTOR,
)
from _nextest_oracle.grammar import (
    all_filters as all_filters,
)
from _nextest_oracle.grammar import (
    configured_names as configured_names,
)
from _nextest_oracle.listing import (
    parameterized_tests as parameterized_tests,
)
from _nextest_oracle.listing import (
    selected as selected,
)
from _nextest_oracle.runner import (
    fail as fail,
)
from _nextest_oracle.runner import (
    instrumented_environment as instrumented_environment,
)
