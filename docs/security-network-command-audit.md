# Audit of MiniJinja network and command helpers

This note captures a quick security and resilience review of the helper
functions exposed to MiniJinja templates from the `network` and `command`
stdlib modules. Each finding summarises the observed behaviour, the risk it
introduces, and concrete remediation tasks that would harden the helpers.

## Network helper findings

- [x] **Cache directories lack target validation and isolation.** *(Status:
  remediated in PR #219 on 19 October 2025.)* The `fetch` helper lets templates
  pick any `cache_dir`, accepts absolute paths, and opens them using the
  ambient filesystem capability (`open_cache_dir` / `write_cache`). A malicious
  manifest can therefore point the cache at arbitrary system locations such as
  `/etc/netsuke-cache` and plant attacker controlled data. *Remediation tasks:*
  - Require caches to reside under a dedicated workspace-relative directory
    (reject absolute or parent-relative paths) and create them via a sandboxed
    `Dir` root.
  - Consider exposing cache configurability only via trusted configuration,
    not per-template input.
  - **Remediation:** caches are now rooted beneath `.netsuke/fetch` within the
    workspace via `StdlibConfig`, which opens directories through a sandboxed
    `Dir` handle. Template code can no longer pass `cache_dir`, so per-template
    overrides now raise an error instead of redirecting caches outside the
    workspace.
- [x] **Outbound requests lack scheme and host validation.** *(Status:
  remediated on 20 October 2025.)* Because `fetch` accepts any URL, manifests
  can reach link-local metadata services (for example,
  `http://169.254.169.254/`) or other internal resources, yielding SSRF.
  *Remediation tasks:*
  - Provide an allowlist / blocklist mechanism (e.g. only `https://` hosts or
    specific domains) and allow administrators to disable outbound requests
    entirely for untrusted manifests.
  - **Remediation:** outbound requests now default to `https://` and are
    vetted against `--fetch-allow-scheme`, `--fetch-allow-host`, and
    `--fetch-block-host` CLI options. `--fetch-default-deny` switches the
    policy to "block by default" with an explicit allowlist. Policy violations
    surface as `InvalidOperation` errors without opening a network connection.
- [x] **Response bodies are read without a size limit.** `fetch_remote` reads
  the entire HTTP response into memory before returning or caching it. An
  attacker controlling the endpoint can stream unbounded data and exhaust
  memory or disk. *Remediation tasks:*
  - Impose a configurable maximum response size, aborting once the budget is
    exceeded and describing the limit in the error message so template authors
    understand the constraint.
  - Stream large downloads directly to cache files without buffering the whole
    body in memory.
  - **Remediation:** fetch now enforces a configurable byte budget (default
    8 MiB) and aborts requests once the limit is exceeded, quoting the
    configured threshold in the error. Cached downloads stream directly to disk
    and partial files are cleaned up when errors occur; cache reads reuse the
    same guard, so stale oversized entries fail fast.

## Command helper findings

- [ ] **Arbitrary command execution is always enabled.** The `shell` and `grep`
  filters launch external programmes via the system shell, with a five-second
  timeout. There is no option to disable them or to restrict the commands that
  can run. If templates originate from untrusted sources this yields instant
  remote command execution. *Remediation tasks:*
  - Allow integrators to opt out of registering these filters when the
    template is not fully trusted.
  - Provide an allowlist-based command runner (e.g. declarative mapping of
    helper names to binaries) so manifests can reference vetted utilities
    without shell access.
- [ ] **Helpers buffer stdout/stderr without limits.** Both filters capture the
  entire command output into memory before returning it. A command that writes
  an unbounded stream will lead to memory exhaustion or at least prolonged
  blocking. *Remediation tasks:*
  - Enforce maximum output sizes with clear errors when exceeded.
  - Stream results to temporary files when callers opt in to large outputs.

## Next steps

The tasks above can be implemented incrementally. A good first milestone is to
make the risky capabilities opt-in, protecting hosts that evaluate manifests
from semi-trusted sources. Subsequent iterations can tighten resource limits
and add streaming code paths to improve robustness for legitimate large
workloads.
