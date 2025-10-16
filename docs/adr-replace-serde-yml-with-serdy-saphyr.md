# ADR: Replace `serde-yml` with `serde-saphyr` for YAML Parsing in Netsuke

## Context and Problem Statement

Netsuke is a YAML-based build system that currently uses the **`serde-yml`**
crate (a fork of `serde_yaml`) to deserialize YAML manifests into Rust structs.
The YAML input is read-only, parsing configuration files into Netsuke data
types, and any YAML output exists solely for debugging (no need to preserve
original formatting, comments, or anchors). The manifest format does not rely
on legacy YAML 1.1 semantics, so strict YAML 1.2 compliance is acceptable or
even preferred.

The `serde-yml` crate has recently been **deprecated/archived** and raised
concerns around maintenance and safety. It was a fork of `serde_yaml` using the
C libyaml parser via unsafe code, and introduced unsound behaviour (segfaults
were demonstrated)([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L176-L184))([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L194-L203)).
This situation prompts the need to choose a more robust and actively maintained
YAML+Serde library. There are no constraints requiring extremely small binaries
or WebAssembly support, so the selection can focus on the best available
solution without platform limitations.

## Decision Outcome (Summary)

This ADR replaces `serde-yml` with **`serde-saphyr`** for YAML parsing in
Netsuke. The `serde-saphyr` crate is a new Serde deserialization framework
built on the pure-Rust **Saphyr** YAML 1.2 parser. This choice is made because
it offers:

- **Safety:** A pure Rust implementation with **no `unsafe` libyaml
  dependencies**([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=For%20the%20,key%20support%20and%20nested%20enums))([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=Those%20who%20dislike%20unsafe%20statements,it%20is%20also%20notably%20faster)),
   eliminating C library risks.

- **Spec Compliance:** Full YAML 1.2 support (including proper handling of
  anchors and merge
  keys)([3](https://github.com/saphyr-rs/saphyr#:~:text=Specification%20Compliance)),
   which aligns with Netsuke requirements and future-proofs the manifest
  format.

- **Robustness:** Built-in handling for resource limits and duplicate keys
  (configurable budgets and policies) to prevent pathological YAML from causing
  crashes or memory
  exhaustion([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=don%E2%80%99t%20apply,to%20prevent%20resource%20exhaustion%20attacks))([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=you%20can%20also%20re,choosing%20between%20LastWins%20and%20FirstWins)).

- **Performance:** A modern zero-copy parsing approach that is significantly
  faster than alternatives in
  benchmarks([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=Parsing%20generated%20YAML%2C%20file%20size,00%20MiB%2C%20release%20build)),
   while using fewer memory allocations.

- **Maintenance & Design:** An actively maintained project (2025) with a clean,
  type-driven API (parsing directly into Rust types without intermediate YAML
  `Value`
  nodes)([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=The%20main%20difference%20from%20other,types%20in%20this%20case%2C%20though)).
   This design suits Netsuke’s strongly-typed manifests and is supported by a
  maintainer focused on correctness and long-term
  support([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=Those%20who%20dislike%20unsafe%20statements,it%20is%20also%20notably%20faster)).

In summary, `serde-saphyr` best meets the selection criteria and will replace
`serde-yml` as the YAML deserialization library in Netsuke.

## Detailed Rationale

### Alternatives Considered and Comparison

Several YAML+Serde libraries were evaluated against the project requirements:

- **Continue with `serde-yml` (Status Quo):** **Not viable.** The `serde_yml`
  crate (a fork of the original `serde_yaml`) has been **archived** by its
  maintainer([6](https://github.com/sebastienrousseau/serde_yml#:~:text=This%20repository%20was%20archived%20by,only))
   and is no longer receiving updates. It inherits the same underlying approach
  as `serde_yaml` (wrapping libyaml via unsafe code) and in fact introduced
  some questionable changes. Notably, David Tolnay (author of Serde) publicly
  critiqued `serde_yml` for being essentially *AI-“maintained”* with “complete
  nonsense” additions and even an unsound YAML emitter that can cause a
  segmentation
  fault([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L176-L184))([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L194-L203)).
   This raises serious trust and safety issues. While `serde_yml` did
  temporarily fill the gap after `serde_yaml`’s deprecation, its quality and
  maintenance are in doubt, so the dependency should be retired.

- **`serde_yaml_ng`:** A fork of `serde_yaml` maintained by the community (by
  @acatton) as a drop-in replacement. It aims for compatibility with the
  original API and has been updating dependencies and fixing bugs. **However**,
  `serde_yaml_ng` still only supports YAML 1.1 at
  present([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L9-L14))
   and relies on the same **libyaml C parser (unsafe)** under the
  hood([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=indexmap%20%3D%20,libyaml%20%3D%20%220.2.11)).
   The maintainer has plans to replace libyaml with a safer alternative in the
  future([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L224-L232)),
   but as of now it hasn’t eliminated the core safety issue. It also does not
  add new features beyond what `serde_yaml` had. Given that Netsuke does not
  need YAML 1.1 quirks and prefers to avoid C library dependencies, this is
  only a
  marginal improvement over `serde_yml`.

- **`serde_norway`:** Another community fork of `serde_yaml` (“norway” being a
  codename) which is similarly intended to maintain YAML support. It is also
  based on the classic approach using libyaml. There’s indication that the
  maintainers are more traditional and careful (i.e. not AI-generated
  code)([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L231-L239)),
   but **feature-wise** it’s comparable to `serde_yaml_ng` (likely YAML 1.1
  only, using libyaml, and no major new capabilities publicly documented). With
  no clear advantages in spec compliance or safety, it does not address
  Netsuke’s core needs beyond providing a maintained fork.

- **`serde_yaml_bw`:** A fork by Bourumir Wyngs (the author behind Saphyr)
  which made **safety and feature improvements** on top of the original code.
  This library introduced support for YAML merge keys and more correct enum
  tagging, and importantly added a **pre-parse “budget” check** using the
  Saphyr parser to defend against resource-exhaustion
  attacks([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=don%E2%80%99t%20apply,to%20prevent%20resource%20exhaustion%20attacks))([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=Budget%2C%20available%20as%20part%20of,DeserializerOptions)).
   It allows configuring limits (document size, nesting depth, etc.) and by
  default **disallows duplicate keys** in mappings (with an option to allow
  them in either First-Wins or Last-Wins mode for legacy
  cases)([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=merge%20keys%20and%20anchors%20,library%20enforces%20configurable%20budget%20constraints))([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=you%20can%20also%20re,choosing%20between%20LastWins%20and%20FirstWins)).
   These are valuable improvements for robustness. However, `serde_yaml_bw`
  still ultimately uses libyaml to do the actual parsing (the Saphyr pass is
  just a
  safeguard)([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=The%20library%20relies%20on%20saphyr,bw))([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=Crate%20Time%20%28ms%29%20Notes%20serde,check%20upfront%20before%20calling%20libyaml)).
   This two-phase approach made it **slower** on large inputs (due to parsing
  twice) – in a 25 MiB YAML benchmark, serde_yaml_bw was the slowest (~703ms)
  because of the extra checking
  step([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=Crate%20Time%20%28ms%29%20Notes%20serde,check%20upfront%20before%20calling%20libyaml)).
   It also retains the older API design with an intermediate YAML `Value` type.
  In short, `serde_yaml_bw` moved the needle on safety, but it did so with
  additional complexity and performance cost, and still wasn’t a clean break
  from libyaml.

- **`serde-saphyr` (Chosen):** A **new implementation** that takes a different
  approach. `serde_saphyr` uses the pure-Rust **Saphyr** YAML 1.2 parser to
  stream YAML **directly into Rust data structures** via Serde, **without first
  building a YAML `Value`
  tree**([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=The%20main%20difference%20from%20other,types%20in%20this%20case%2C%20though)).
   This design has multiple benefits:

- **No Unsafe or C Dependencies:** By relying on Saphyr (written in Rust), it
  avoids linking to libyaml entirely. This removes the `unsafe-libyaml`
  dependency and its potential
  vulnerabilities([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=For%20the%20,key%20support%20and%20nested%20enums))([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=Those%20who%20dislike%20unsafe%20statements,it%20is%20also%20notably%20faster)).
   Extended fuzz testing found that libyaml could even **stall on malicious
  inputs**, whereas Saphyr handles pathological cases more
  robustly([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=The%20library%20relies%20on%20saphyr,bw)).
   This greatly improves confidence in parsing untrusted or complex YAML.

- **YAML 1.2 Full Compliance:** Saphyr is a fresh implementation that passes
  the official YAML 1.2 test
  suite([3](https://github.com/saphyr-rs/saphyr#:~:text=Specification%20Compliance)).
   It supports modern YAML features and stricter spec interpretations that the
  older libyaml-based crates (which largely target YAML 1.1) do not. For
  example, YAML 1.2 changes how certain strings (like `yes`, `no`) are
  interpreted and drops some legacy features — `serde_saphyr` adheres to 1.2
  rules, which is acceptable and even desirable for Netsuke because the
  manifest format does not require outdated 1.1 implicit conversions. It also
  properly supports anchors and merge keys during deserialization (anchors are
  resolved by “replaying” the anchored content when
  needed)([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=without%20first%20building%20an%20intermediate,types%20in%20this%20case%2C%20though)).
   Because Netsuke does not need to preserve anchors or aliases in output, it
  is sufficient that the parser handles them correctly at read time – which it
  does.

- **Built-in Resource Budgeting:** Inspired by the `serde_yaml_bw`
  improvements, `serde_saphyr` provides a **configurable pre-check** to guard
  against resource-exhaustion attacks. Before fully deserializing, it can run a
  fast parsing pass (without building a tree) that simply counts events and
  ensures limits are not
  exceeded([7](https://lib.rs/crates/serde-saphyr#:~:text=serde,any%20resource%20limit%20is%20exceeded))([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=Budget%2C%20available%20as%20part%20of,DeserializerOptions)).
   The default settings are conservative and can be tuned. For instance, if an
  input YAML has extremely deep nesting or huge text values, this can be caught
  early. This is an extra safety net that neither the original serde_yaml nor
  the basic forks provide. In Netsuke’s case, manifests are typically moderate
  in size and come from trusted sources (developers), so hitting the default
  limits is unlikely. But enabling this budget check by default means any
  accidental or maliciously large input will be caught gracefully, preventing
  potential DOS scenarios. If the defaults prove too strict, the limits can be
  adjusted or disabled (the library allows disabling the budget for fully
  trusted
  inputs)([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=The%20default%20budget%20values%20are,to%20disable%20the%20budget%20entirely)).

- **Duplicate Key Policy:** By default, `serde-saphyr` treats duplicate keys in
  the same mapping as an **error**, which aligns with YAML spec recommendations
  and prevents ambiguous data. (In a type-driven parse, a duplicate mapping key
  would either map to the same struct field twice or the same `HashMap` entry
  twice – which is undesirable and likely a user error.) This behaviour matches
  the intended goal of catching duplicate fields in manifests early. If needed
  for backward compatibility, the underlying Saphyr parser or serde_yaml_bw
  allowed toggling to a first-wins/last-wins
  policy([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=you%20can%20also%20re,choosing%20between%20LastWins%20and%20FirstWins)),
   but there is no requirement to allow duplicates in Netsuke manifests.

- **Performance:** Eliminating the intermediate YAML `Value` and parsing in one
  pass makes `serde_saphyr` very efficient. In the maintainer’s benchmarks (25
  MiB file), it outperformed all libyaml-based crates **(~290 ms vs ~470–480 ms
  for others, and 703 ms for the double-parse
  approach)**([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=Crate%20Time%20%28ms%29%20Notes%20serde,check%20upfront%20before%20calling%20libyaml)).
   While Netsuke manifest files are much smaller, this indicates the approach
  has lower overhead. It parses “just in time” into the target structures,
  which is especially beneficial for large inputs or when running many parses
  in a build. This delivers speed without compromising safety.

- **Active Maintenance and Modern API:** `serde_saphyr` is under **active
  development** (it was announced in late 2025 with frequent
  updates)([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=Those%20who%20dislike%20unsafe%20statements,it%20is%20also%20notably%20faster))
   and is built with modern Rust idioms. The API is very similar to
  `serde_yaml`’s (providing `from_str`, `to_string`, etc., via Serde traits),
  so switching over is straightforward. Internally, it avoids the need to
  handle generic YAML `Value` types or represent the entire document in memory.
  This type-driven parsing means any YAML that does not match the Rust struct
  definitions fails
  fast([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=,execution%20exploits%20are%20not%20applicable)),
   which makes error handling simpler and potentially clearer. Given that
  Netsuke’s YAML structure is known (manifest schema), this fits perfectly.

In summary, **`serde-saphyr` addresses all the shortcomings** of the other
options in the context of Netsuke:

It is **safe** (pure Rust), **spec-compliant** (YAML 1.2), and **actively
improved**, with features that specifically target reliability (duplicate key
checks, resource limits). The other libraries each fell short on one or more of
these aspects (either using outdated YAML 1.1, relying on unsafe C code,
lacking maintenance, or adding overhead). `serde-saphyr` provides the best
long-term solution for Netsuke’s YAML deserialization needs.

### Why not wait for `serde_yaml_ng`/others to improve?

It’s worth noting that `serde_yaml_ng`’s maintainer is working on integrating a
safer libyaml or alternative
parser([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L224-L232)),
 and `serde_norway` may similarly evolve. However, these efforts are uncertain
 in timeline and still wouldn’t necessarily achieve YAML 1.2 compliance or the
one-pass design delivered by Saphyr. Meanwhile, `serde_saphyr` is already
delivering these advantages today. Adopting it now positions Netsuke on a
forward-looking path with minimal downsides.

## Implementation Plan

- **Update Dependencies:** Remove `serde-yml` from `Cargo.toml` and add
  `serde-saphyr` (at the latest stable version, e.g. `0.0.x`). No native C
  library linking is required, so this should be straightforward. Ensure that
  `serde` itself remains as a dependency (with the `derive` feature if needed
  for the existing structs).

- **Code Changes:** The public API of `serde_saphyr` is designed to mirror
  `serde_yaml`’s, so most usage can remain the same:

- For example, replace calls like `serde_yml::from_str::<T>(...)` with
  `serde_saphyr::from_str::<T>(...)`. Similarly, use `serde_saphyr::to_string`
  or `to_writer` for any YAML output (e.g., when dumping debug info).

- The error type will change (e.g., `serde_yml::Error` ->
  `serde_saphyr::Error`), so adjust function signatures or error handling
  accordingly. The new error type implements `std::error::Error` and `Display`
  (just like serde_yaml’s did), so logging or displaying errors should work the
  same. Keep in mind that error messages or formatting might differ slightly
  (e.g. line/column reporting).

- If the codebase uses `serde_yml::Value` to manipulate YAML generically,
  refactor that call site. `serde_saphyr` does **not use** a `Value` model
  internally. Netsuke currently parses directly into concrete structs (e.g.,
  manifest definitions), so this scenario is unlikely, but confirm that no
  intermediate YAML value logic is required (such as merging two YAML
  documents or manually inspecting the YAML DOM). If such behaviour exists,
  consider using the `saphyr` crate’s `Yaml` type or adjusting the design. For
  now, the expectation is that no change is needed.

- Anchors and aliases in manifests will be automatically resolved by
  `serde_saphyr` during parsing. Netsuke does not output YAML with anchors, so
  no special handling is required to preserve them. If a manifest uses an
  anchor to duplicate a section, it will come through as duplicated data in the
  Rust structs (which is usually fine).

- Remove any feature flags or workarounds that were specific to `serde_yml`.
  For instance, if there was a cargo feature like "yaml" vs "yaml-ng", simplify
  to just use `serde_saphyr` unconditionally since it covers the full use case.

- **Testing and Validation:** Run the test suite and specifically exercise YAML
  parsing:

- All existing manifest examples should parse identically (with the same
  resulting Rust data). Differences could arise if some input relied on YAML
  1.1 behaviour. For example, strings like "yes" or "no" were considered
  booleans in YAML 1.1; under YAML 1.2 (and `serde_saphyr`), they will be plain
  strings unless explicitly `true`/`false`. Audit the manifest definitions to
  ensure no such patterns are used, or update them to explicit values
  (`true/false` instead of yes/no, quotes around strings that look like numbers
  if needed, etc.). This is a positive change toward clearer YAML, but document
  it in release notes.

- If any tests expected duplicate key handling, adjust them. For instance, if
  previously a duplicate key in a map was silently accepted (or last-wins due
  to `IndexMap` insertion), now it will likely throw an error during parse.
  Decide whether this warrants a specific error message to the user of Netsuke
  (e.g., "duplicate key in manifest") – the `serde_saphyr` error might already
  convey that. In general, this stricter behaviour is acceptable.

- Verify error messages for common failure cases (like schema mismatches) to
  ensure they are understandable. `serde_saphyr`’s errors include location
  information (line/col) for YAML, which is helpful. Consider catching and
  rewording errors in some cases for user-friendliness, though that remains
  optional.

- Benchmark if needed: though performance is expected to improve or at least
  stay on par, parse a representative large manifest file and confirm no
  regressions.

- **Debug Output Changes:** When outputting YAML for debugging, use
  `serde_saphyr`’s serializer. It will produce YAML text without special
  styling (comments will not be present, formatting may be minimal). This is
  fine for Netsuke’s debugging needs. If custom pretty-print logic exists,
  continue using it, or rely on `to_string` and perhaps run it through a
  formatter if needed. Note that `serde_saphyr`’s serializer even supports
  serialising shared references (e.g., `Rc`/`Arc`) as YAML
  anchors([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=Serializer)),
   but that is an advanced feature that Netsuke is unlikely to need. Simple
  serialisation of the data structures to a YAML string is sufficient.

- **Documentation:** Update Netsuke’s documentation (README or user guide) to
  note that Netsuke now supports **YAML 1.2** fully. If there are any changes in
  accepted syntax from YAML 1.1 (e.g., the booleans example), call that out.
  Also, document that duplicate keys in manifests will result in an error (if
  it wasn’t already documented as invalid). Essentially, clarify that manifests
  should adhere to standard YAML 1.2.

- **Optional Tuning:** By default, `serde_saphyr` enables the resource budget
  check with conservative limits. Since the Netsuke use-case (build manifests)
  is not likely to hit those limits, the defaults can remain. If parsing
  extremely large manifests reveals performance overhead, disable the budget
  check in `DeserializerOptions` for a small
  speed-up([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=actually%20building%20a%20full%20data,overhead%2C%20though%20this%20typically%20becomes))([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=The%20default%20budget%20values%20are,to%20disable%20the%20budget%20entirely)).
   However, given the negligible overhead on typical files, it is probably best
  to leave it on as a safe default. Expose a way to configure it if Netsuke
  ever parses user-supplied YAML of arbitrary size (for example, an environment
  with untrusted manifest input might tighten the limits, whereas a fully
  trusted environment might loosen them). For now, stick with defaults and
  monitor.

## Consequences and Risks

**Positive consequences:**

- Eliminates a deprecated and potentially unsafe dependency (`serde_yml` and
  its underlying libyaml) in favour of a safer, pure-Rust solution. This reduces
  the chance of YAML parsing causing crashes or security issues in
  Netsuke([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=The%20library%20relies%20on%20saphyr,bw)).

- Aligns with the YAML 1.2 spec, which means better consistency going forward
  and avoiding subtle bugs related to older YAML behaviour. Netsuke manifests
  will be parsed more strictly and correctly.

- Performance and memory usage should be improved, albeit parsing was not
  previously a major bottleneck. Netsuke can handle larger YAML manifests more
  gracefully.

- Maintenance-wise, the project hitches onto an actively maintained upstream.
  If issues arise in YAML handling, there is an upstream that is responsive and
  improving. This reduces long-term tech debt (no need to maintain an internal
  fork or cling to an archived crate).

**Potential risks or downsides:**

- **Maturity of `serde-saphyr`:** The library is still new (version 0.x). There
  may be undiscovered bugs or edge cases. Mitigation: The author has put it
  through the official YAML test suite and fuzzed it
  extensively([3](https://github.com/saphyr-rs/saphyr#:~:text=Specification%20Compliance))([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=The%20library%20relies%20on%20saphyr,bw)),
   which gives confidence. The team should also run its own tests. Since
  Netsuke’s YAML usage is relatively straightforward (known schema), the risk
  is manageable. Keep an eye on patch releases from `serde-saphyr` for bug
  fixes as it matures.

- **Changes in YAML interpretation:** As noted, some YAML that was previously
  accepted might be rejected or handled differently. For example, unquoted
  `yes` in a manifest would previously become `true` (YAML 1.1) but now remains
  a string "yes" (YAML 1.2). If any user has unknowingly relied on such cases,
  their manifests could break. The existing manifests already use explicit
  booleans and similar constructs, but communicate this change clearly. This is
  more of an education/updating issue than a technical problem. The stricter
  behaviour (and error on duplicate keys) will actually surface potential config
  errors that could otherwise go unnoticed.

- **Dependency churn:** Introducing a new library always carries a small risk
  in dependency tree changes. `serde-saphyr` brings in the `saphyr-parser` and
  a few other Rust crates (like `smallvec`). One notable point: it currently
  uses
  `smallvec 2.0.0-alpha`([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=port%20simpler%20many%20old%20tests,from%20serde_yaml_bw)).
   Using an alpha dependency in the project could be a concern, but in practice
  this is a well-understood crate and the choice was likely made for
  performance. The risk of serious issues here is low, and `smallvec 2.0` is
  expected to stabilise in due time. The project can live with this or pin the
  dependency if needed. Overall compile times and binary size impact from the
  new crates should be minimal (no huge C library to build, pure Rust, and the
  codebase is not extremely large).

- **Learning curve:** Developers on the team need to get familiar with
  `serde_saphyr`, but since it intentionally mirrors Serde’s usual patterns,
  this is trivial. Reading the crate docs and understanding the differences (no
  `Value`, different error struct) is straightforward.

By weighing these factors, the benefits of moving to `serde-saphyr` clearly
outweigh the minor risks. Proceed with the migration and monitor for any issues
in CI and subsequent usage. This decision positions Netsuke to have a robust
YAML foundation moving forward.

## Appendix: References and Further Reading

- **`serde-saphyr` crate:** *YAML Deserializer for Serde built on Saphyr.* –
  Crates.io serde-saphyr, Docs.rs serde_saphyr documentation. (Introduces the
  type-driven YAML parsing approach and usage examples.)

- **Saphyr YAML Parser:** *Pure Rust YAML 1.2 parser library.* – GitHub
  repository: saphyr-rs/saphyr. (Details the YAML 1.2 compliance and design
  philosophy of the parser that powers serde-saphyr.)

- **`serde_yaml_ng` crate:** *“Next-Gen” fork of serde_yaml (YAML 1.1,
  libyaml).* – Crates.io serde_yaml_ng, GitHub: acatton/serde-yaml-ng. (Project
  README contains context on forking, and notes from July 2025 about issues
  with
  serde_yml([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L170-L180))([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L224-L232)).)

- **`serde_norway` crate:** *Another maintained fork of serde_yaml.* –
  Crates.io serde_norway, Docs.rs serde_norway documentation. (Aims to be a
  drop-in replacement for serde_yaml; limited public info on differences.)

- **`serde_yml` crate:** *Archived fork of serde_yaml (now deprecated).* –
  GitHub: sebastienrousseau/serde_yml (archived). (This was Netsuke’s current
  YAML crate, now unmaintained. See discussion of its issues in Tolnay’s
  commentary([1](https://github.com/acatton/serde-yaml-ng/blob/3628102977f3ec9e02b95ef32fcec30b3df91390/README.md#L176-L184)).)

- **`serde_yaml_bw` crate:** *Fork with budget checks and YAML improvements.* –
  Crates.io serde_yaml_bw, GitHub: bourumir-wyngs/serde-yaml-bw. (Contains
  design notes on safety measures like Budget and duplicate key
  handling([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=don%E2%80%99t%20apply,to%20prevent%20resource%20exhaustion%20attacks))([4](https://github.com/bourumir-wyngs/serde-yaml-bw#:~:text=you%20can%20also%20re,choosing%20between%20LastWins%20and%20FirstWins)).)

- **Discussion – Reddit r/rust thread about the loss of serde-yaml** – Thread
  (2025) discussing the deprecation of serde_yaml and community
  alternatives([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=For%20the%20,key%20support%20and%20nested%20enums))([2](https://www.reddit.com/r/rust/comments/1bo5dle/we_lost_serdeyaml_whats_the_next_one/#:~:text=Those%20who%20dislike%20unsafe%20statements,it%20is%20also%20notably%20faster)).

- **Announcement – New Serde YAML (serde-saphyr):** – Rust Internals Forum post
  by the serde-saphyr
  author([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=I%20am%20pleased%20to%20share,has%20been%20much%20discussed%20here))([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=The%20library%20relies%20on%20saphyr,bw)),
   explaining the approach and benchmarking
  results([5](https://users.rust-lang.org/t/new-serde-deserialization-framework-for-yaml-data-that-parses-yaml-into-rust-structures-without-building-syntax-tree/134306#:~:text=Parsing%20generated%20YAML%2C%20file%20size,00%20MiB%2C%20release%20build)).
