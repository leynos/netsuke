# Netsuke CLI Design Document

## Introduction

This document specifies the design of the Netsuke command-line interface (CLI),
building on the core architecture outlined in the Netsuke mid-level design
document. The focus here is on the user-facing CLI implementation and
behaviour, including user experience, internationalization, accessibility,
real-time feedback, error reporting, configuration, and advanced usage. The CLI
will be implemented using the following Rust crates:

- **Clap** (for argument parsing – as established in the core design)

- **Indicatif** (for spinners, progress bars, and status indicators)

- **Fluent** (for message localization and internationalization)

- **Ortho-Config** (for layered configuration via CLI flags, environment, and
  config files)

Netsuke aims to deliver a **friendly, accessible, and fully localizable** CLI
that adheres to modern best practices and Section 508 accessibility guidelines.
This document is targeted at implementing engineers and serves as a companion
to the core design spec, detailing how the CLI should behave and how these
libraries are applied in Netsuke. Design decisions are explained with examples,
and key recommendations (especially for accessibility) are cited from research
and standards.

## Friendly and Welcoming UX

The Netsuke CLI should offer a **“quick start”** experience that is intuitive
for first-time users. When a user invokes `netsuke` for the first time (for
example, in a new project directory), the tool should provide immediate,
helpful feedback. If a valid `Netsukefile` manifest is present, running
`netsuke` with no arguments will default to building the project’s **default
target(s)** as defined in the manifest. This allows a novice user to simply run
the tool and see results with minimal configuration. If no targets are
specified and the manifest’s `defaults` section is defined, those targets are
built by default – a convenient behaviour called out in the core design. If the
user runs `netsuke` in a directory without a manifest, the CLI should
**gracefully handle the missing file**. For example, it might print:

> **Error:** No `Netsukefile` found in the current directory.

*Hint:* Run `netsuke --help` to see how to specify or create a manifest.

This message is clear about what went wrong (“No Netsukefile found”) and
immediately points the user toward a next step (using `--help`). Such friendly
error handling aligns with Netsuke’s philosophy that error messages should
guide the user constructively.

From the very first run, Netsuke aims to be welcoming. The **help output**
(invoked via `netsuke --help` or `netsuke help`) will be **concise and
useful**, listing available subcommands (e.g. `build`, `clean`, `graph`,
`manifest`) with one-line descriptions of each. Clap auto-generates this usage
text, and the wording remains newcomer-friendly. For example, the help summary
for the default `build` command might say: “`build` – Build specified targets
(or the default targets defined in the Netsukefile).” A **“Hello World”**
example should appear in documentation: for instance, creating a minimal
Netsukefile and running `netsuke` to show how quickly the tool produces a
result. This immediate feedback is crucial for a positive first impression.

Intuitive **defaults** further contribute to a smooth UX. As noted, if no
subcommand is given, `netsuke build` is assumed by default. Similarly, common
options have sensible defaults: by default, Netsuke looks for `Netsukefile` in
the current directory (configurable via a `-f/--file` flag which defaults to
`Netsukefile`). If the user doesn’t specify parallelism, it will use Ninja’s
default or an auto-detected number of jobs. Each default is chosen to “do the
right thing” in the typical case so that novices aren’t forced to supply a lot
of flags.

Finally, the tone of CLI output should be **approachable and human-friendly**.
Where possible, avoid overly terse or technical language in user-facing
messages. Netsuke’s design treats the CLI as a conversation with the user, so
even error messages should avoid sounding like cryptic exceptions. For example,
rather than a raw stack trace, an error might say, “Error: Build failed during
execution. Caused by: Ninja could not build target `my_program` because an
input file is missing. **Hint:** Ensure a target produces `main.o` before
building `my_program`.” This conversational style helps users quickly
understand and fix issues. In summary, the CLI’s UX will emphasize **immediate
success** (quickly getting a build running) and **clear guidance** when
something goes wrong – setting a welcoming tone from the very first interaction.

## Internationalization and Localization with Fluent

Netsuke’s CLI will be fully **localizable** using the Fluent localization
system. All user-facing text (help messages, status output, warnings, errors,
etc.) will be externalized into Fluent `.ftl` resource files rather than
hard-coded strings. This allows translation teams to provide messages in
different languages without modifying code. Message keys shall be defined for
every output, using a clear naming scheme (e.g. `cli-output.build-started`,
`cli-output.build-finished`, `error.manifest.parse_failed`, etc.). The
application will load the appropriate locale’s `.ftl` file at runtime and
format messages via the Fluent crate.

Using Fluent ensures the implementation can handle **pluralization, gender, and
variable interpolation** properly. For example, consider a status message that
indicates how many build targets are being built. In English and many
languages, the wording should change if there is exactly one target versus
multiple. In Fluent, this can be expressed with a plural block:

```ftl
progress-target-count = Building { $count ->
    [one] { $count } target...
   *[other] { $count } targets...
}
```

In this example, the Fluent message `progress-target-count` will produce
“Building 1 target…” or “Building 5 targets…” as appropriate, based on the
value of the variable `$count`. The Fluent system handles the plural logic for
each locale, so languages with more complex plural rules are supported.
Similarly, variables can be interpolated safely. If a message for a
file-not-found error is needed, it might be defined in the `.ftl` file:

```ftl
error-file-not-found = Error: File "{ $path }" was not found.
```

When printing this error, the code would supply the actual file path for the
`{ $path }` variable, and the Fluent library will substitute it. This ensures
that sentence structure can be correctly translated – for instance, some
languages might need the path at the start or need quotes around it
differently. By handing formatting to Fluent, the implementation avoids
constructing sentences via string concatenation in code, which is not
translatable.

All **error messages** and their explanatory hints will be localized as well.
Netsuke’s internal error types (often implemented via `thiserror`) will not
directly user-facing text in the code; instead, they can carry error codes or
keys that map to Fluent messages. For example, an internal enum variant
`ManifestError::ParseTabError` could correspond to a Fluent key
`error.manifest.parse_tab` that has a value like: *“Found a tab character in
the manifest, which is not allowed. Please use spaces for indentation.”* The
CLI would lookup this key and display the message in the user’s locale.
Structured data from the error (like the line number or offending character)
can be passed as Fluent variables to embed in the message. This approach
ensures that even complex error explanations (with causes and hints) are
available in all supported languages.

Fluent resource files should be organized by context (e.g., a section for
general CLI messages, a section for errors). Here’s a small example of what a
**Netsuke Fluent file** (`en-US.ftl`) might contain for CLI messages:

```ftl
cli-help-description = A modern build system for intuitive, fast builds.
cli-help-build = Build specified targets (or defaults if none are specified).
cli-help-clean = Remove build artifacts and temporary files.
cli-help-graph = Visualize the build dependency graph.
cli-help-manifest = Generate the Ninja build file without executing it.

status-parsing = Parsing manifest...
status-generating = Generating build plan...
status-building = Building { $targetCount ->
    [one] { $targetCount } target
   *[other] { $targetCount } targets
}...
status-done = Build completed successfully in { $seconds } seconds.

error-manifest-parse = Failed to parse the Netsukefile.
error-manifest-parse-hint = Ensure the file is valid YAML and all templates are correct.
```

In this snippet, `cli-help-*` keys define help text for various subcommands,
`status-*` keys define messages shown during progress, and `error-*` keys
define an error with a hint. Note how pluralization (`$targetCount`) and
variable insertion (`$seconds`) are handled in a locale-aware way. Translators
for other languages (e.g. `fr.ftl` for French) can then provide equivalent
messages in those languages, adjusting word order or plural forms as needed.

At runtime, Netsuke loads the Fluent messages using the `fluent` crate (and
possibly `fluent-bundle` for message formatting). A **default locale (en-US)**
is maintained within the binary while alternate locales can be loaded from
resource files. Users can specify a locale via an environment variable or
config (e.g., `LANG` or a `--locale` flag), and auto-detection may consult the
environment. All messages include **unicode characters** as needed by
translations (Fluent handles UTF-8 text seamlessly), and message concatenation
outside Fluent is avoided to keep translations correct. Designing the CLI with
Fluent from the start guarantees that Netsuke can provide a **first-class
experience in any language**, which is essential for a welcoming UX globally.

## Accessibility and Section 508 Compliance

Accessibility is a first-class concern for the Netsuke CLI. Although
command-line interfaces are text-based and keyboard-driven (qualities which
make them *inherently* more accessible than GUIs in some respects[^1]), CLIs
still pose unique challenges for users with disabilities. The implementation
adheres to **Section 508 standards** and research-backed recommendations to
ensure that Netsuke is usable by developers with visual or other impairments.
Below this section details how Netsuke’s CLI meets key accessibility criteria:
detail how Netsuke’s CLI will meet key accessibility criteria:

- **Keyboard Operability:** The CLI will be fully operable via keyboard alone
  (by nature, CLIs accept text input and do not require a mouse). There will be
  no hidden functionality that assumes mouse or pointer interaction[^1]. If
  Netsuke ever provides interactive prompts (for example, a yes/no confirmation
  or a menu selection in a future feature), those will be navigable with
  keyboard controls (arrow keys, Enter, etc., or single-key shortcuts) and will
  have clear instructions. However, by default, Netsuke’s operations (build,
  clean, etc.) are non-interactive batch processes initiated by a single
  command, which aligns with keyboard-only usage.

- **No Reliance on Color Alone:** Netsuke supports monochrome terminals and
  users who cannot perceive colour. Any information conveyed with colour is
  **also conveyed in text or symbol form**[^1]. For example, if a successful
  build is indicated with a green message or a ✅ check mark, the text also
  includes a word like “Success.” Error messages might be coloured red for
  emphasis, but they also begin with an explicit **“Error:”** label (and/or an
  `✖` symbol) so that they are identifiable in plain text. Output colours are
  selected with **high contrast** against both dark and light backgrounds
  (ensuring compliance with colour contrast guidelines[^1]). Additionally, the
  CLI respects the standard `NO_COLOR` environment variable to disable coloured
  output entirely, falling back to pure text indicators[^2]. This ensures users
  with monochrome displays or those who prefer no colour (including many screen
  reader setups that ignore colour) get the full meaning of the output.

- **Screen Reader-Friendly Output:** CLI output is structured to be as linear
  and “screen-reader digestible” as possible. A known issue with many CLIs is
  that they produce **unstructured or dynamically updated text** that screen
  readers struggle with. Netsuke avoids complex ASCII art or elaborate
  control-character animations that can confuse screen readers. For example,
  instead of a spinner that rotates with characters like “|/-” on a single
  carriage-returning line (which a screen reader might read incoherently or not
  at all), Netsuke can either: (a) use a simple textual indicator (like
  printing “…working” with incremental dots) or (b) provide an alternative
  **“quiet” mode by default for screen readers**. The CLI may detect if the
  `TERM` environment equals `"dumb"` (often used for basic terminals or some
  screen reader terminals) and automatically simplify the output (e.g., no
  live-updating spinners)[^2][^2]. A user setting (CLI flag or config) enables
  a **“no-spinner” or “accessible” mode**, ensuring that progress is reported
  with static text updates instead of live animation.

- **Status Feedback for Long Operations:** Every Netsuke command that takes
  significant time will provide some form of **status or progress indication**.
  This is crucial because a screen reader user cannot quickly gauge if a CLI is
  working or stuck unless there is output. For instance, during a longer build,
  Netsuke will periodically output progress (see the next section on how
  `indicatif` is used). In an interactive terminal, this might be a progress
  bar or spinner; in a non-interactive or accessible mode, it could be log
  lines like “Parsed 10/50 files…” or a percentage text. The key is to avoid
  silent stalls – users should receive feedback that the build is underway.
  Even a simple textual progress like “Stage 3/6: Generating build plan…” that
  updates each stage is better than no indication. This aligns with
  Recommendation 5 from the research: all commands should provide
  status/progress info when appropriate.

- **Screen Reader-Friendly Progress Indicators:** Relatedly, any progress
  indicators employed by the CLI are designed or configured to be screen reader
  friendly. Screen readers typically cannot interpret graphical progress bars
  or spinner animations as intended. Therefore, Netsuke’s progress output
  always includes a textual component. For example, instead of showing only a
  moving bar like `[=====>       ] 50%`, output such as “50%” or “Halfway done”
  appears alongside it. If using a spinner icon, it is accompanied by a label
  like “Processing” that remains on screen. Audible beeps or messages at
  certain milestones can be offered if that aids users (though this is uncommon
  in CLI tools, it could be an opt-in accessibility feature). At minimum, the
  **final results** (success or failure) are announced in a clear, static line
  that a screen reader can easily pick up (e.g., “✔ Build succeeded” or “✖
  Build failed – see errors above”).

- **Structured Output Options (Alternate Formats):** To improve accessibility,
  Netsuke offers **alternate output formats** for certain information, such as
  JSON or HTML. Large volumes of text in a terminal can be hard to navigate
  with a screen reader (users often resort to copying output to a text editor
  or browser for easier navigation). Support for a `--diag-json` flag for error
  output (as noted in the core design) emits structured error details in JSON,
  which can be parsed or presented by external tools in a more accessible way.
  For example, an IDE integration could catch the JSON and display errors in an
  interface with headings and links. Similarly, for potentially lengthy outputs
  like dependency graphs or build plan data, output can be redirected to a file
  or formatted in HTML/CSV. The research strongly recommends providing ways to
  translate CLI output (especially tables or complex structures) into
  accessible formats like CSV or HTML. In line with this, the `netsuke graph`
  command outputs the dependency graph in DOT format by default, but
  `netsuke graph --html` produces an HTML visualization (useful for sighted
  users). For an accessible alternative, `netsuke graph --json` outputs the
  graph data as JSON. A screen reader user could take that JSON and navigate
  the structure with a JSON viewer, or script custom queries, rather than
  trying to parse a raw DOT text dump (which is essentially a visually
  structured representation). Documentation describes the structure of such
  outputs clearly so users know what to expect (e.g., which fields appear in
  the JSON), per Recommendation 3 about documenting output structure in advance.

- **Accessible Documentation:** Netsuke ensures that all documentation is
  available in an accessible format, not solely within the CLI. Specifically,
  while a man page for Unix (via `clap_mangen`) serves traditional users, man
  pages or lengthy `--help` text in the terminal are not easily used by screen
  reader users. To address this, an **HTML version of all user documentation**
  (usage guide, CLI reference, etc.) is maintained on the project website or
  repo. This follows Recommendation 1: ensure an HTML version of documentation
  is available. The CLI help text may even mention the online docs (e.g., “For
  a formatted version of this help, visit <https://netsuke.dev/docs”>). By
  providing documentation on the web with proper headings and navigation, users
  who rely on screen reader virtual navigation or browser zoom can more easily
  read about Netsuke. Even for sighted users, HTML docs can be preferable for
  search and readability.

- **Avoiding Ephemeral or Non-Verbal Cues:** The CLI avoids any UI mechanism
  that conveys information purely through ephemeral or non-textual cues. For
  example, some CLI tools use a flashing cursor or changing colours to indicate
  progress, or rely on timing (like “press any key in 5 seconds to abort”).
  Netsuke does not use timing-based interactions (all interactions wait
  indefinitely for user input if needed, or proceed with sensible defaults).
  Any visual cues (like a spinner animation or an ASCII art logo) do not carry
  critical meaning that isn’t also given in text. If an ASCII logo or banner is
  included for aesthetics on startup, an option is provided to disable it (for
  a “no-banner” quiet mode, as some tools do) since screen readers would just
  read out a jumble of characters which is not useful.

- **Testing with Assistive Technologies:** Beyond design, the CLI is **tested**
  with screen readers (such as NVDA or VoiceOver) to ensure the reading order
  and verbosity are appropriate. Verification confirms that, for instance, when
  a progress bar is updating, the screen reader isn’t stuck reading the spinner
  animation character repeatedly. Providing a hidden textual progress update
  for screen readers can help; one technique is to periodically print an `\r`
  carriage return with a message like “Progress X%” while hiding it from normal
  view using ANSI codes. However, this can be complex; a simpler approach is to
  avoid fancy animations entirely when an assistive mode is enabled. User
  feedback from visually impaired developers in studies indicates a preference
  for continuous text output that can be scrolled at their own pace. Netsuke’s
  **“quiet” or “accessible” mode** can convert the live progress into a stream
  of log lines (e.g., one line per completed stage or per 10% of progress)
  which a screen reader can read line-by-line. This way, no important
  information is lost and the user isn’t overwhelmed with constant updates
  either.

By implementing these measures, Netsuke’s CLI will strive to be **Section 508
compliant** and user-friendly for people with disabilities. Many of these
practices (e.g., not relying on colour, providing alternate outputs) will
benefit all users – for instance, JSON output is great for automation, and
clear textual messages help even when not using a screen reader. Accessibility
is treated not as an afterthought but as an integral part of the CLI design,
aligning with the philosophy that a truly “modern” CLI must be inclusive.

## Real-Time Feedback and Status Display (Using Indicatif)

A hallmark of a good CLI is how it provides feedback during long-running
operations. Netsuke will use the `indicatif` crate to present **spinners,
progress bars, and status messages** that keep the user informed of what the
tool is doing. The challenge is to do this in a way that is clear and not
overwhelming, while also being mindful of accessibility (as discussed above)
and localization. This section outlines how Netsuke will display real-time
progress information, especially during the build process, and how Netsuke
separates its own progress output from the output of the build commands being
run.

**Stages and Progress Bars:** Netsuke’s build pipeline consists of six stages
(Manifest Ingestion, YAML (YAML Ain’t Markup Language) Parsing, Template
Expansion, Deserialization & Rendering, IR Generation, Ninja Execution). The
CLI reflects these stages in the output so that users see a high-level
progression. Using `indicatif`, a **multi-progress bar** setup can represent
each stage or simply list the stages with a spinner. For example, Netsuke might
print something like:

```text
[1/6] Parsing Netsukefile...      (✔ done)
[2/6] Expanding templates...      (✔ done)
[3/6] Generating build plan...    (✔ done)
[4/6] Validating build graph...   (✔ done)
[5/6] Writing Ninja file...       (✔ done)
[6/6] Running build commands...   (→ in progress)
```

In this sketch, each line corresponds to a stage. As the stage is ongoing, a
spinner or an arrow can indicate activity; when it finishes, a checkmark (✔) or
the word “done” appears. The use of “[n/6]” gives a numeric sense of progress
(useful for screen reader and also cognitive mapping of how far along the
process is). Stage descriptions are localized (e.g., “Parsing Netsukefile”
might become “Analyse du Netsukefile” in French, via Fluent). The stage count
“1/6” can remain numeric and is universally understood.

Implementing this with `indicatif` can be done by creating multiple
`ProgressBar` instances managed by a `MultiProgress`. Each stage can be a
progress bar with length 1 (where “in progress” is 0/1 and “done” is 1/1). Each
message updates as the build advances. However, this approach prints multiple
lines persistently, which might clutter the screen. An alternative approach is
to have a **single progress bar that represents overall progress** (0% to 100%
through all stages), and simply update the message as each stage is reached.
For example, a single progress bar might display “Stage 4/6: IR Generation…
[===_______] 60%”. This reduces vertical space usage. But one drawback is that
the history of stages is not shown; users only see the current stage. Given
builds are usually quick through early stages and slower in execution, it might
suffice to show one line for current stage plus perhaps a second line for
sub-progress (like within the execution stage).

**Build Execution Progress:** The final stage (invoking Ninja to actually run
compile/link commands) can itself have many sub-steps (e.g., hundreds of
compiler invocations). Ninja itself provides a progress count (like “[17/100]”)
when running in verbose mode. Since Netsuke intercepts Ninja’s output, the CLI
can do better: it knows the number of edges in the Ninja build graph from the
IR, so Netsuke can display a **task progress bar** for the execution phase. For
instance, when starting stage 6, if there are 100 tasks, an indicatif progress
bar with length 100 is instantiated. As each build command completes, Netsuke
increments the bar. Notifications of completion can be obtained by reading
Ninja’s output or by Ninja’s return code for each command (though Ninja doesn’t
naturally stream per-edge completion messages except in its own output).
Easiest is to parse Ninja’s own status lines: Ninja typically prints lines like
“[17/100] Compiling foo.c”. Netsuke can catch those through the piped output
and use them to advance the progress bar to 17%. In effect, Netsuke can replace
Ninja’s default progress line with its own unified progress bar.

However, mixing Netsuke-driven progress with Ninja’s output requires care. It
is important to **clearly delineate** Netsuke’s status from the actual command
output (which may include compiler warnings, etc.). One strategy is to dedicate
a portion of the screen/UI to progress indicators and another to command log
output. `indicatif`’s multi-progress can maintain a **persistent progress bar
area** at the bottom of the terminal while allowing new lines of output to
appear above it. Netsuke can spawn a thread to continuously read Ninja’s
stdout/stderr and any time a new line arrives (e.g., a compiler message),
`indicatif` will temporarily move the cursor above the progress bars, print the
line, and then redraw the bars. This way, the progress stays “fixed” in one
place. The end result is similar to how modern package managers or build tools
display output: the bottom of the screen shows an updating status, while the
build logs scroll above.

For example, imagine a scenario of building two C files with a warning in one:

```text
[6/6] Running build commands...   [▸▸▸▸▸---------] 50% (1/2 tasks)

/usr/bin/cc -c src/foo.c -o build/foo.o
src/foo.c:10: warning: implicit declaration of function 'bar' [-Wimplicit-function-declaration]

[6/6] Running build commands...   [▸▸▸▸▸▸▸▸▸▸----] 100% (2/2 tasks)
✔ Build succeeded in 3.4s
```

In this illustration, the progress bar line updates in place. The compiler
command and warning printed in between as normal lines. Notice that the
progress bar includes textual progress “50% (1/2 tasks)” so even if the bar
(represented by “▸” characters) isn’t visible or meaningful to a user, the
numbers convey the progress. Also, the final success message is prefixed with a
checkmark and “Build succeeded”. The implementation ensures that success or
failure is **unambiguously communicated**: success could be green with a check,
failure red with an “Error:” label and cross mark, but either way the text
“succeeded” or “failed” is present.

For localization, all the static parts of these messages (“Running build
commands…”, “tasks”, “Build succeeded in Xs”) are from Fluent, with variables
for numbers or durations.

**Use of Symbols and Formatting:** Unicode symbols like `✔` and `✖` (or
`✅/❌`) act as friendly cues for success and failure, and `▸` or `=`
characters can support progress bars. These make the output more visually
scannable[^2]. Importantly, these symbols **augment** but do not replace text.
A screen reader will likely read `✔` as “check mark”, which along with the word
“succeeded” remains clear. If certain symbols are read poorly (e.g., the
spinner might be read as “vertical bar, slash, dash…” each frame), accessible
mode falls back to simpler characters. The CLI also respects user preferences:
for instance, if the environment variable `NETSUKE_NO_EMOJI` is set (an example
feature that can be supported), plain text “OK”/“FAIL” replaces check/cross.
Similarly, if `NO_COLOR` is set, coloured bars or coloured symbols are
omitted[^2].

Animations are disabled or simplified in **non-interactive contexts**. If
output is being piped to a file or a continuous integration (CI) system
(detected by !isatty on stdout), `indicatif` will automatically disable its
adaptive animations (to avoid garbage characters in logs)[^2]. In such cases,
Netsuke can either print nothing until completion (to keep logs clean) or print
minimal static updates. A likely approach: if not a TTY, the live progress bar
is not shown at all, and instead just prints a single line “Building… (tasks
running)” at start and “Build completed” at end, or perhaps a few milestone
lines. The reasoning is that in CI logs or when redirecting output, a steady
stream of progress updates can spam the log (making it hard to read)[^2]. A
high-level summary is preferred unless the user explicitly requests verbose
output.

To summarize, the CLI’s real-time feedback system will use `indicatif` to give
**clear and continuous insight** into what Netsuke is doing, without
overwhelming the user. Each major phase of the build pipeline is communicated,
and the potentially long-running execution phase is accompanied by a dynamic
progress indicator. By capturing Ninja’s output, the integration remains neatly
aligned with the progress UI. The design ensures that a user always knows
*which stage* the tool is in and *how far along* the build is, which improves
confidence that the tool is working correctly. All of this is done in an
**accessible** manner: textual progress information is always present alongside
any graphical elements, and modes are provided to turn off fancy output when
not appropriate. Netsuke’s use of progress bars and spinners will make the
build experience feel fast, responsive, and modern, much like popular package
managers or build tools that give feedback in real time.

## Differentiating Command Output and Diagnostics

One important design goal is to **clearly delineate Netsuke’s own messages from
the output of the commands it runs** (i.e., the Ninja/compiler output), and to
handle error diagnostics in a structured, user-friendly way. Users should be
able to distinguish whether a line of text came from Netsuke or from a
subprocess. Additionally, error messages must be formatted distinctly and
helpfully. This section describes how the CLI separates these concerns in the
output and how `anyhow`, `thiserror`, and `miette` are employed for robust
error reporting.

**Separation of Streams:** Netsuke will use separate I/O streams for different
types of output. By convention, **Netsuke’s own status messages and
warnings/errors will be sent to stderr**, while the normal output of build
commands will flow to stdout. This separation is common in CLI design (stdout
for primary output, stderr for ancillary messages[^2]) and allows advanced
users to redirect or pipe outputs as needed. For example, if a user only wants
to capture the actual build output (e.g., compiler warnings) to a log, they can
redirect stdout to a file, and Netsuke’s progress and status (on stderr) will
still display to the console. Conversely, in a silent or machine mode, a user
might ignore stdout and only heed structured JSON on stderr, etc. Under the
hood, as mentioned earlier, Netsuke captures Ninja’s stdout and stderr via
piped streams and re-emits them appropriately. The implementation ensures that
Ninja’s output order is **preserved** exactly – i.e., Netsuke does not buffer
things in a way that jumbles the sequence of messages.

When printing messages, the CLI includes **explicit markers** or formatting to
distinguish origin. Netsuke’s messages can be prefixed or styled. For instance,
Netsuke might prefix its messages with its name or a short tag like `[Netsuke]`
in verbose mode. In normal mode, styling can be used: e.g., Netsuke status
messages could be in bold or a different colour (cyan, for example), while
command outputs remain in default text. Either way, the design guarantees that
in the absence of colour, there is still a differentiation. One possibility is
indentation: subprocess output can be printed indented by a couple of spaces,
so it visually appears as a sub-block under a Netsuke heading. For example:

```text
Netsuke: Compiling target "app"
  gcc -c app.c -o app.o
  app.c: In function 'main': warning: ...
Netsuke: Compiling target "lib"
  gcc -c lib.c -o lib.o
```

Here the indent of the command and its output helps the eye, and the prefix
“Netsuke:” only appears on Netsuke’s own lines. This approach is evaluated – it
can make parsing by tools slightly harder (since lines have leading spaces),
but for human readers it provides a clear separation. Alternatively, a simple
`[>]` or similar marker on command lines could be used (like
`> gcc -c app.c ...`).

Crucially, **no important information is conveyed solely by a subtle difference
in formatting**. When indentation is used as above, documentation also states
that “non-indented lines are Netsuke messages; indented lines are command
output” for clarity. When prefixes are used, they remain textual (“Netsuke:” or
perhaps an emoji like 🠶 for commands) – something a screen reader will
announce. In summary, the user should never be confused about what output came
from their compiler/test commands versus what Netsuke reports on its progress.

**Error Reporting:** Netsuke’s error handling strategy is a hybrid one,
leveraging `anyhow` for capturing error contexts and `thiserror`+`miette` for
producing rich diagnostics. All **user-facing errors** appear in a structured,
multi-line format designed to answer the questions “What went wrong? Where?
Why? and How to fix?”. Error messages are treated as an extension of the UI/UX
and should be as helpful as documentation.

For general runtime errors or unexpected failures, Netsuke will output a
concise **error summary** followed by the cause chain. For example, if an OS
error occurs (like failing to read a file due to permissions), Netsuke might
print:

```text
Error: Failed to execute build

Caused by:
    0: Could not open Ninja build file for writing
    1: Permission denied (os error 13)
```

This formatting comes from printing the `anyhow::Error` chain, which `anyhow`
nicely formats with “Caused by” entries. However, for anticipated errors
(especially those related to user input, like errors in the Netsukefile or in
templates), the CLI provides **even richer context** using `miette::Diagnostic`.

For instance, if there is a YAML syntax error in the Netsukefile, the error
presented will include a snippet of the manifest with an arrow pointing to the
offending part. Netsuke’s core design has a `YamlDiagnostic` struct exactly for
this purpose. When the YAML parser (`serde_yaml` or similar) returns an error
(say “found character that cannot start any token at line 10, column 5”),
Netsuke will catch that and wrap it in the `YamlDiagnostic`, which includes the
source text and a `SourceSpan` for the error location. The CLI will then invoke
`miette` to render this nicely. The output could look like:

```text
✖ Error: Failed to parse Netsukefile

   ┌─ Netsukefile:10:5
   │
10 │     foo: "bar
   │         ^ missing closing quote
   │
   = Hint: Every string value must be enclosed in quotes. Did you forget a " at the end?
```

In this mock-up, the example shows a file name and line/col (provided by
miette’s snippet mechanism), the line from the file with a caret under the
problematic spot (the `^` is placed under column 5), a label explaining the
issue (“missing closing quote”), and a **remediation hint** after a hint marker
(here `=` is used by miette to denote an advisory note) suggesting how to fix
it. The exact formatting will be handled by `miette` – which is known for
producing compiler-like error messages with colour highlighting of the error
span and bold labels. These messages remain **clear when read aloud** by a
screen reader as well (for example, the above would be read line by line, so it
should make sense even without colour – which it does, thanks to the arrows and
text).

Netsuke will implement similar diagnostics for **Jinja template errors**. If a
Jinja expression fails (say the user used an undefined variable in a `{{ }}`
expression), the implementation captures the template engine’s error (which
hopefully includes location in the template or the manifest) and present a
message like:

```text
✖ Error: Template rendering failed at target "my_exe"
   ┌─ Netsukefile:20:15
   │
20 │     output: "{{ undefined_var }}/out.bin"
   │               ^^^^^^^^^^^^^^ unknown variable
   =
   = Hint: The variable `undefined_var` is not defined. Make sure it is passed in or spelled correctly.
```

Each structured error in Netsuke is backed by a `thiserror` type implementing
`Diagnostic`. For example, `IrGenError::RuleNotFound` might produce an error
like “Target 'X' uses a rule 'Y' which was not defined in the rules section”.
When such an error bubbles up, it will likely be wrapped in a higher-level
`ManifestError::Parse` or `BuildError` with context, but the message shown to
the user remains the human-friendly one defined in the error’s
`#[error("...")]` string, along with any labels or help text defined in the
`Diagnostic` implementation. The core design explicitly requires that all
surfaced errors implement `miette::Diagnostic` so that spans and suggestions
can be presented – this requirement is followed strictly. It means even if an
error is caught via `anyhow`, if it started as a `Diagnostic` (like the YAML or
IR errors), that diagnostic metadata is preserved through the error chain
(using `anyhow::Error`’s ability to carry sources that implement
`Error + Diagnostic`). Care is taken never to lose the diagnostic info when
propagating (the design notes never to convert a `miette::Diagnostic` into a
plain error without preserving the diagnostic, otherwise spans and help text
are discarded).

**Consistent Formatting:** All error messages from Netsuke itself will start
with a common prefix (e.g., `Error:` or `✖ Error:`) and are usually printed to
stderr. They will often be multi-line as shown, with indentation for the file
snippet, etc. Long backtraces or internal details are avoided by default –
those are not helpful to most users[^2][^2]. Instead, for unexpected panics or
developer-level debugging, a **verbose error mode** can be enabled. For
instance, if an environment variable `NETSUKE_DEBUG=1` or a `-vv` flag is set,
it might include a backtrace or internal log path. But normally, the error
shown is meant to be understood by the user and give them next steps (this
echoes best practices from CLI guidelines: “catch errors and rewrite them for
humans”[^2], and provide actionable messages rather than cryptic ones).

**Distinguishing Build Failures vs Tool Failures:** If a build command (like
the compiler or a user script) fails, Ninja will exit with a nonzero status.
Netsuke will catch this and report it as a build failure. In such cases, it is
important to highlight that *the build failed*, not Netsuke itself. For
example, if one of the compile commands returns an error, Ninja’s output will
contain the compiler error message. Netsuke will print that to stdout as
captured, and then when Ninja exits, Netsuke will print an **error summary** on
stderr, such as:

```text
✖ Build failed.
One of the build commands exited with an error. See above for the specific error output.

(To run the failing command again with more info, use ... )
```

This separates the tool’s role (Netsuke is telling you the build didn’t
succeed) from the command’s role (the compiler provided details of the failure
above). The core design shows an example of Netsuke wrapping a Ninja failure
with a user-friendly explanation and hint. That pattern is followed. **Error
codes or categories** may also appear in the error messages (for example,
`[NET-001] Build configuration is invalid`). In the `Diagnostic` definitions
there are codes like `netsuke::yaml::parse`. These can be displayed to the user
as well (perhaps in verbose mode or in machine-readable output) to help with
documentation and support.

**JSON and Machine Readable Diagnostics:** As mentioned earlier, a flag like
`--diag-json` outputs errors in JSON form. When this flag is used, instead of
printing the fancy human-readable error, Netsuke outputs a JSON object
describing the error(s), including file paths, line/col, error code, message,
and even the snippet of source if applicable. This is extremely useful for
editor integrations or other tools (and aligns with recommendation to provide
alternate output formats). For instance, an IDE plugin could run
`netsuke --diag-json build` and get a JSON array of any errors which it can
then display with proper UI elements (the user wouldn’t even see the CLI text
in that case). The JSON schema is documented (so it’s an official interface),
and it includes all the information that is in the text diagnostic (error code,
message, labels, suggestions).

**No Hidden Meaning in Symbols/Colors:** Caution is exercised so that any
symbols used (checkmarks, crosses, arrows, etc.) are simply decorative or
reinforcing a message that is also in text. For example, just printing “⚠️” to
indicate a warning is not enough – it must say “Warning:” or similar. Netsuke
might emit warnings for deprecated syntax or minor issues; those will be
formatted with a label like “Warning:” (possibly in yellow text) so they are
easy to pick out. Multiple warnings of the same type may be grouped or
summarized to avoid overwhelming the user (following the guideline of
maintaining a good signal-to-noise ratio[^2]).

In summary, Netsuke’s CLI output is carefully organized into two channels:
**informational progress output vs. command output**, and within errors:
**user-friendly diagnostics vs. raw tool errors**. By leveraging structured
error handling with `miette` and clearly prefixing or separating outputs, the
design ensures the user can always tell what the build system is saying versus
what the compiler or other tools are saying. This clarity will make
troubleshooting easier. Combined with localization, these errors and outputs
will be understandable in the user’s own language, and combined with
accessibility features, they will be as clear when heard or parsed as they are
when read visually.

## Configuration and Customization (Ortho-Config Integration)

Netsuke’s CLI is designed to be not only easy out-of-the-box, but also
configurable to fit into different workflows. This is achieved by using the
**OrthoConfig** crate to manage configuration from multiple sources –
command-line flags, environment variables, and configuration files – in a
unified way[^3]. This means users can customize the CLI’s behaviour (like
output style, verbosity, default targets, etc.) persistently, rather than
having to type numerous flags each time.

**Layered Configuration Sources:** OrthoConfig allows the engineering team to
define a configuration struct in Rust and have it automatically populated from
(1) program defaults, (2) a config file (e.g., a TOML (Tom’s Obvious, Minimal
Language) or YAML file), (3) environment variables, and (4) CLI arguments, with
that precedence order[^3]. In Netsuke, the struct (say `CliConfig`) includes
fields such as:

- `verbose: bool` – for verbose output mode

- `quiet: bool` – for noise-free mode

- `color: Option<String>` – colour mode (“always”, “never”, or “auto”)

- `output_format: Option<String>` – format for output/diagnostics (“text” or
  “json”)

- `default_target: Option<String>` – default target(s) to build if none
  specified

- `spinner: bool` – whether to show spinner/progress (default true for
  interactive)

- `theme: Option<String>` – theme for output (e.g., “unicode” vs “ascii”)

This is an illustrative set – actual fields will be determined by what is
needed. Using OrthoConfig’s derive macros, each field can automatically map to
a CLI flag, an env var, and a config file entry. For example, `verbose: bool`
can map to a `--verbose` flag (already in Clap), an env var like
`NETSUKE_VERBOSE=true`, and a config file entry `verbose = true`. OrthoConfig’s
**orthographic naming** feature will handle the naming conventions (so
`--no-color` flag might correspond to env `NETSUKE_NO_COLOR` and config file
key `color = "never"` etc.) without a lot of manual wiring[^3][^3]. A prefix
like `NETSUKE_` is used for environment variables to avoid conflicts (the
OrthoConfig derive allows specifying a prefix for env vars and file
sections[^3][^3]).

**Configuration File:** By default, Netsuke will look for a config file in
standard locations. Thanks to OrthoConfig’s discovery mechanism, the
implementation can support a config file name like `.netsuke.toml` or
`$XDG_CONFIG_HOME/netsuke/config.toml`. The OrthoConfig docs indicate it
searches for a file in the current directory or home directory with a
prefix-based name[^3]. The prefix is typically set to "netsuke", meaning it
looks for `.netsuke.toml` or `.netsuke.yaml` (if YAML support is enabled) in
the current directory or `~/.netsuke.toml` in the user’s home. This lets users
define project-specific config (in the project directory) or global config (in
their home) that affects Netsuke’s behaviour. The config file is optional – if
not present, default settings apply.

**Environment Variables:** Each config option will also map to an environment
variable. For instance, to force colour off globally, a user could set
`NETSUKE_NO_COLOR=1` in their shell profile. Or to always get JSON diagnostic
output in a CI environment, one could set `NETSUKE_OUTPUT_FORMAT=json`.
Environment vars are convenient for CI and also for users who prefer them over
config files. They override the config file but are themselves overridden by
explicit CLI flags[^3].

**Command-Line Flags:** OrthoConfig integrates with Clap such that flags parsed
by Clap feed into the config struct. Since Clap is already being used for
primary CLI parsing (subcommands etc.), Clap’s results can directly construct
the config. Alternatively, OrthoConfig could parse `std::env::args` itself. A
typical approach parses arguments with Clap into an `Args` struct, then calls
`AppConfig::load()` (from OrthoConfig) which merges env and file, and then
overriding with any flags from Clap. The exact integration can be decided
during implementation, but the outcome is that after startup there is one
unified `config` object with all settings resolved in the right precedence.

**Configurable Options:** Users may want to customize the following via config:

- **Output Verbosity:** Instead of always requiring `-v` for verbose or `-q`
  for quiet, a user can set a default. For example, `verbose = true` in config
  makes Netsuke always verbose unless `--quiet` is passed. Or a user who
  dislikes any extra output can set `quiet = true` globally.

- **Color Theme:** Users can decide if they always want colour or never. For
  instance, in some continuous integration setups, the terminal might actually
  support colour but they still prefer plain text logs – setting
  `color = "never"` in config handles that. Conversely, if someone’s workflow
  pipes output to a pager that supports ANSI, they might set `color = "always"`
  to force colour even when stdout is not a TTY. (Clap/Ortho can interpret an
  enum for such options; support will cover the standard values “auto”
  (default), “always”, “never”.)

- **Spinner/Progress Display:** Some users might find spinners distracting or
  might want more compact output. Configuration allows `spinner = false` (or
  `progress = "none"`) in config to disable live progress indicators globally,
  equivalent to always using a quiet mode. This could be useful for screen
  reader users who want to opt-out entirely.

- **Default Targets or Profiles:** If a user frequently wants to build a
  specific target or use certain options, config can help. For example,
  `default_target = "all"` could override the manifest’s default section by
  always building the “all” target unless the user specifies otherwise. The
  configuration can also allow a default build *profile* (if Netsuke ever
  supports things like debug vs release modes in the future, that could go
  here).

- **Output Format:** A user could set `output_format = "json"` in their config
  for a CI environment, so that every Netsuke invocation automatically gives
  JSON errors (unless they override with `--diag-json=false` on a specific
  run). This is safer than expecting every developer to remember to pass
  `--diag-json` in CI scripts.

- **Themes and Symbols:** Some users might prefer ASCII-only output (no Unicode
  symbols) due to font or locale issues. Support can include `theme = "ascii"`
  which would make Netsuke use `"[OK]"` and `"[FAIL]"` instead of ticks/crosses
  and use `.` or `#` for progress bar blocks instead of fancy Unicode blocks.
  This again can be auto-set by detection (e.g., if `LANG=C` or on Windows
  console), but giving the user control via config is ideal.

- **Logging and Debugging:** Possibly, config can enable persistent debug
  logging to a file (`debug_log = "/path/to/log.txt"`), though that might be
  beyond OrthoConfig’s scope if it is not exposed as a CLI flag. However,
  anything that Clap could also accept can be incorporated.

OrthoConfig makes adding these options straightforward and consistent. Adding a
field to the struct ensures the derive generates CLI flags and env var names in
a consistent way (kebab-case for CLI, upper-snake for env, etc.)[^3][^3].

### Example: Setting Configurations

Imagine a user who is visually impaired and wants Netsuke in a very minimal,
screen-reader-friendly mode by default. They could create a `~/.netsuke.toml`
like:

```toml
# User-level Netsuke config file (~/.netsuke.toml)
verbose = false
quiet = true
color = "never"
output_format = "text"
spinner = false
```

This ensures that by default, Netsuke will not use colour, will suppress
non-essential output, and will not show spinners or progress bars (quiet mode).
When they do need to see more details, they can still run `netsuke -v` to
temporarily override quiet mode. OrthoConfig will merge that flag appropriately.

Another example: a developer who loves fancy output might put in their config:

```toml
color = "always"
theme = "unicode"   # use all the fancy Unicode bars and symbols
```

So even if they redirect output, they’d still get colour (say, piping through
`less -R` to view coloured output).

Documentation will cover all these configuration knobs in the Netsuke manual.
Using OrthoConfig gives a lot of flexibility “for free” – it allows the CLI to
cater to both simple use (no config needed) and advanced tuning. It also
future-proofs the CLI; if new options are added, they can naturally fit into
this layered scheme.

Finally, note that Clap and OrthoConfig together ensure **consistent naming and
discovery**. For instance, if the binary name is `netsuke`, OrthoConfig by
default might look for `.netsuke.toml`. If a user wants to use a custom config
file path, a `--config <path>` flag (which Ortho can generate via an
attribute)[^3] can be provided. The CLI likely allows
`Netsuke --config myconfig.toml` to load a specific config. This helps in CI or
multiple project scenarios where one might store config with the project.

In conclusion, by integrating OrthoConfig, Netsuke’s CLI becomes highly
customizable without adding burden on the user to always specify options. Users
get **intuitive, orthogonal config names** across CLI flags, env vars, and
files, enabling them to adapt Netsuke to their environment – whether that’s
making it quieter, more verbose, more colourful, or altering defaults. This
layering also plays well with the concept of different **profiles** (if build
profiles or contexts are introduced later, config could switch between them).
The CLI will read configurations in a predictable order and apply them,
ensuring a smooth experience that can be as simple or as tailored as the user
desires.

## User Journey and Progression Path

The Netsuke CLI is designed to support a user’s journey from a beginner just
running their first build, to a power user leveraging advanced features for
debugging and CI integration. In this section, a typical progression path and
how the CLI accommodates each stage of expertise, illustrating how features
like introspection commands and diagnostic modes come into play.

- **First Invocation – “Hello World” Build:** A new user installs Netsuke and
  runs `netsuke` on a simple project. Thanks to the friendly defaults, this
  likely just works (assuming a `Netsukefile` is present). The CLI prints a
  brief welcome or version info (if the design chooses to show one) and
  immediately goes into building with intuitive output. The user sees a
  spinner/progress and a success message. If something is misconfigured (like
  no manifest), the error message gently guides them, as discussed earlier.
  This positive first experience (immediate feedback, either a built artifact
  or a clear next step) builds the user’s trust. They can also run
  `netsuke --help` to see available commands, which is formatted for quick
  scanning (each subcommand and flag is explained in a sentence). At this
  stage, the user learns that `netsuke build` is the main command (and is
  default), and they might not use other commands yet.

- **Using Basic Options:** The user next discovers they can adjust verbosity.
  If they want to see more details, they try `netsuke build -v`. The CLI in
  verbose mode might print additional information (like the exact ninja command
  being invoked, or more detailed logging of what Netsuke is doing internally).
  For example, in verbose mode Netsuke could echo the location of the manifest
  it loaded, how many targets it found, etc., which is normally suppressed.
  This helps the user understand the tool’s process. Conversely, they try
  `netsuke build -q` (if a quiet flag is provided) to see a minimal output –
  perhaps just errors or a final success line. Over time, they figure out their
  preferred level of verbosity and can set it via config for convenience.

- **Exploring Other Subcommands:** As the user becomes more comfortable, they
  may try the other subcommands Netsuke offers:

- `netsuke clean`: They run this to clear build artifacts. The CLI prints a
  short confirmation message (“Build directory cleaned”) or any relevant info
  from Ninja’s clean command. It’s straightforward.

- `netsuke graph`: The user is curious about the build graph. Running
  `netsuke graph` outputs the dependency graph in DOT format to stdout (or
  perhaps to a file). The CLI might note “Graph generated in buildgraph.dot
  (open with `dot` or view with `netsuke graph --html`)”. If they run
  `netsuke graph --html`, Netsuke could start a local HTML viewer or produce an
  HTML file and open it in a browser, showing a nice visualization. This mode
  demonstrates Netsuke’s more advanced introspection capabilities. This ensures
  that even in this case, if the user is in a text-only environment, there’s a
  fallback (like just instructions or a text summary). The help text for
  `graph` clearly states what it does and mentions the `--html` option, guiding
  users to advanced usage.

- `netsuke manifest`: The user can output the Ninja file via
  `netsuke manifest output.ninja`. The CLI confirms by writing the file and
  printing a message like “Ninja file generated at output.ninja (not
  executed)”. If verbose, it might also print some stats (how many rules, how
  many targets). This command is for advanced debugging – the user sees exactly
  what build commands Netsuke generated. It’s an advanced tool for trust but
  also helps if they need to manually inspect or run ninja themselves. By
  offering it as a first-class command, the CLI demonstrates that Netsuke
  caters to power users.

- **Encountering and Understanding Errors:** At some point, the user will write
  something wrong in a Netsukefile or encounter a build failure. This is a
  crucial moment in the journey. The CLI responds with a detailed error message
  (as described in the error section) that doesn’t just dump a stack trace, but
  rather points out exactly what in their input is wrong and how to fix it.
  Suppose they put a duplicate key in YAML; the error highlights it. The user
  is able to correct the mistake quickly. This positive error experience turns
  a potentially frustrating moment into a learning opportunity. The user also
  notices that error messages are documented or have codes – the documentation
  may note “for error codes, run `netsuke --help-errors` or consult the
  manual.” An `netsuke explain <code>` command (similar to Rust’s
  `rustc --explain E0425`) could provide a longer explanation of certain
  complex errors. That would be a further advanced feature to consider for user
  education.

- **Customization and Environment Integration:** As the user integrates Netsuke
  into bigger projects and teams, they utilize the config ability. For example,
  in a CI pipeline, they add `NETSUKE_OUTPUT_FORMAT=json` so that the CI can
  parse errors. They might use `netsuke --diag-json` locally when working with
  an editor that reads JSON diagnostics to highlight errors in code (some
  editors could invoke netsuke on save to get live error feedback). They also
  might set up a global config to turn off colour if the CI logs were getting
  escape codes (they recall from documentation that `NO_COLOR` is respected or
  use `NETSUKE_NO_COLOR`). In a team, one developer might prefer very quiet
  output while another wants to see the full commands; each can use their
  config to set that without affecting others, since it can be in their home
  directory.

- **Advanced Debugging and Introspection:** A power user will find ways to
  debug complex build scenarios. For instance, if a build is not producing
  expected results, they might run `netsuke build -v` or even with a
  hypothetical `-vv` for debug logs, to see all the context Netsuke is
  processing. They might enable a debug log file (if that is provided) to send
  very verbose output (including internal decisions, dependency resolution
  info, etc.) to a file for analysis. Netsuke could incorporate an option
  `--dump-ir` to output the internal Intermediate Representation of the build
  plan, or `netsuke graph` might be used heavily to visualize dependencies. All
  these are part of advanced usage that the CLI makes accessible with clear
  commands and options.

- **Scripting and Automation:** Over time, users may script Netsuke. For
  example, they might have a script that calls `netsuke build` and then does
  something with the result. The script may rely on Netsuke’s exit codes (0 on
  success, nonzero on failure) and perhaps parse its output. Because the output
  is designed to be structured (error messages start with "Error:", etc.), a
  script can easily grep for “Error:” or use the JSON output for reliable
  parsing. In automation contexts, the user will appreciate that Netsuke
  doesn’t produce unpredictable output – e.g., if quiet mode is used, it won’t
  spam progress, just the essentials, making log parsing easier. Also, the
  presence of a machine format for errors ensures integration with tools is
  robust. It is reasonable to imagine a future where Netsuke can output a
  summary of build results in JSON (like which targets were built, which were
  up-to-date, etc.). While not initially in scope, the CLI design leaves room
  for such features (maybe a `--report-json` flag down the line).

- **Updating and Extensibility:** As the tool evolves, the CLI might add new
  subcommands (for example, an `init` command to scaffold a Netsukefile, or a
  `test` command to run tests, etc.). The design philosophy remains to keep
  things **consistent**. For instance, if an `init` command is added, it will
  follow the same style: clear description, maybe an interactive prompt (with
  accessible defaults), etc. Users who have grown accustomed to Netsuke will
  find new features familiar in usage. Our documentation (in HTML and built-in
  help) will cover new commands thoroughly so advanced users can discover them
  (`netsuke --help` always showing all commands).

Throughout this journey, **Section 508 compliance and localization** remain in
effect. A user using a Spanish locale will see all the messages in Spanish from
day one. A blind user will have used the `--quiet` mode or other accessible
config early on and will still be able to do everything the power user can
(view graphs in a textual form, get JSON outputs, etc.). The CLI’s flexibility
ensures that advanced usage doesn’t compromise accessibility – for example,
even the `graph --html` feature would have an alternative way to consume the
data.

In summary, the Netsuke CLI supports a gradual learning curve: **easy to
start**, with sensible defaults and guidance; **informative as needed**, with
verbosity control; and **empowering at advanced levels** with introspection and
integration features. This approach aligns with modern CLI design philosophy of
being helpful and “human-first”[^2][^2], ensuring that as users invest more in
Netsuke, Netsuke continues to support them with a rich, configurable, and
scriptable interface.

## Visual Design and Layout Guidance

The visual presentation of Netsuke’s CLI output is carefully considered to
enhance readability and clarity. This section provides guidelines for layout,
styling, and other visual elements of the CLI, tying together many points
discussed earlier into a coherent style guide for implementers.

**Overall Layout:** Netsuke’s CLI output should avoid looking like an
unstructured wall of text. Use **whitespace and line breaks strategically** to
separate concerns. For example, when a build starts, printing a blank line
after the command invocation can help visually separate it from any shell
prompt or previous output. Group related messages together and put an empty
line before significant sections (like before printing an error summary, or
before a block of warnings). However, do not overdo blank lines such that
output becomes sparse – it’s about creating logical chunks. For instance, one
might output the progress bars continuously during a build, then on completion
print a blank line and the final success message to clearly delineate the end
of progress display from the next shell prompt.

**Progress Indicator Style:** Use **consistent symbols** for progress.
Indicatif offers many progress bar styles; the implementation should choose one
that is **simple and high contrast**. A solid block `█` or `#` could represent
completed work and a `-` or space could represent pending work. A single-line
progress bar with a percentage or fraction displayed is preferred. For example:
`[\▓\▓\▓\▓\░░░░░░] 40%` (where \▓ is a filled block and \░ is an empty block)
is a possible style. If Unicode block characters don’t render well in some
environments, a fallback to ASCII like `====>` and `----` can be used. The
progress bar should update in-place. The length of the bar can be fixed (say 40
or 50 characters) for consistency. The colour for completed portion might be
green (indicating progress/success), and the pending portion gray. If colour is
disabled, the difference is just via the characters themselves. The CLI does
not use overly ornate Unicode symbols that might not be universally supported
(like exotic emoji) for the core progress bar, to avoid rendering issues.

**Status Messages and Alignment:** When printing stage or status lines (like
the “[1/6] Parsing…” example), consider aligning the text in columns for a
clean look. Allocating, say, 25 characters for the stage description lets the
checkmarks or “done” text line up vertically. Consistent indentation or
alignment makes scanning easier, as the eyes can follow a column of ✔ symbols,
for instance. However, ensure that this alignment adapts or truncates if
message text is longer in some languages – indicatif’s width calculation can be
relied upon or the formatting can apply a max width, adding ellipsis if needed
rather than breaking layout. The layout should also be **responsive to terminal
width**: if the terminal is very narrow, long messages should gracefully wrap
to the next line with an indent. (Clap’s help messages do this wrapping; for
output, messages can be kept short or terminal width detected via
`indicatif::Terminal` features or similar.)

**Differentiating Text Styles:** Netsuke uses **text styling (ANSI escape
codes)** to differentiate categories of output:

- **Informational status** (normal progress messages): perhaps coloured
  **cyan** or **blue** to distinguish from normal text. Blue/cyan is often used
  for info, and it generally has good contrast on both black and white
  backgrounds (especially cyan). Avoid using solely blue if it might be dark on
  black; cyan is usually brighter. Alternatively, styling can use bold instead
  of colour, or italic. Bold white (bright gray) could also highlight Netsuke
  messages.

- **Success messages**: coloured **green** (with a check mark). E.g., the “Build
  succeeded” line in green bold. Green typically signals success and is
  readable on dark backgrounds; on light backgrounds, the terminal typically
  has a darker green that still contrasts.

- **Error messages**: coloured **red** and bold. The word “Error:” in bright
  red, and any important portions (like file names or error codes) could be
  underlined or in red as well. It must be ensured that even without colour,
  the error stands out due to the "Error" label and maybe an exclamation or
  cross symbol.

- **Warnings**: coloured **yellow**. Warnings (non-fatal issues) can be
  prefixed “Warning:” and coloured yellow. Yellow on white can be low contrast,
  but most terminals render “yellow” as a brownish or goldenrod that is
  visible; still, it may be preferable to use bold text plus maybe magenta if
  yellow is problematic. (Some CLI tools use magenta for warnings to be more
  legible on light backgrounds.)

- **Command output**: ideally not recoloured from what the command itself
  outputs. If the compiler prints warnings with its own colour (e.g., GCC
  prints warnings in yellow typically), the CLI preserves that since the tool
  is not altering the bytes, just piping them. If interception occurred, those
  escape codes should be kept. Netsuke itself should not recolour the command
  text (except maybe dim it or indent to separate – but it generally remains
  as-is for authenticity).

- **Interactive elements**: If in the future any interactive prompt appears
  (not currently planned in core design), use clear prompts like “(Y/n)” for
  yes/no, and ensure the default is capitalized. Keep any prompt text short and
  to the point, with the choices obvious.

Remember that any use of colour or style will be conditional on detection and
user preference (no colour mode disables it, etc.). Also, provide sufficient
contrast in chosen colours: for example, a dark blue might be hard to read on
black; bright blue (cyan) should be used instead.

**Use of Emojis and Symbols:** Emojis like ✅ and ❌ can add clarity and a bit
of friendly character, but use them judiciously[^2]. The CLI uses:

- **“✅” (green check mark)** for final success or possibly for each completed
  stage in progress output.

- **“❌” (red cross mark)** for failures or critical errors.

- **“⚠️” (warning sign)** for warnings if a symbol is desired (or maybe the
  Unicode “!” in triangle).

- **“▶” or “→”** for indicating something is in progress or being started.

- **“…” (ellipsis)** after messages that are waiting (e.g., “Parsing
  manifest…”).

These symbols will be placed adjacent to text, not alone. For instance, “✅
Done” or “⚠️ Warning: Low disk space”. By having text after the emoji, the
output ensures it’s understandable even if the emoji doesn’t render or is read
oddly.

ASCII art diagrams or banners that rely on monospacing and alignment are
avoided, as these often don’t scale or may break on different setups. The only
ASCII art might be the progress bar itself or the error snippet drawn by
`miette` (which is fine).

**Headings and Sections in Output:** If Netsuke needs to show a sectioned
output (perhaps in verbose mode it might dump some structured info), formatting
should use clear headings. For example, a heading could be underlined with
`===` or prefixed with `##` to stand out. E.g.:

```text
== Build Summary ==
Targets built: 10
Time elapsed: 3.2s
Cache hits: 8
```

This is just an idea if summary info is presented. The key is to **label
sections** clearly, similar to how human-written logs have separators. However,
do not unnecessarily add such sections unless they provide value; a concise
output is better for quick scanning.

**Help Output Formatting:** Clap handles the formatting of `--help` output,
which typically includes usage, then a list of arguments and flags. The
descriptions in the help should stay **one line each** (to keep it scannable)
and may include examples. Clap allows custom-placing of example usage in the
long description or as part of about. An example might look like:

```text
USAGE:
    netsuke [OPTIONS] [SUBCOMMAND] [targets...]

OPTIONS:
    -f, --file <FILE>      Path to the Netsuke manifest (defaults to "Netsukefile")
    -C, --directory <DIR>  Change to this directory before doing anything
    -j, --jobs <N>         Set the number of parallel build jobs (passed to Ninja)
    -v, --verbose          Enable verbose logging output
    -q, --quiet            Minimal output (only errors and essential info)
    --diag-json            Output error diagnostics in JSON format
    --no-color             Disable coloured output

SUBCOMMANDS:
    build        Build specified targets (or default targets if none given) [default]
    clean        Remove build artifacts and intermediate files
    graph        Display the build dependency graph (DOT format or visual)
    manifest     Write the generated Ninja manifest to a file without running Ninja
    help         Print this help information
```

This hypothetical help text shows how these features integrate into the CLI
interface. The layout uses columns where possible (Clap does that
automatically, wrapping descriptions nicely). Double-checking ensures that
these help messages are themselves localizable (Clap supports localization, but
manual overrides may be applied to route text through Fluent, depending on how
Clap’s derive macro interacts with internationalization).

**Ephemeral vs Persistent Output:** When using `indicatif` progress bars, by
default once a progress bar finishes, it can either persist (leave the bar on
screen) or be cleared. It is often desirable to **persist important progress
lines** so that a user can scroll back and see them after completion. For
example, after finishing, a line can remain that says “Stage 6/6: Running build
commands… ✔ done”. Some CLI tools erase the progress on completion and replace
it with a single “Done” line. That is clean, but it loses the context of each
stage. A compromise is to persist a summary of each stage with a checkmark (so
the user sees all 6 stages listed with checkmarks at the end). This also is
helpful in logs: one can see each stage took place. The final output might look
like:

```text
✔ Stage 1: Parse manifest (123 ms)
✔ Stage 2: Expand templates (8 ms)
✔ Stage 3: Generate build plan (5 ms)
✔ Stage 4: Validate build graph (2 ms)
✔ Stage 5: Write Ninja file (16 ms)
✔ Stage 6: Run build commands (2.1 s, 2 tasks)
✔ Build succeeded in 2.3 s
```

Here timing info per stage is included (which is optional, but could be useful
in verbose mode or as a final summary). The layout is tabular: stage name and
in parentheses the time. This gives advanced users insight into performance
(for example, if template expansion took unusually long, it becomes visible).
Per-stage timings typically appear only in verbose mode to avoid cluttering
normal output. But the alignment and format would be as above, and each line
starts with a checkmark for easy identification of completion.

**Spacing and Punctuation:** A consistent style for punctuation is followed in
messages. Error messages should be complete sentences (starting with capital
letter, ending with a period), or at least consistently formatted phrases.
Choose either to include trailing punctuation or not and stick to it. Often,
error messages printed by CLIs do not include a period to avoid looking like a
paragraph; they treat it as a label and description. But there are
multi-sentence hints, etc., so periods will likely be included for full
sentences in hints and descriptions. The main error line might be formatted as
“Error: `<description>`.” with a period, or some styles omit it (Rust’s error
messages typically have no period at end of main error line). Rust style can be
followed: no period on the main error message (because additional cause lines
follow), but periods in the Hint or cause sentences as needed. What matters is
translators know the convention and keep it.

**Logging (Verbose Mode):** In verbose/debug modes, if internal logs are
printed, they can be prefixed with a debug marker. For example, “[DEBUG]
Resolved 45 targets” or “[TRACE] Reading config file…”. Extremely low-level
logs are unlikely to be exposed unless requested, but when they are, having
them easily filterable (with tags like DEBUG/TRACE) is useful. They should go
to stderr as well, and could be coloured faintly (dim white) so as not to
distract from primary output.

**Testing Visuals:** The CLI output is tested on different terminals (Windows
`cmd` vs PowerShell vs various Linux terminals) to ensure the Unicode and
colour sequences behave as expected. If necessary (for Windows older consoles
that don’t support ANSI), the implementation might include the Windows console
mode enabling code or use crates that abstract that. Modern Windows terminals
support ANSI colours and Unicode well, so likely it’s fine.

By adhering to these visual design guidelines, the Netsuke CLI will present
information in a **clean, professional, and user-friendly manner**. Key
information will stand out (errors in red, success in green, prompts clearly
marked), and the output will avoid common pitfalls like misaligned text or
overuse of colour without text. The style choices also reinforce the Netsuke
brand as a modern tool: use of emojis and Unicode where appropriate can add a
friendly touch, but the core output remains **structured and informative**, not
gimmicky[^2]. Ultimately, these visual decisions serve the goals of
**usability, readability, and accessibility**, ensuring that whether a user is
glancing quickly at their screen, poring over a log file, or listening via a
screen reader, they can quickly grasp what Netsuke is communicating.

## References

- Harini Sampath et al., *“Accessibility of Command Line Interfaces,”* CHI ’21
  – Recommendations for making CLI output accessible.

- Netsuke Design Document – error handling strategy with `miette` diagnostics
  and JSON output for CI.

[^1]: Reddit discussion on Section 508 guidelines for command-line interfaces.
    <https://www.reddit.com/r/accessibility/comments/1em96h5/section_508_guidelines_for_command_line_interfaces/>

[^2]: Command Line Interface Guidelines. <https://clig.dev/>

[^3]: OrthoConfig documentation detailing layered configuration precedence.
    <https://github.com/leynos/ortho-config/blob/0373169f70dcb5e98da8deeebe1c7570e77a8194/README.md#L18-L26>
