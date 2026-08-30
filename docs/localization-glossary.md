# Netsuke localization glossary

<!-- markdownlint-disable-file MD060 -->

<!-- MD060 (table-column-style) is suppressed for this file only. Table padding
is owned by the repository formatter (`mdtablefix`, run by `make fmt`), whose
column widths disagree with MD060's display-width model for the right-to-left,
Indic, and combining-mark scripts used in the per-locale tables below. The rule
stays enabled everywhere else. -->

This document is the source of truth for Netsuke terminology across every
locale. It records the preferred, allowed, and forbidden forms of each
source-language term, explains which terms resist translation and why, and sets
out per-locale terminology so translators and reviewers apply the same
vocabulary consistently. It complements the
[translator guide](translators-guide.md), which owns the locale registry,
Fluent mechanics, and fallback policy, and the
[localization style guide](localization-styleguide.md), which owns voice, tone,
and mechanics.

## Schema

Each row in a terminology table represents one term record. The columns are:

- `title`: the canonical term as it appears in the source catalogue or
  repository documentation.
- `preferred`: the form translators must use.
- `allowed`: acceptable alternatives, separated by semicolons.
- `forbidden`: forms that must not appear, separated by semicolons.

An em dash (—) means the set is empty. Netsuke's source catalogue is
`locales/en-US/messages.ftl`; the `en-US` terms below are authoritative for
message content. Per-locale sections map the difficult subset of these terms
into each shipped locale.

Keep forbidden values conservative. Add a value only for an observed
misspelling, an identifier variant that would break a literal interface, or a
collision between distinct Netsuke concepts.

## Source terminology (en-US)

### Product and ecosystem names

These names are invariant in every locale. They are never translated,
transliterated, inflected with affixes that alter the written form, or respelt,
although surrounding text follows the target language's grammar.

Table 1: product and ecosystem names

| title         | preferred       | allowed                                                                | forbidden                                          |
| ------------- | --------------- | ---------------------------------------------------------------------- | -------------------------------------------------- |
| Netsuke       | `Netsuke`       | `netsuke` (executable, library target, command, OS package, code path) | —                                                  |
| netsuke-build | `netsuke-build` | `cargo install netsuke-build`; `cargo binstall netsuke-build`          | `netsuke` (for the Cargo package); `netsuke_build` |
| Netsukefile   | `Netsukefile`   | `Netsuke manifest`; `manifest file`                                    | `NetsukeFile`; `Netsuke file`                      |
| Ninja         | `Ninja`         | `ninja` (executable or literal command)                                | —                                                  |
| Fluent        | `Fluent`        | `Project Fluent`; `Fluent Translation List`; `FTL`                     | —                                                  |
| Jinja         | `Jinja`         | `MiniJinja` (the Rust implementation)                                  | `Jinja2` (for the dialect)                         |
| YAML          | `YAML`          | —                                                                      | `Yaml`; `yaml` (in prose)                          |
| Graphviz      | `Graphviz`      | —                                                                      | —                                                  |
| DOT           | `DOT`           | `Graphviz DOT`                                                         | `dot` (for the language)                           |
| Rust          | `Rust`          | —                                                                      | —                                                  |
| CLDR          | `CLDR`          | `Unicode CLDR`                                                         | —                                                  |

### Manifest and build-model concepts

These terms carry precise, mutually exclusive meanings in Netsuke's build
model. Substituting a near-synonym in any language blurs a distinction the
product depends on.

Table 2: manifest and build-model terms

| title                 | preferred               | allowed                                      | forbidden                     |
| --------------------- | ----------------------- | -------------------------------------------- | ----------------------------- |
| manifest              | `manifest`              | `Netsuke manifest`; `manifest file`          | `configuration file`          |
| target                | `target`                | `build target`; `default target`             | `rule`; `goal`; `objective`   |
| action                | `action`                | `build action`; `action catalogue`           | `rule`; `task`                |
| rule                  | `rule`                  | `Ninja rule`                                 | `target`; `action`            |
| dependency            | `dependency`            | `direct dependency`; `order-only dependency` | `dependancy`                  |
| dependencies          | `dependencies`          | `direct dependencies`; `dependency graph`    | `dependancies`                |
| order-only dependency | `order-only dependency` | —                                            | `weak dependency`             |
| build graph           | `build graph`           | `dependency graph`; `graph`                  | —                             |
| build edge            | `build edge`            | `edge`                                       | —                             |
| phony target          | `phony target`          | `implicitly phony target`                    | `fake target`; `false target` |
| default target        | `default target`        | `manifest default`                           | —                             |
| build                 | `build` (noun and verb) | `build run`                                  | `compilation` (generic use)   |
| clean                 | `clean` (subcommand)    | —                                            | `purge`; `wipe`               |
| generate              | `generate` (subcommand) | —                                            | —                             |
| artefact              | `artefact`              | `build artefact`; `graph artefact`           | —                             |
| workspace root        | `workspace root`        | —                                            | `project root` (in messages)  |
| working directory     | `working directory`     | `current directory`                          | `folder`                      |
| dyndep                | `dyndep`                | `dyndep file`; `generated dyndep file`       | `dynamic dependency file`     |
| serial dependency     | `serial dependency`     | `serial dependency ordering`                 | —                             |

### Template and expression language

Netsuke manifests embed Jinja templates and expressions. Several of these terms
are identifiers or near-identifiers that must survive translation byte-for-byte.

Table 3: template and expression terms

| title               | preferred           | allowed                                 | forbidden                                |
| ------------------- | ------------------- | --------------------------------------- | ---------------------------------------- |
| template            | `template`          | `Jinja template`                        | `boilerplate`                            |
| standard library    | `standard library`  | `template standard library`; `stdlib`   | —                                        |
| macro               | `macro`             | `manifest macro`                        | `function` (for macros)                  |
| expression          | `expression`        | `when expression`; `foreach expression` | `formula`                                |
| `foreach`           | `foreach`           | —                                       | `for each`; `for-each`                   |
| `when`              | `when`              | —                                       | —                                        |
| `vars`              | `vars`              | —                                       | `variables` (for the key)                |
| `command_available` | `command_available` | `command_available(name, **kwargs)`     | `command-available`; `command available` |
| glob                | `glob`              | `glob pattern`                          | `wildcard search`                        |
| helper              | `helper`            | `template helper`; `built-in helper`    | —                                        |
| filter              | `filter`            | `Jinja filter`                          | —                                        |
| fetch               | `fetch` (helper)    | `fetch helper`                          | `download` (for the helper)              |
| which               | `which` (helper)    | `which helper`                          | —                                        |

### Localization and message concepts

Table 4: localization terms

| title             | preferred           | allowed                             | forbidden                 |
| ----------------- | ------------------- | ----------------------------------- | ------------------------- |
| locale            | `locale`            | `locale tag`; `BCP 47 tag`          | `language` (for the tag)  |
| placeable         | `placeable`         | `Fluent placeable`; `variable`      | `placeholder text`        |
| message key       | `message key`       | `key`                               | `message ID`              |
| plural category   | `plural category`   | `CLDR plural category`              | `plural form` (imprecise) |
| select expression | `select expression` | —                                   | `switch`                  |
| fallback          | `fallback`          | `fallback chain`                    | —                         |
| source catalogue  | `source catalogue`  | `source catalog`; `en-US catalogue` | —                         |

### Command-line and diagnostic vocabulary

Table 5: CLI and diagnostic terms

| title                | preferred              | allowed                      | forbidden                    |
| -------------------- | ---------------------- | ---------------------------- | ---------------------------- |
| allowlist            | `allowlist`            | `host allowlist`             | `whitelist`                  |
| blocklist            | `blocklist`            | `blocked hosts`              | `blacklist`                  |
| host pattern         | `host pattern`         | —                            | —                            |
| scheme               | `scheme`               | `URL scheme`                 | `protocol` (for URL schemes) |
| cache                | `cache`                | `fetch cache`; `which cache` | —                            |
| stdout               | `stdout`               | `standard output` (prose)    | `STDOUT`                     |
| stderr               | `stderr`               | `standard error` (prose)     | `STDERR`                     |
| exit status          | `exit status`          | `status`                     | `error level`                |
| signal               | `signal`               | —                            | —                            |
| broken pipe          | `broken pipe`          | —                            | —                            |
| timeout              | `timeout`              | `timed out`                  | `time-out`                   |
| capture              | `capture` (mode)       | `capture mode`               | —                            |
| streaming            | `streaming` (mode)     | `streaming mode`             | —                            |
| verbose              | `verbose`              | `verbose logging`            | `noisy`                      |
| pipeline             | `pipeline`             | `build pipeline`             | —                            |
| stage                | `stage`                | `pipeline stage`             | `phase`; `step` (in status)  |
| environment variable | `environment variable` | `env var` (informal)         | —                            |
| path                 | `path`                 | `file path`                  | `route`                      |
| UTF-8                | `UTF-8`                | —                            | `UTF8`; `utf-8` (in prose)   |
| canonicalize         | `canonicalize`         | —                            | `normalise` (for paths)      |

## Localizability notes

The terms below are the ones most likely to go wrong in translation. Each note
explains the hazard; the per-locale sections apply these notes to each shipped
locale.

- **`target` versus `action` versus `rule`.** Netsuke separates the three
  concepts, while many languages use one everyday word for all of them (compare
  French *cible*, *action*, *règle*). A target is a build output or named build
  entry; an action is an implicitly phony target; a rule is the Ninja construct
  a target references. Translations must keep three distinct words and use them
  consistently.
- **`build`.** English uses one word as noun and verb; many languages must
  split the pair (German *Build* as a loan noun beside the verb *erstellen*).
  Pick one noun and one verb per locale and keep them stable.
- **`dependency` (direct versus order-only).** "Order-only dependency" is
  Ninja jargon with no natural equivalent in most languages. Translate the head
  noun, keep the qualifier literal, and gloss it on first use if the target
  language would otherwise read it as "a dependency that is only an order".
- **`dyndep`, `glob`, `stdlib`, `phony`.** These are loan words even in
  English: they name Ninja and Unix constructs, not ideas. Keep them as loan
  words in every locale; translating them severs the link to the underlying
  tool and its documentation.
- **`manifest`.** In several languages the everyday cognate means a shipping
  document or a political manifesto. Prefer the local software sense where one
  is established; otherwise keep the loan word rather than fall back to
  "configuration file", which Netsuke forbids because a manifest is not
  configuration.
- **`allowlist` and `blocklist`.** Prefer local equivalents of allow/block
  lists over legacy white/black metaphors, matching the source's own choice.
- **`cache`.** Widely borrowed, but the local form varies (French *cache*,
  German *Cache*, Spanish *caché*, Russian *кэш*). Use the established local
  form; do not invent a descriptive phrase.
- **`placeable`.** Fluent-specific jargon. Most locales should keep the
  Fluent term untranslated on first use with a local gloss, because it names a
  syntax object translators must not alter.
- **`stdout`, `stderr`, `UTF-8`, identifiers.** Never translated,
  transliterated, or respaced in any locale, including locales that
  transliterate freely elsewhere.
- **`stage` and status vocabulary.** Status lines such as `pending`,
  `in progress`, `done`, and `failed` render in aligned, space-constrained
  output. Locales should choose short, parallel forms and keep the set
  internally consistent rather than translating each state in isolation.
- **`artefact`.** The source catalogue uses the spelling `artefact`. Locales
  translate the concept (a produced file), not the spelling.
- **`fetch` and `which`.** As helper names they are identifiers and stay
  literal; as prose ("the fetch helper") the surrounding words translate.

## Locale terminology

Each section below records how the difficult subset of the source terminology
is rendered in one shipped locale, together with register notes and the loan
words that locale conventionally keeps. `en-US` is the source and has no
section. Terms listed as invariant in Table 1 stay invariant in every locale
and are not repeated per locale.

### Arabic (`ar`)

Netsuke's Arabic output should be Modern Standard Arabic (MSA), never a
regional dialect. Both the
[Microsoft Arabic localization style guide](https://aka.ms/arabic-styleguide)
and the
[WordPress.com Arabic translation style guide](https://translate.wordpress.com/glossaries-and-style-guides/arabic-style-guide/)
require MSA and a formal, professional register for software strings.
Microsoft's guide distinguishes registers by message type: prompts that
instruct the reader use the imperative addressed to "you" (e.g. "Would you like
to continue?" becomes a direct second-person question), while system commands
and status text use the infinitive/verbal-noun (maṣdar) form instead of an
imperative or first person. Netsuke diagnostics are declarative status reports,
not instructions to the reader, so they should follow the second pattern: state
the condition using a verbal noun or passive construction, and reserve direct
second-person address ("أدخل…", "تحقق من…") for genuinely interactive prompts.
Arabic has no formal/ informal (T–V) pronoun split comparable to French
*tu/vous*; MSA itself carries the formality, and Microsoft's guide additionally
asks for gender-neutral phrasing (plural or collective nouns) rather than the
generic masculine wherever the reader's gender is unknown.

Netsuke's technical nouns split between native Arabic terms and English loans
written in Latin script inline with the Arabic text. **Cache**, **template**,
**dependency**, **target**, **rule**, and **environment variable** all have
established Arabic equivalents in Microsoft and vendor documentation (see the
table). **Macro** and **artefact**, by contrast, commonly stay as
transliterated or bare English loans in Arabic technical prose: Office
documentation transliterates macro as **ماكرو**, and Arabic DevOps writing
frequently keeps *`artifact`* in Latin script inside an Arabic sentence rather
than coining a native word. Because Arabic script is right-to-left and
Netsuke's diagnostics embed Latin-script identifiers, paths, and flags
(`{ $path }`, `command_available`, `stdout`), directional runs can visually
reorder against surrounding Arabic punctuation; the translator guide's
direction-mark rule (wrapping embedded Latin runs with the appropriate Unicode
direction marks) applies to every Netsuke string that mixes scripts, not only
to this locale's glossary entries.

The main hazard is **manifest**. The bare word **بيان** overwhelmingly reads as
"statement," "declaration," or "political manifesto" in everyday and news
Arabic — the same word appears in phrases like "بيان مشترك" (joint statement)
and "بيان الوزارة" (the ministry's statement). Google's Arabic Android
developer documentation avoids this collision by always qualifying the word as
**ملف البيان** ("manifest file"), never bare **بيان**; Netsuke should do the
same. A second hazard is that **target**, **action**, and **rule** must stay
three visibly distinct Arabic words (هدف / إجراء / قاعدة) throughout
diagnostics — a translator reaching for near-synonyms under space pressure
could easily blur them. A third hazard is **order-only dependency** and **phony
target**: neither has a fixed vendor-attested Arabic term, so the table below
marks them as constructed rather than established.

Worked example, preserving the `{ $path }` placeable exactly:

```text
تعذّر تحميل ملف البيان في { $path }.
```

Table 6: Arabic terminology

| en-US                  | preferred                         | notes                                                                                                                                                                                                                                                                                              |
| ---------------------- | --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | ملف البيان (milaff al-bayān)      | Not bare **بيان** — that reads as "statement/manifesto." Matches [Android Arabic developer docs](https://developer.android.com/guide/topics/resources/app-languages?hl=ar), which use "ملف البيان" for `AndroidManifest`.                                                                          |
| target                 | هدف (hadaf)                       | Build output or named build entry; kept distinct from إجراء and قاعدة. Attested in Arabic discussion of GNU Make `TARGET`/`.PHONY` semantics (see phony target row).                                                                                                                               |
| action                 | إجراء (ijrā')                     | Netsuke-specific: an implicitly phony target. Standard Arabic software term for "action" (menus, commands); distinct from هدف and قاعدة.                                                                                                                                                           |
| rule                   | قاعدة (qā'ida)                    | The Ninja rule construct a target references; standard Arabic translation of "rule."                                                                                                                                                                                                               |
| dependency             | تبعية (taba'iyya)                 | Standard term in Arabic package-manager and build-tool writing (e.g. npm/Composer "تبعيات").                                                                                                                                                                                                       |
| order-only dependency  | تبعية ترتيب فقط                   | Constructed compound; no vendor-attested Arabic term exists for this Ninja-specific concept. Gloss on first use.                                                                                                                                                                                   |
| build (noun)           | البناء (al-binā')                 | The build process/output as a noun.                                                                                                                                                                                                                                                                |
| build (verb)           | بناء (binā')                      | Verbal-noun (maṣdar) form used for the system-command register, matching the infinitive-style command pattern in the [WordPress Arabic style guide](https://translate.wordpress.com/glossaries-and-style-guides/arabic-style-guide/). Same spelling as the noun; disambiguate by sentence context. |
| build graph            | الرسم البياني للبناء              | Standard Arabic CS term "رسم بياني" (graph) applied to the build's dependency graph.                                                                                                                                                                                                               |
| phony target           | هدف وهمي                          | "وهمي" (fake/dummy) is the conventional Arabic gloss for Make/Ninja `.PHONY`; attested in Arabic developer discussion of [Makefile TARGET/PHONY](https://www.reddit.com/r/C_Programming/comments/1ghy847/the_perfect_makefile/?tl=ar).                                                             |
| artefact               | `artifact` (أرتيفاكت)             | Kept as an English/transliterated loan in Arabic DevOps prose rather than a native calque; "ناتج البناء" (build output) is an acceptable descriptive gloss on first use.                                                                                                                           |
| working directory      | الدليل الحالي / دليل العمل الحالي | Both forms are widely used in Arabic Linux/CLI documentation for "current working directory."                                                                                                                                                                                                      |
| workspace root         | جذر مساحة العمل                   | "مساحة العمل" is the established Arabic term for "workspace" (VS Code, Chrome DevTools Arabic docs); "جذر" (root) composes as in "المجلد الجذر لمساحة العمل" in Arabic developer tutorials.                                                                                                        |
| cache                  | ذاكرة التخزين المؤقت              | Standard term, confirmed on [Microsoft Edge Arabic support](https://support.microsoft.com/ar-sa/edge/what-to-do-if-microsoft-edge-isn-t-working). Informal "كاش" (loan) also circulates but is not the formal-register choice.                                                                     |
| allowlist              | قائمة السماح                      | Confirmed on [Microsoft Defender for Office 365 Arabic docs](https://learn.microsoft.com/ar-sa/defender-office-365/tenant-allow-block-list-about) ("قائمة السماح/الحظر").                                                                                                                          |
| blocklist              | قائمة الحظر                       | Confirmed alongside قائمة السماح in the same Microsoft source.                                                                                                                                                                                                                                     |
| template               | قالب (qālib)                      | Standard Arabic term across Microsoft Office and Azure Pipelines Arabic documentation.                                                                                                                                                                                                             |
| macro                  | ماكرو (mākrū)                     | Established transliterated loan, standard in Arabic Office/VBA documentation; not calqued.                                                                                                                                                                                                         |
| environment variable   | متغير بيئة (mutaghayyir bī'a)     | Confirmed in the official Arabic translation of Wine ([Debian sources ar.po](https://sources.debian.org/src/wine/4.0-2/po/ar.po/), "متغير البيئة").                                                                                                                                                |
| exit status            | رمز الخروج (ramz al-khurūj)       | Confirmed usage ("رمز خروج غير صفري") in [Arabic Azure Pipelines coverage](https://apidog.com/ar/blog/azure-pipelines-api-testing/); "حالة الخروج" is a less common synonym.                                                                                                                       |
| stage (pipeline stage) | مرحلة (marḥala)                   | Standard Arabic term for a pipeline/process phase; used throughout Arabic DevOps and project-management writing.                                                                                                                                                                                   |
| locale                 | الإعدادات الإقليمية                 | Confirmed Microsoft term across [Microsoft Support](https://support.microsoft.com/ar-SA/Excel/set-a-locale-or-region-for-data-power-query) and [Microsoft Learn](https://learn.microsoft.com/ar-sa/answers/questions/2774241/10) Arabic content.                                                   |
| placeable              | عنصر نائب (placeholder)           | No fixed Arabic industry term exists for Fluent's specific "placeable"; adapted from the well-established Arabic UI term for "placeholder."                                                                                                                                                        |

### Czech (`cs`)

Czech diagnostics and CLI help address the reader with the formal second person
plural (vykání: "vy"/"váš"), the register the Mozilla Czech localization team
documents for desktop software: "use the second person plural (vy - vykání) to
address the user" while avoiding first-person and personified phrasing such as
"Stahuji…" in favour of impersonal or passive forms like "Stahuje se…"
([Mozilla Czech style guide](https://mozilla-l10n.github.io/styleguides/cs/general.html)).
This suits Netsuke's calm, non-marketing tone: diagnostics stay impersonal and
state the condition rather than addressing "you" directly, and any CLI help
that does address the operator uses vykání consistently.

Netsuke's formal register favours native Czech calques over raw English loans
for core build vocabulary: "build" becomes "sestavení" (noun) and "sestavit"
(infinitive verb, per the Czech UI convention of using infinitives for
actions), "cache" is "mezipaměť", and "template" is "šablona" — all attested in
the
[L10N.cz translators' dictionary](https://www.l10n.cz/wiki/Slovn%C3%ADky/P%C5%99ekladatelsk%C3%BD_slovn%C3%ADk/).
Lower-level Ninja/Netsuke jargon such as glob, stdlib, dyndep, and phony stays
as an English technical identifier when naming a concrete Ninja construct, but
is glossed in Czech prose ("vzor glob", "fiktivní cíl"). "Pipeline" is commonly
left as an undeclined English loan in Czech DevOps writing, while "stage" is
rendered natively as "fáze", as seen throughout Czech GitLab/Jenkins tutorials.

The main hazard is "manifest": in general Czech it most often means a political
manifesto, not a build-time input file. Software contexts (package managers,
app manifests) already use "manifest" as an established loan, so Netsuke keeps
it, but any surrounding sentence must make the file-input sense unambiguous
rather than relying on the bare word. "Akce" (action) similarly collides with
the everyday sense of a public event; pair it with "cíl" or "pravidlo" nearby
so the three concepts stay distinguishable, since Netsuke treats target,
action, and rule as different things. "Cíl" (target) also carries an everyday
sense of "aim/goal", which context in build output resolves without difficulty.
No casing or script traps apply, since Czech uses the Latin script with
diacritics that do not affect identifier casing.

Worked example, preserving the placeable exactly:

```text
Nepodařilo se načíst manifest v { $path }.
```

Table 7: Czech terminology

| en-US                  | preferred                            | notes                                                                                                                                                 |
| ---------------------- | ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest                             | Loan word; also means "political manifesto" generally — see hazards above. [Microsoft Czech style guide](https://aka.ms/czech-styleguide)             |
| target                 | cíl                                  | Distinct from action/rule; confirmed in Czech Make teaching material ([d3s.mff.cuni.cz](https://d3s.mff.cuni.cz/cz/teaching/nswi177/202425/labs/10/)) |
| action                 | akce                                 | Implicit phony target; disambiguate from the everyday "event" sense                                                                                   |
| rule                   | pravidlo                             | The Ninja construct a target references; same source as target                                                                                        |
| dependency             | závislost                            | Standard term, [L10N.cz dictionary](https://www.l10n.cz/wiki/Slovn%C3%ADky/P%C5%99ekladatelsk%C3%BD_slovn%C3%ADk/)                                    |
| order-only dependency  | pořadová závislost                   | Qualifies "závislost"; not independently attested, built on the established base term                                                                 |
| build (noun)           | sestavení                            | Preferred formal register; colloquial "build" loan also occurs                                                                                        |
| build (verb)           | sestavit                             | Infinitive, per Czech UI convention (Mozilla style guide)                                                                                             |
| build graph            | graf sestavení                       | Compound of "graf" + "sestavení"                                                                                                                      |
| phony target           | fiktivní cíl                         | Rendering used in Czech GNU Make tutorials for `.PHONY` targets                                                                                       |
| artefact               | artefakt                             | Loan, standard in Czech DevOps writing (e.g. Jenkins/CI guides)                                                                                       |
| working directory      | pracovní adresář                     | Standard                                                                                                                                              |
| workspace root         | kořenový adresář pracovního prostoru | Compound; no single established short form found                                                                                                      |
| cache                  | mezipaměť                            | [L10N.cz dictionary](https://www.l10n.cz/wiki/Slovn%C3%ADky/P%C5%99ekladatelsk%C3%BD_slovn%C3%ADk/)                                                   |
| allowlist              | seznam povolených                    | Attested in Czech security/filtering docs (AdGuard)                                                                                                   |
| blocklist              | seznam blokovaných                   | Attested in Czech Azure/AdGuard docs                                                                                                                  |
| template               | šablona                              | [L10N.cz dictionary](https://www.l10n.cz/wiki/Slovn%C3%ADky/P%C5%99ekladatelsk%C3%BD_slovn%C3%ADk/)                                                   |
| macro                  | `makro`                              | Established adapted loan                                                                                                                              |
| environment variable   | proměnná prostředí                   | Ubiquitous in Czech Microsoft Learn docs                                                                                                              |
| exit status            | návratový kód                        | [L10N.cz dictionary](https://www.l10n.cz/wiki/Slovn%C3%ADky/P%C5%99ekladatelsk%C3%BD_slovn%C3%ADk/) (as "exit code")                                  |
| stage (pipeline stage) | fáze                                 | Common in Czech CI/CD tutorials (GitLab/Jenkins)                                                                                                      |
| locale                 | národní prostředí                    | [L10N.cz dictionary](https://www.l10n.cz/wiki/Slovn%C3%ADky/P%C5%99ekladatelsk%C3%BD_slovn%C3%ADk/); informal "locale" loan also seen                 |
| placeable              | zástupný výraz                       | No established Czech Fluent term found; descriptive gloss, treat as tentative                                                                         |

### Welsh (`cy`)

Netsuke's Welsh diagnostics, CLI help, and status output should address the
reader with the second-person plural/formal pronoun `chi`, not the familiar
`ti`. The
[Microsoft Welsh Localization Style Guide](https://learn.microsoft.com/en-us/globalization/reference/microsoft-style-guides)
states plainly that Microsoft products "use the formal form of address (chi)"
and gives paired examples such as `Ydych chi'n siŵr...?` over
`Wyt ti'n siŵr...?` for confirmation prompts, and recommends the `-wch`
imperative ending for instructions. This matches the register used throughout
the Welsh Government's own bilingual-software standards,
[Safonau a Chanllawiau ar gyfer Meddalwedd Dwyieithog](https://orca.cardiff.ac.uk/44056/1/3962.pdf)
(Bwrdd yr Iaith Gymraeg), which likewise assumes an impersonal or `chi`-form
register for system messages and avoids `ti` outside of casual consumer apps
aimed at children. Netsuke's diagnostics are addressed to developers and CI
operators, so `chi` (or the impersonal passive, e.g. `methwyd â...`) is the
correct default; `ti` should not appear anywhere in the CLI.

Several Netsuke terms are best left as English loan words rather than calqued.
`macro` is used unchanged in Welsh technical writing (confirmed in
[Glosbe's Welsh dictionary](https://cy.glosbe.com/en/cy/macro), which draws on
Microsoft and KDE translation memory). `locale` is also conventionally left
unmutated and untranslated, including grammatically (`y locale`), a convention
documented explicitly in the Bwrdd yr Iaith Gymraeg standards above, which use
"locale" throughout their own Welsh-language text rather than coining a calque.
Identifiers such as `dyndep`, `foreach`, and `vars`, and product names such as
Netsuke, Ninja, Fluent, Jinja, and YAML, are invariant and are written in the
Latin script exactly as in English; all loan words and identifiers in this
section are given in the standard Latin alphabet used for Welsh.

The most serious hazard is `manifest`. Welsh has a well-established cognate,
`maniffesto`, but it means only a political manifesto (compare the
[Welsh translation of the Communist Manifesto](https://llyfrgell.porth.ac.uk/View.aspx?id=1982~4u~vx7iUPMZ),
`Maniffesto'r Blaid Gomiwnyddol`); using it for Netsuke's build manifest would
mislead readers. Netsuke keeps `manifest` as an unmutated loan word instead. A
second hazard is initial-consonant mutation: Welsh normally mutates a noun
following certain triggers (soft, nasal, or aspirate mutation), but the
[Microsoft Welsh Localization Style Guide](https://learn.microsoft.com/en-us/globalization/reference/microsoft-style-guides)
warns that trademarked and proper names must stay unchanged, and recommends
rephrasing sentences to avoid a mutation position before such a name. Product
names — Netsuke, Ninja, Fluent, Jinja, YAML — must therefore never be mutated
(never `Netsuke` → `Netsuke` with an initial `N` softened to nothing, nor
`Ninja` treated as a feminine noun triggering a following soft mutation);
diagnostics should be phrased so the product name does not sit in a mutating
position.

Worked example, preserving the placeable exactly:

```text
Methwyd â llwytho'r manifest yn { $path }.
```

Table 8: Welsh terminology

| en-US                  | preferred                | notes                                                                                                                                                            |
| ---------------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest                 | Loan word; `maniffesto` is a false friend (political manifesto).                                                                                                 |
| target                 | `targed`                 | Computing sense confirmed in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/target) ("the database/object an operation acts on").                                   |
| action                 | gweithred                | Distinct from `targed`/`rheol`; computing sense confirmed in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/action).                                                |
| rule                   | rheol                    | Confirmed for the "conditions and associated actions" computing sense in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/rule).                                      |
| dependency             | dibyniaeth               | Confirmed in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/dependency).                                                                                            |
| order-only dependency  | dibyniaeth trefn-yn-unig | Calque built on `dibyniaeth`; no independent attestation found, verify before shipping.                                                                          |
| build (noun)           | adeiladiad               | Result/artefact of building; pairs with the verb below.                                                                                                          |
| build (verb)           | adeiladu                 | Confirmed in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/build).                                                                                                 |
| build graph            | graff adeiladu           | Calque; `graff` is the standard Welsh computing term for "graph".                                                                                                |
| phony target           | `targed` ffug            | Calque; `ffug` = "false/sham", matching Ninja's phony-rule semantics.                                                                                            |
| artefact               | arteffact                | Standard Welsh spelling, confirmed in [Glosbe](https://cy.glosbe.com/en/cy/artefact).                                                                            |
| working directory      | cyfeiriadur gwaith       | `cyfeiriadur` = directory, attested for `PATH` in [KDE TM via Glosbe](https://cy.glosbe.com/kk/cy/%D0%B1%D0%B0%D2%93%D0%B4%D0%B0%D1%80%D0%BB%D0%B0%D0%BC%D0%B0). |
| workspace root         | gwraidd y gweithle       | Calque from `gwraidd` (root) + `gweithle` (workplace/workspace).                                                                                                 |
| cache                  | storfa                   | Confirmed for computing sense (e.g. "storfa gwrthrychau" = object cache) in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/cache).                                  |
| allowlist              | rhestr ganiatâu          | Calque; not independently attested, verify before shipping.                                                                                                      |
| blocklist              | rhestr rwystro           | Calque; not independently attested, verify before shipping.                                                                                                      |
| template               | templed                  | Confirmed in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/template).                                                                                              |
| macro                  | macro                    | Loan word, confirmed in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/macro).                                                                                      |
| environment variable   | newidyn amgylchedd       | Confirmed via `PATH` example in [Glosbe/KDE TM](https://cy.glosbe.com/en/cy/eg).                                                                                 |
| exit status            | statws ymadael           | Constructed from attested components (`statws`, `ymadael`); not found as a fixed term, verify before shipping.                                                   |
| stage (pipeline stage) | cam                      | Standard Welsh word for a step/stage in a process.                                                                                                               |
| locale                 | locale                   | Loan word, left unmutated even grammatically; see [Bwrdd yr Iaith Gymraeg standards](https://orca.cardiff.ac.uk/44056/1/3962.pdf).                               |
| placeable              | placeadwy                | Tentative calque; Fluent-specific jargon with no attested Welsh precedent found — consider keeping as the loan word `placeable` instead.                         |

### Danish (`da`)

Danish technical writing addresses the reader as **du** ("you", informal). The
De/Dem form is obsolete in software and reads as stilted or ironic; the
[Microsoft Danish style guide](https://chatscontrol.com/learn/style-guides/microsoft-da)
states plainly that Danish moved to du-form decades ago and that De sounds
antiquated. Netsuke diagnostics should therefore use imperative or impersonal
phrasing (as in the worked example below) rather than forcing a pronoun, but
any second-person address must be **du**.

Danish tech writing borrows English terms freely rather than coining calques,
and Netsuke should follow that norm. `build` (the noun), `cache`, and
`manifest` stay as English loans, written in the ordinary Latin alphabet with
no diacritic changes; `macro` is naturalised as `makro`. This matches usage
attested in the community
[dansk-gruppen/KLID English–Danish computing glossary](http://www.klid.dk/dansk/ordlister/ordliste.php3),
which lists `cache` alongside the native `hurtigbuffer` and `template` as
`skabelon`, and in Microsoft's own Danish support content, which uses
`miljøvariabel` for environment variable.

Two hazards matter most. First, `manifest` is a genuine loan for a file format,
but the same word in ordinary Danish also names a political manifesto (e.g.
"Det Kommunistiske Manifest") and, separately, a shipping or cargo manifest
(`ladningsmanifest`); readers will resolve the ambiguity from context, but
authors should never modify `manifest` with words that tilt it towards the
political sense. Second, do not calque "workspace root" as a solid compound
`arbejdsrod` — that existing Danish word means "clutter" or "mess", not
"workspace root". Use `arbejdsområdets rodmappe` or `projektroden` instead,
following the attested `rodmappe` used by
[Microsoft support for "root directory"](https://support.microsoft.com/da-dk/servicing/os/windows-7/2017/01/a-folder-that-is-created-under-the-root-of-the-system-drive-is-missing-entries-in-its-security-descr).
More generally, Danish forms compounds as single unspaced words
(`afhængighedsgraf`, not "afhængigheds graf"); calquing an English open
compound as separate words is a recognizable error and should be avoided
throughout diagnostics and help text.

```text
Kunne ikke indlæse manifestet på { $path }.
```

Table 9: Danish terminology

| en-US                  | preferred                | notes                                                                                                                                                                                                                                                                    |
| ---------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| manifest               | manifest                 | Loan; Windows also uses `manifest`/`manifestfil` for application manifests. See hazards above re: political/shipping senses.                                                                                                                                             |
| target                 | mål                      | Confirmed by the [GNU Make Danish translation](https://translationproject.org/latest/make/da.po) (`target` → `mål` throughout).                                                                                                                                          |
| action                 | handling                 | Per the [dansk-gruppen/KLID glossary](http://www.klid.dk/dansk/ordlister/ordliste.php3) (`action` → `handling`).                                                                                                                                                         |
| rule                   | regel                    | GNU Make da.po: "Ingen regel til at skabe mål" (`rule` → `regel`). Distinct word from `mål` and `handling`.                                                                                                                                                              |
| dependency             | afhængighed              | Standard in Danish package/build tooling, e.g. Debian's Danish installation guide ("afhængighed af pakken …").                                                                                                                                                           |
| order-only dependency  | rækkefølgeafhængighed    | GNU Make da.po renders "order-only" descriptively: "angiver kun rækkefølgen ift. målet" (indicates only the order). No single attested noun; this compound follows that gloss.                                                                                           |
| build (noun)           | build                    | Loan, as in general Danish CI/CD usage ("lave en build", "CI-build").                                                                                                                                                                                                    |
| build (verb)           | bygge                    | Widely attested, e.g. BeeWare's Danish contributor docs ("bygge … dokumentationen").                                                                                                                                                                                     |
| build graph            | byggegraf                | Calqued on the attested pattern `afhængighedsgraf` ("dependency graph"), seen in Danish dev discussion of Maven/Gradle module graphs.                                                                                                                                    |
| phony target           | `falsk` mål              | Direct match in GNU Make da.po: "Phony target" → "`Falsk` mål".                                                                                                                                                                                                          |
| artefact               | artefakt                 | Attested in Danish CI/CD discussion of build artefacts ("Pointen med CI er … at producere ét artefakt").                                                                                                                                                                 |
| working directory      | arbejdskatalog           | dansk-gruppen/KLID glossary; uses `katalog`, not `mappe`, for this term specifically.                                                                                                                                                                                    |
| workspace root         | arbejdsområdets rodmappe | Avoid the solid compound `arbejdsrod` (see hazards). `rodmappe` for "root directory" is attested in Microsoft support content.                                                                                                                                           |
| cache                  | cache                    | Loan, listed as a standard alternative to `hurtigbuffer`/`mellemlager` in the dansk-gruppen/KLID glossary.                                                                                                                                                               |
| allowlist              | tilladelsesliste         | Attested across multiple Danish vendor docs (Citrix, forwardemail.net) and matches Microsoft's move away from "whitelist".                                                                                                                                               |
| blocklist              | blokeringsliste          | Attested in [Microsoft Danish support content](https://support.microsoft.com/da-dk/servicing/dotnetframework/2017/03/xbap-content-is-not-resized-to-fit-a-firefox-browser-after-you-install-the-hotfix-from-kb-article-97) (`extensions.blocklist` → "blokeringsliste"). |
| template               | skabelon                 | dansk-gruppen/KLID glossary.                                                                                                                                                                                                                                             |
| macro                  | `makro`                  | Naturalised loan (e.g. Excel-`makro`); standard in Danish IT usage.                                                                                                                                                                                                      |
| environment variable   | miljøvariabel            | Confirmed in Microsoft Danish support articles on environment variables.                                                                                                                                                                                                 |
| exit status            | afslutningskode          | dansk-gruppen/KLID glossary; alternative `afslutningsstatus` also seen in vendor docs but `afslutningskode` is the glossary-preferred form.                                                                                                                              |
| stage (pipeline stage) | trin                     | Generic, well-understood term for a pipeline step; avoids clashing with `fase` (project phase) or `handling` (action).                                                                                                                                                   |
| locale                 | landestandard            | Used by Microsoft Danish localization and by the NVDA screen-reader's Danish translation team for the language/region setting.                                                                                                                                           |
| placeable              | pladsholder              | No dedicated Danish Fluent/l10n glossary entry was found; this is a descriptive rendering, not an attested term — flag for review if a more specific Danish Fluent convention emerges.                                                                                   |

### German (`de`)

German technical writing addresses the reader with the formal pronoun `Sie`,
not the informal *du*. This is the near-universal convention for developer
tools and operating-system UI: the Microsoft German style guide mandates `Sie`
(Höflichkeitsform) throughout software localization, and the GNOME German
translation team's own guidelines make the same rule explicit, giving "Möchten
`Sie` die Datei wirklich löschen?" as correct and the *du* form as wrong (see
[GNOME de/UebersetzungsRichtlinien](https://wiki.gnome.org/de(2f)UebersetzungsRichtlinien.html)).
GNOME also directs translators to prefer the infinitive or passive voice for
status and error messages rather than the imperative or first person, which
matches Netsuke's own house style of stating the condition rather than
addressing the user directly. Netsuke's German diagnostics should therefore use
passive or infinitival constructions, reserving `Sie` for the rare message that
instructs the reader directly.

German developer documentation keeps a small set of English technical nouns as
capitalized loan words rather than coining native equivalents: **Cache** (der
Cache), **Build** (der Build, alongside the verb *erstellen*, the term Visual
Studio's German UI uses for "build"), and, in Netsuke's own vocabulary, **
`Makro`** (an already-Germanized loan, not a fresh borrowing). Established
German build-tool prose instead translates the core Make/Ninja vocabulary
natively: **Ziel** (target), **Regel** (rule), and **Abhängigkeit**
(dependency) are the terms used by German-language Make tutorials and
documentation (for example the Rheinwerk *Linux-UNIX-Programmierung* openbook
and the German Wikibooks AVR-GCC guide), so Netsuke should follow suit rather
than borrow "Target" or "Rule" as loans. Because all German nouns, including
loan words, are capitalized, and because German favours long solid or
hyphenated compounds over multi-word phrases, terms such as "build graph" and
"dependency graph" become single compounds (**Build-Graph**,
**Abhängigkeitsgraph**) rather than two capitalized words as in English.

The main hazard is **Manifest**. Unlike some locales, German has no false
friend here in the everyday sense — *Manifest* already carries the shipping
document and political-manifesto senses, and major vendors (.NET, Android) use
the same word for a build/app manifest in their German documentation, so
"Manifest" is safe and idiomatic in Netsuke's diagnostics. The hazard lies
elsewhere: fluent-sounding calques for niche Ninja concepts — **order-only
dependency** and **phony target** — do not exist in any vendor glossary, so
translators may be tempted to invent something misleading; Netsuke's table
below marks these as constructed terms rather than attested ones. A second trap
is text expansion: German strings typically run 20-35% longer than English,
which matters for fixed-width status columns and progress labels. The worked
example below shows the passive, `Sie`-free register used for diagnostics, with
the `{ $path }` placeable preserved exactly:

```text
Manifest unter { $path } konnte nicht geladen werden.
```

Table 10: German terminology

| en-US                  | preferred                            | notes                                                                                                                                                                                                                                     |
| ---------------------- | ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | Manifest                             | Native German word; also used for build/app manifests in [.NET](https://learn.microsoft.com/de-de/) and Android German docs, so the technical sense is safe.                                                                              |
| target                 | Ziel                                 | Established Make/Ninja term; kept distinct from `Aktion` and Regel — see [Rheinwerk openbook](https://openbook.rheinwerk-verlag.de/linux_unix_programmierung/Kap17-000.htm).                                                              |
| action                 | `Aktion`                             | Netsuke-specific: an implicitly phony target. Distinct word from Ziel and Regel.                                                                                                                                                          |
| rule                   | Regel                                | The Ninja rule construct a target references; standard German Make terminology.                                                                                                                                                           |
| dependency             | Abhängigkeit                         | Standard term, confirmed in German Make documentation.                                                                                                                                                                                    |
| order-only dependency  | Ordnungsabhängigkeit                 | Constructed compound; no vendor-attested German term exists. Gloss on first use.                                                                                                                                                          |
| build (noun)           | Build                                | Established loan, der Build (capitalized as all German nouns are).                                                                                                                                                                        |
| build (verb)           | erstellen                            | Visual Studio German UI term for "build"; see [Microsoft Learn](https://learn.microsoft.com/de-de/cpp/build/cmake-projects-in-visual-studio?view=msvc-170).                                                                               |
| build graph            | Build-Graph                          | Loan+native compound, hyphenated per the GNOME rule for mixed-language compounds.                                                                                                                                                         |
| phony target           | Phony-Ziel                           | Loan+native compound; "phony" has no established German gloss in this sense.                                                                                                                                                              |
| artefact               | Artefakt                             | Confirmed usage in [Azure Pipelines German docs](https://learn.microsoft.com/de-de/azure/devops/pipelines/ecosystems/kubernetes/canary-demo?view=azure-devops).                                                                           |
| working directory      | Arbeitsverzeichnis                   | Confirmed in [Microsoft Learn German docs](https://learn.microsoft.com/de-de/rest/api/batchservice/jobs/create-job?view).                                                                                                                 |
| workspace root         | Stammverzeichnis des Arbeitsbereichs | Confirmed phrasing in [IBM](https://www.ibm.com/docs/de/developer-for-zos/17.0.x?topic=editing-setting-preferences) and [Microsoft CMake](https://learn.microsoft.com/de-de/cpp/build/cmakesettings-reference?view=msvc-170) German docs. |
| cache                  | Cache                                | Loan, der Cache; "Zwischenspeicher" exists but is not the developer-tool norm.                                                                                                                                                            |
| allowlist              | Zulassungsliste                      | Confirmed in [Microsoft Defender German docs](https://learn.microsoft.com/de-de/defender-office-365/tenant-allow-block-list-about).                                                                                                       |
| blocklist              | Sperrliste                           | Confirmed alongside Zulassungsliste in the same Microsoft source.                                                                                                                                                                         |
| template               | Vorlage                              | Confirmed in Azure Pipelines German docs (YAML "Vorlagen").                                                                                                                                                                               |
| macro                  | `Makro`                              | Established, already-Germanized loan; standard in German developer prose.                                                                                                                                                                 |
| environment variable   | Umgebungsvariable                    | Confirmed in Microsoft Learn German docs.                                                                                                                                                                                                 |
| exit status            | Beendigungsstatus                    | Confirmed in [PowerShell German docs](https://learn.microsoft.com/de-de/powershell/module/microsoft.powershell.core/about/about_scripts?view=powershell-7.6); "Exitcode" is a common informal loan alternative.                           |
| stage (pipeline stage) | Phase                                | Confirmed in Azure Pipelines German docs.                                                                                                                                                                                                 |
| locale                 | Gebietsschema                        | Confirmed Microsoft term, e.g. [Azure Communication Services docs](https://learn.microsoft.com/de-de/azure/communication-services/how-tos/ui-library-sdk/localization).                                                                   |
| placeable              | Platzhalter                          | Matches GNOME's own term for message placeholders (`strftime`-style).                                                                                                                                                                     |

### Greek (`el`)

Greek developer-facing text uses the second-person plural (πληθυντικός
ευγενείας, «εσείς») as the polite, distance-marking address form, rather than
the informal singular «εσύ». Microsoft's Greek localization guidance confirms
this: it directs translators to address the user with the formal plural and to
avoid the impersonal third-person «ο χρήστης» ("the user"), which reads as
institutional rather than direct
([Microsoft Greek style guide, summarized](https://chatscontrol.com/learn/style-guides/microsoft-el)).
Diagnostics and CLI help in Netsuke are read by developers and CI operators,
not consumers, but the same register applies: plain, professional, and free of
colloquialism, using «εσείς» forms for any second-person phrasing (for example,
imperative correction hints).

Several Netsuke terms are conventional English loans in Greek technical
writing, written in Latin script inline with the surrounding Greek sentence:
**build** (as a bare noun in casual developer speech, e.g. «build system»),
**stage** (pipeline stage, often left as «stage» in CI contexts), and
**pipeline**. Others have long-settled Greek calques instead and should not be
left as loans: cache is «κρυφή μνήμη», not "cache"; macro is «μακροεντολή»,
confirmed by Microsoft's own el-GR Excel documentation; and locale is «τοπικές
ρυθμίσεις», confirmed by Microsoft's el-GR Power BI documentation. Where
Netsuke's prose needs the formal build-tool sense (the section below), the
established Greek term «δόμηση» is preferred over the loan, following the
precedent set by GNOME's Greek translations (e.g. GNOME Builder's "build" →
«δόμηση»/«Αναλυτική δόμηση»); «κατασκευή» appears as an attested alternative in
some developer contexts and can be used in prose but not in the table to avoid
inconsistency.

Two hazards are worth flagging. First, «μανιφέστο» is a false friend: in
everyday Greek it means a political or artistic manifesto (as in «Κομμουνιστικό
Μανιφέστο»), not a build-tool descriptor file, so Netsuke's manifest is best
rendered with the English loan «manifest» in Latin script rather than
«μανιφέστο», which would mislead readers into expecting a declaration of
intent. Second, several table terms end in sigma (ς) — «στόχος», «κανόνας»,
«κατάλογος» — and Greek requires the final-sigma glyph (ς) rather than medial
sigma (σ) in that position; getting this wrong is a common script-level error
in machine-assisted translation. Greek also uses the semicolon (;) as its
question mark and does not use the Latin "?", which matters if Netsuke ever
emits Greek-locale interactive prompts.

Worked example, preserving the placeable exactly:

```text
Αποτυχία φόρτωσης του manifest στη διαδρομή { $path }.
```

Table 11: Greek terminology

| en-US                  | preferred               | notes                                                                                                                          |
| ---------------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| manifest               | manifest                | Kept as an English loan in Latin script; «μανιφέστο» is a false friend meaning political/artistic manifesto.                   |
| target                 | στόχος                  | Standard in Greek Make/build discussions; ends in final sigma (ς).                                                             |
| action                 | ενέργεια                | Distinct from στόχος and κανόνας per Netsuke's three-way distinction.                                                          |
| rule                   | κανόνας                 | Standard Greek Make terminology, e.g. «δεν υπάρχει κανόνας για να φτιαχτεί ο στόχος».                                          |
| dependency             | εξάρτηση                | Well established in Greek build/CS writing.                                                                                    |
| order-only dependency  | εξάρτηση μόνο σειράς    | Coined compound; no established Greek precedent found, flagged rather than asserted as standard.                               |
| build (noun)           | δόμηση                  | GNOME-established calque; «κατασκευή» attested as an informal alternative.                                                     |
| build (verb)           | δομώ / κάνω δόμηση      | Verb form of δόμηση, used consistently with the noun.                                                                          |
| build graph            | γράφος δόμησης          | Compositional from δόμηση + γράφος (graph).                                                                                    |
| phony target           | εικονικός στόχος        | Coined; no established Greek term for Ninja/Make "phony" was found.                                                            |
| artefact               | τεχνούργημα             | Attested in Greek DevOps/Agile texts and Wiktionary as the CI/CD "build `artifact`" sense.                                     |
| working directory      | κατάλογος εργασίας      | Confirmed by official Greek RStudio and general Linux command references.                                                      |
| workspace root         | ρίζα χώρου εργασίας     | Attested compound in Greek developer documentation ("χώρος εργασίας" = workspace).                                             |
| cache                  | κρυφή μνήμη             | Long-established Greek CS term; not a loan.                                                                                    |
| allowlist              | λίστα επιτρεπόμενων     | Standard modern Greek security-terminology pairing with blocklist.                                                             |
| blocklist              | λίστα αποκλεισμένων     | Attested alongside allowlist in Greek security trade press.                                                                    |
| template               | πρότυπο                 | Standard, unambiguous Greek term across technical domains.                                                                     |
| macro                  | μακροεντολή             | Confirmed by Microsoft's official el-GR Excel documentation.                                                                   |
| environment variable   | μεταβλητή περιβάλλοντος | Confirmed by official Greek Helm and FreeBSD Handbook translations.                                                            |
| exit status            | κατάσταση εξόδου        | Distinguished from exit code; attested in Greek developer forums and translations.                                             |
| stage (pipeline stage) | στάδιο                  | Standard Greek term for a pipeline phase; "pipeline" itself is commonly left as a loan.                                        |
| locale                 | τοπικές ρυθμίσεις       | Confirmed by Microsoft's official el-GR Power BI documentation.                                                                |
| placeable              | placeable               | Kept as an English loan in Latin script; no established Greek Fluent/l10n term was found, so a native calque was not invented. |

### English, United Kingdom (`en-GB`)

`en-GB` is a dialect variant of the `en-US` source, not a translation, so
register is identical: second-person "you", the same imperative and declarative
sentence shapes, no added formality. The
[Microsoft English (UK) style guide](https://aka.ms/english-uk-styleguide)
treats UK English as a light edit of US copy — punctuation, spelling, and a
handful of vocabulary items change, but voice, tone, and directness do not.
This matches Netsuke's house style of calm, plain diagnostic language.

Inspecting `locales/en-US/messages.ftl` against `locales/en-GB/messages.ftl`
shows the two catalogues are closer than a typical US/UK split: the `en-US`
source already spells "artefact" (not `artifact`), "colour" (in
`cli.flag.color.help` and `cli.subcommand.build.long_about`-adjacent copy),
"catalogue" (`cli.subcommand.help.long_about`), and `localisation` (file
header) throughout. The remaining `en-US` -> `en-GB` edits are narrower: a
handful of `-ize`/`-ization` spellings that had been missed (`serialize`,
`canonicalize`, `finalize`, `materialized`, `Synthesizing`, `Deserializing`)
become `-ise`/`-isation`, and "IO error" becomes "I/O error" for consistency
with house terminology. Note that the `en-US` file's own header comment claims
"Oxford spelling" while the corrected `en-GB` forms are the `-ise` variant, not
the `-ize` variant that
[Oxford spelling](https://en.wikipedia.org/wiki/Oxford_spelling) actually
prescribes; this is a pre-existing mislabel in the source repository, not an
`en-GB` policy choice, and a future editor should not "fix" `en-GB` back toward
`-ize` on the strength of that comment.

Loan words are unaffected by the dialect split: build, cache, template, macro,
glob, stdlib, dyndep, phony, allowlist, stage, and pipeline all stay as-is,
spelled identically in both catalogues. The only hazard specific to `en-GB` is
over-correction: message **keys** (`stdlib.path.action.canonicalize`,
`cli.validation.color.invalid`), the `--color` CLI flag name, JSON field names,
and any other machine-read identifier must keep their `en-US` spelling even
where the human-facing message text now reads "colour" or `canonicalise`.
Confusingly, `cli.validation.color.invalid`'s displayed text already read
"Invalid `color` policy" in `en-US` and was corrected to "Invalid colour
policy" in `en-GB` — the key `color.invalid` is untouched. A translator
skimming for `color` and blanket-replacing it with "colour" would corrupt the
flag name and break argument parsing.

The worked example needs no change from the source, because the source sentence
contains no affected spelling and no locale-specific vocabulary:

```text
Failed to load manifest at { $path }.
```

Table 12: English, United Kingdom terminology

| en-US                        | preferred                       | notes                                                                                                                                                                                                                                         |
| ---------------------------- | ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| serialize / serialization    | `serialise` / `serialisation`   | `-ize` -> `-ise`; see [Microsoft English (UK) style guide](https://aka.ms/english-uk-styleguide)                                                                                                                                              |
| canonicalize                 | `canonicalise`                  | `-ize` -> `-ise`; message **key** `stdlib.path.action.canonicalize` / `stdlib.which.canonicalize_failed` stays unchanged                                                                                                                      |
| finalize                     | `finalise`                      | `-ize` -> `-ise`                                                                                                                                                                                                                              |
| materialize / materialized   | `materialise` / `materialised`  | `-ize` -> `-ise`                                                                                                                                                                                                                              |
| synthesize / synthesizing    | `synthesise` / `synthesising`   | `-ize` -> `-ise`                                                                                                                                                                                                                              |
| deserialize / deserializing  | `deserialise` / `deserialising` | `-ize` -> `-ise`                                                                                                                                                                                                                              |
| IO error                     | I/O error                       | house wording fix, not a US/UK spelling pair; applied identically to both `stdlib.path.io.other` and `manifest.glob.unknown_io_error`                                                                                                         |
| `artifact`                   | artefact                        | already the spelling used in `en-US` source; no `en-GB` change needed                                                                                                                                                                         |
| `catalog`                    | catalogue                       | already the spelling used in `en-US` source; no `en-GB` change needed                                                                                                                                                                         |
| `color` (prose)              | colour                          | already the preferred `en-US` prose spelling in most strings; `cli.validation.color.invalid`'s message text was the one straggler, now corrected                                                                                              |
| — (identifiers, flags, keys) | unchanged                       | message keys (e.g. `color.invalid`, `action.canonicalize`), the `--color` flag, JSON field names, and Fluent placeables such as `{ $path }` are invariant across all locales — never respell them, even when the displayed text says "colour" |

### Spanish, Latin America (`es-419`)

Netsuke's `es-419` strings follow Microsoft's neutral-Spanish guidance for
error and status text: the impersonal form is preferred over repeated `tú`, and
third-person constructions are used only when naming the cause of a problem,
per the
[Microsoft Spanish (Neutral) style guide](https://aka.ms/spanish-neutral-styleguide),
§3.3 and §6. Where the reader must be addressed directly (help text, prompts),
Microsoft's modern voice favours informal `tú` over formal `usted` for most
products, and `ustedes` rather than `vosotros` for the plural, since `vosotros`
is a Spain-only form that Latin American readers do not use. Diagnostics
therefore read as impersonal statements of fact ("No se pudo cargar…"), not as
messages spoken to the user.

Several terms stay as English loans in Latin American developer writing:
`build` is naturalised as the noun `compilación` and verb `compilar` (see the
table note on this tension), but `caché` is the RAE-sanctioned Spanish spelling
(feminine, "la caché") rather than an unadapted loan, per the
[RAE Libro de estilo](https://www.rae.es/libro-estilo-lengua-espa%C3%B1ola/c).
`Template` and `stage` take native equivalents (`plantilla`, `etapa`) rather
than loans, confirmed by es-419 vendor documentation such as
[Bazel](https://bazel.build/remote/output-directories?hl=es-419) and
[GitLab CI](https://www.jpgboost.com/es-419/blog/automatizar-compresion-imagenes-ci).
Product and syntax names (Netsuke, Ninja, Fluent, Jinja, YAML) remain
unchanged in Latin script; no digraphs, diacritics, or spacing rules in
`es-419` create ambiguity around these identifiers.

The main hazard is `manifest` → `manifiesto`: everyday Spanish uses
`manifiesto` for a political manifesto, and as an adjective it means "evident,
patent." The computing sense (Android's `AndroidManifest.xml`, Docker
manifests) is nonetheless the established translation in Spanish technical
writing, so Netsuke keeps `manifiesto`, but readers unfamiliar with the
software sense may briefly parse it as the political meaning; the surrounding
sentence should keep `manifiesto` adjacent to a file path or `.yaml` extension
to disambiguate. A second hazard is `dependencia`: it is correct for Netsuke's
dependency edges, but GNU Make's own Spanish translation now prefers
`prerrequisito`
([GNU Make manual, ch. 4](https://www.ecoop.net/coop/translated/GNUMake4.4/ch04.es.html)).
Netsuke keeps `dependencia` for consistency with general Spanish developer
usage (npm, pip), a deliberate rather than automatic choice. Compared with
`es-ES`, `es-419` prefers `computadora` over `ordenador` and treats `archivo`
(not the Spain-leaning `fichero`) as the default word for "file" — differences
large enough in everyday vocabulary to justify a separate locale rather than
folding Latin American Spanish into `es-ES`.

Worked example:

```text
No se pudo cargar el manifiesto en { $path }.
```

Table 13: Spanish, Latin America terminology

| en-US                  | preferred                   | notes                                                                                                                                                                      |
| ---------------------- | --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | `manifiesto`                | False friend: also "political manifesto" / "evident". Standard IT sense per Android/Docker usage.                                                                          |
| target                 | objetivo                    | Confirmed by [GNU Make es manual](https://www.ecoop.net/coop/translated/GNUMake4.4/ch04.es.html): "objetivo (target)".                                                     |
| action                 | acción                      | Netsuke-specific sense (implicitly phony target); keep distinct from "objetivo" and "regla".                                                                               |
| rule                   | regla                       | Confirmed by [GNU Make es manual](https://www.ecoop.net/coop/translated/GNUMake4.4/ch04.es.html): "regla (rule)".                                                          |
| dependency             | dependencia                 | Widely used in es-419 dev docs (npm, pip); GNU Make now prefers "prerrequisito", see hazards above.                                                                        |
| order-only dependency  | dependencia de solo orden   | Calque; no established es-419 precedent found, meaning made explicit in surrounding prose on first use.                                                                    |
| build (noun)           | compilación                 | Established even though Netsuke also links/copies files, not only compiles; see prose.                                                                                     |
| build (verb)           | compilar                    | Consistent with the noun form.                                                                                                                                             |
| build graph            | grafo de compilación        | Calque from established components ("grafo", "compilación"); not independently attested.                                                                                   |
| phony target           | objetivo ficticio           | Confirmed by [GNU Make es manual](https://www.ecoop.net/coop/translated/GNUMake4.4/ch04.es.html) and [Learn Make in Y minutes, es](https://learnxinyminutes.com/es/make/). |
| artefact               | artefacto (de compilación)  | Confirmed by [Google Cloud es-419 docs](https://docs.cloud.google.com/build/docs/building/store-artifacts-in-cloud-storage?hl=es-419).                                     |
| working directory      | directorio de trabajo       | Standard in es-419 shell/Linux documentation.                                                                                                                              |
| workspace root         | raíz del espacio de trabajo | Confirmed by [Bazel es-419 docs](https://bazel.build/remote/output-directories?hl=es-419).                                                                                 |
| cache                  | caché (la caché)            | RAE-sanctioned spelling with accent, feminine noun; see [RAE Libro de estilo](https://www.rae.es/libro-estilo-lengua-espa%C3%B1ola/c).                                     |
| allowlist              | lista de permitidos         | Confirmed by [Microsoft es-es Defender docs](https://learn.microsoft.com/es-es/defender-office-365/tenant-allow-block-list-about).                                         |
| blocklist              | lista de bloqueados         | Confirmed by [Microsoft es-es Defender docs](https://learn.microsoft.com/es-es/defender-office-365/tenant-allow-block-list-about).                                         |
| template               | plantilla                   | Standard, unambiguous.                                                                                                                                                     |
| macro                  | macro (la macro)            | Loan noun, feminine gender in Spanish computing usage.                                                                                                                     |
| environment variable   | variable de entorno         | Standard, unambiguous.                                                                                                                                                     |
| exit status            | estado de salida            | Standard in es-419 shell/Bash documentation; "código de salida" is an accepted synonym.                                                                                    |
| stage (pipeline stage) | etapa                       | Confirmed by es-419 [CI/CD usage](https://www.jpgboost.com/es-419/blog/automatizar-compresion-imagenes-ci).                                                                |
| locale                 | configuración regional      | Standard Microsoft term; see [style guide summary](http://www.monicamartinez.es/resumen_guia_ms.pdf).                                                                      |
| placeable              | marcador de posición        | Descriptive choice; no established es-419 gloss for Fluent's specific term was found.                                                                                      |

### Spanish, Spain (`es-ES`)

Netsuke's Spanish (Spain) text uses the formal, impersonal register that
Spain's established open-source localization teams apply to technical and
system software. KDE's Spanish team states that "por lo general, trataremos al
usuario de usted" but adds that the literal word "usted" should almost always
be avoided, favouring impersonal or third-person phrasing instead
([KDE Spanish general rules](https://l10n.kde.org/teams/es/normas_generales.php)).
Ubuntu's Spanish team gives the same instruction independently: "el
tratamiento adecuado para estos textos es la tercera persona del singular
(usted)… sin embargo no debe utilizarse la palabra «usted»"
([Ubuntu Spanish style guide](https://wiki.ubuntu.com/UbuntuSpanishTranslators/Estilo)).
Microsoft's current Spain style guide recommends the informal "tú" for its
consumer "Microsoft voice," but explicitly scopes that choice to marketing and
conversational UI copy, noting that "formal, informative, and factual" tone
still applies to technical texts
([Microsoft Spanish (Spain) style guide](https://aka.ms/spanish-spain-styleguide)).
Netsuke's diagnostics and CLI help are technical, developer-facing text, so
this locale follows the KDE/Ubuntu convention: third-person, impersonal phrasing
(`se han encontrado`, `no se puede`) with no visible "tú" or "usted,"
diverging from Microsoft's newer consumer-voice default.

Several Netsuke terms stay as English loan words in Spain's technical register:
`build` (as a stage name, e.g. `build system`), `caché` (Hispanicised spelling,
not "cache"), `pipeline`, `stdlib`, and `glob` are commonly left unlocalized or
lightly adapted, following KDE's practice of keeping unavoidable loans in
italics or quotes rather than inventing calques
([KDE general rules](https://l10n.kde.org/teams/es/normas_generales.php)).
`macro`, `plantilla` (template), `caché`, and `directorio de trabajo` (working
directory) are established native terms and are not loans. `file` is translated
as "archivo," not "fichero": the KDE glossary states this explicitly ("no
traducir como «fichero»"), and Microsoft's terminology also uses "archivo," so
this section follows that pan-Hispanic consensus rather than the older
Peninsular-only "fichero" convention
([KDE glossary](https://l10n.kde.org/teams/es/glosario.php)).

Hazards: `manifiesto` is a false friend for `manifest` in this context — in
everyday Spanish it means a political manifesto or public declaration, not a
build input file, so Netsuke keeps `manifiesto` only with clear technical
qualification (e.g. "`manifiesto` de compilación") to avoid the political sense.
`target`, `action`, and `rule` must stay distinct: this table uses "objetivo"
for `target`, following the long-established GNU Make/CMake Spanish translation
tradition and systemd's own "objetivo" for `.target` units, while "destino" is
reserved for filesystem destinations elsewhere in the documentation, avoiding a
collision between the two senses. "Compilar" is a false friend for Netsuke's
`build (verb)`: it implies strict source compilation, but Netsuke's build step
also runs arbitrary actions, so "generar" is preferred as the neutral build
verb, with "compilar" reserved for compiler invocations specifically.

Worked example, preserving the placeable exactly:

```text
No se pudo cargar el manifiesto en { $path }.
```

Table 14: Spanish, Spain terminology

| en-US                  | preferred                   | notes                                                                                                                                                                                      |
| ---------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| manifest               | `manifiesto`                | qualify as "`manifiesto` de compilación" where ambiguity with political sense is possible                                                                                                  |
| target                 | objetivo                    | Make/CMake/systemd convention; see [KDE glossary](https://l10n.kde.org/teams/es/glosario.php) ("target partition" → "partición de destino" shows destino is reserved for filesystem sense) |
| action                 | acción                      | implicitly phony target; keep distinct from "objetivo"                                                                                                                                     |
| rule                   | regla                       | Ninja construct; matches Make/CMake Spanish usage                                                                                                                                          |
| dependency             | dependencia                 | standard term, confirmed in Make Spanish tutorials                                                                                                                                         |
| order-only dependency  | dependencia de solo orden   | calque of the Ninja/Make term; no established loan exists                                                                                                                                  |
| build (noun)           | compilación                 | when referring to the overall process/output                                                                                                                                               |
| build (verb)           | generar                     | avoids implying pure source compilation; "compilar" reserved for compiler steps                                                                                                            |
| build graph            | grafo de compilación        | compositional, follows "grafo" for graph in CS Spanish usage                                                                                                                               |
| phony target           | objetivo ficticio           | "ficticio" is the established Make-translation qualifier for phony                                                                                                                         |
| artefact               | artefacto                   | standard software-engineering loan, now in general technical use                                                                                                                           |
| working directory      | directorio de trabajo       | CLI context uses "directorio"; GUI contexts use "carpeta" per [KDE glossary](https://l10n.kde.org/teams/es/glosario.php)                                                                   |
| workspace root         | raíz del espacio de trabajo | compositional; "espacio de trabajo" is the standard rendering of workspace                                                                                                                 |
| cache                  | caché                       | Hispanicised spelling per [KDE glossary](https://l10n.kde.org/teams/es/glosario.php) ("memoria caché")                                                                                     |
| allowlist              | lista de permitidos         | avoids the retired "whitelist" calque                                                                                                                                                      |
| blocklist              | lista de bloqueados         | parallel construction to allowlist                                                                                                                                                         |
| template               | plantilla                   | established native term, no loan needed                                                                                                                                                    |
| macro                  | macro                       | invariant loan, feminine in Spanish computing usage                                                                                                                                        |
| environment variable   | variable de entorno         | standard term per [KDE glossary](https://l10n.kde.org/teams/es/glosario.php) ("entorno")                                                                                                   |
| exit status            | estado de salida            | compositional, widely used in shell/CLI documentation                                                                                                                                      |
| stage (pipeline stage) | etapa                       | preferred over `fase` for a pipeline step in CI/CD Spanish usage                                                                                                                           |
| locale                 | configuración regional      | Microsoft/KDE convention; "locale" as a bare loan is also seen but this is the formal preferred term                                                                                       |
| placeable              | marcador de posición        | Fluent/ICU-message convention; not to be confused with "marcador" (bookmark) alone                                                                                                         |

### Persian (`fa`)

Netsuke's Persian output should address the reader with **شما** (*shomâ*), the
plural second-person pronoun taking plural verb agreement, even when addressing
a single reader. Persian has no French-style *tu/vous* split: **تو** (*to*) is
reserved for intimate, family, or child-directed speech and never appears in
software strings, so شما is not a "formal" choice among alternatives but the
only pronoun software can use. The
[Microsoft Persian localization style guide](https://aka.ms/persian-styleguide)
confirms this in its guidance to "address the user as you" (§2.1.1) and notes a
distinct formal/informal *tone* axis that is separate from pronoun choice: it
recommends an informal, friendly register for word choice and sentence
structure while still using شما throughout (§4.1.9). The established
open-source Persian translation of the
[Git book](https://git-scm.com/book/fa/v2) follows the same pattern,
consistently pairing شما with plural verb forms ("شاخهٔ کاری شما پاک است").

Netsuke's technical nouns split between native Persian terms and English loans.
**Cache** and **macro** stay as loans (**کش**, **ماکرو**) in everyday Persian
technical writing, and **manifest** is conventionally transliterated
(**مانیفست**) in Persian Android and developer documentation rather than
calqued. **Target**, **rule**, **dependency**, **template**, and **environment
variable**, by contrast, have established native equivalents in Persian
build-tool, package-manager, and Microsoft documentation (see the table).
Persian technical prose also mixes Latin-script loans freely mid-sentence (for
example DevOps writing that keeps `Artifact` untranslated); Netsuke's
diagnostics favour the native descriptive gloss instead, for consistency with
the rest of the glossary.

Three hazards need attention. First, **manifest**: the bare native word
**بیانیه** overwhelmingly reads as "statement" or "political manifesto," and
**اظهارنامه** reads as a customs or legal declaration — both are false friends,
so Netsuke should keep the transliterated loan مانیفست, as Persian Android
documentation does. Second, **target**, **action**, and **rule** must stay
three visibly distinct words (هدف / کنش / قاعده); **عمل** ("action") is avoided
for the second because it is heavily overloaded in everyday Persian (general
"deed," medical "operation"), and **قانون** ("rule") is avoided for the third
because it specifically means statute or law. Third, two mechanical hazards
accompany the script itself: Persian is right-to-left, so the translator
guide's direction-mark rule (prefixing a value with U+200F when its first
strong character would otherwise set left-to-right direction) applies to every
Netsuke string that opens with a placeable or Latin token; and Persian
compounds use ZWNJ, نیم‌فاصله (U+200C), to join parts such as می‌خواهد or plural
پرونده‌ها — tooling that normalizes whitespace, diffs Fluent source, or
round-trips through non-Unicode-aware editors can silently drop or replace ZWNJ
with a plain space, corrupting the compound. Translators should also watch the
ezafe (kasre) construction, the vowel linking a noun to its modifier, which
Persian requires when chaining Netsuke's compound terms (for example "workspace
root" needs two ezafe links: ریشهٔ فضای کاری).

Worked example, preserving the `{ $path }` placeable exactly:

```text
بارگذاری مانیفست در { $path } ناموفق بود.
```

Table 15: Persian terminology

| en-US                  | preferred                                       | notes                                                                                                                                                                                                                                                                                                                             |
| ---------------------- | ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | مانیفست (mânifest)                              | Transliterated loan, not native **بیانیه** (political manifesto) or **اظهارنامه** (customs/legal declaration). Matches Persian Android developer material (e.g. [tahlildadeh.com Android training](https://www.tahlildadeh.com/files/amoozesh-android-full_(www.tahlildadeh.com).pdf)), which uses مانیفست for `AndroidManifest`. |
| target                 | هدف (hadaf)                                     | Build output or named build entry; kept distinct from کنش and قاعده. Standard in Persian discussion of Makefile targets.                                                                                                                                                                                                          |
| action                 | کنش (konesh)                                    | Netsuke-specific: an implicitly phony target. Chosen over **عمل**, which collides with the everyday/medical sense ("deed," "surgical operation").                                                                                                                                                                                 |
| rule                   | قاعده (qâ'`ede`)                                | The Ninja rule construct a target references. Chosen over **قانون**, which specifically means statute or law.                                                                                                                                                                                                                     |
| dependency             | وابستگی (vâbastegi)                             | Standard in Persian package-manager and DI writing, e.g. "تزریق وابستگی" for dependency injection.                                                                                                                                                                                                                                |
| order-only dependency  | وابستگی فقط‌ترتیبی                               | Constructed compound; no vendor-attested Persian term exists for this Ninja-specific concept. Gloss on first use.                                                                                                                                                                                                                 |
| build (noun)           | ساخت (sâkht)                                    | The build process/output as a noun.                                                                                                                                                                                                                                                                                               |
| build (verb)           | ساختن (sâkhtan)                                 | Infinitive form; disambiguate from the noun by sentence context.                                                                                                                                                                                                                                                                  |
| build graph            | گراف ساخت (gerâf-e sâkht)                       | "گراف" is a standard loan for "graph" in Persian computer science.                                                                                                                                                                                                                                                                |
| phony target           | هدف مجازی (hadaf-e majâzi)                      | "مجازی" (virtual/fake) is the conventional Persian gloss for Make/Ninja `.PHONY`; constructed, no single fixed vendor term.                                                                                                                                                                                                       |
| artefact               | برون‌داد ساخت (borun-dâd-e sâkht)                | Native descriptive gloss ("build output"). English loan **آرتیفکت** circulates in Persian DevOps/QA prose (e.g. [testrail.ir](https://testrail.ir/2100/qa-resume-job-evidence-parse-privacy-guide/)) but is avoided here for diagnostic clarity.                                                                                  |
| working directory      | شاخهٔ کاری (shâxe-ye kâri)                       | Matches the official Persian translation of the [Git book](https://git-scm.com/book/fa/v2); **پوشهٔ کاری** is an attested synonym.                                                                                                                                                                                                 |
| workspace root         | ریشهٔ فضای کاری (rishe-ye fazâ-ye kâri)          | Constructed, composing the attested **فضای کاری** (workspace) with **ریشه** (root) via two ezafe links.                                                                                                                                                                                                                           |
| cache                  | کش (kesh)                                       | Loan; ubiquitous in everyday Persian tech usage (e.g. "پاک کردن کش مرورگر", clear browser cache). Native **حافظهٔ نهان** is attested in specialist dictionaries but rare in UI strings.                                                                                                                                            |
| allowlist              | فهرست مجاز (fehrest-e mojâz)                    | Descriptive compound, parallel to blocklist.                                                                                                                                                                                                                                                                                      |
| blocklist              | فهرست مسدود (fehrest-e masdud)                  | Descriptive compound, parallel to allowlist.                                                                                                                                                                                                                                                                                      |
| template               | قالب (qâleb)                                    | Standard across Persian software localization, e.g. the [NVDA Persian user guide](https://download.nvaccess.org/releases/2022.4/documentation/fa/userGuide.html).                                                                                                                                                                 |
| macro                  | ماکرو (mâkro)                                   | Established transliterated loan, standard in Persian Office/scripting documentation.                                                                                                                                                                                                                                              |
| environment variable   | متغیر محیطی (moteqayer-e mohiti)                | Confirmed across multiple independent Persian developer sources (e.g. [Liara's Python install guide](https://liara.ir/blog/نصب-پایتون-روی-ویندوز-installing-python-on-windows/), [Ferdowsi Cloud](https://ferdowsi.cloud/blog/install-python/)).                                                                                  |
| exit status            | وضعیت خروج (vaz'iyat-e khoruj)                  | Descriptive, standard in Persian CLI/DevOps writing.                                                                                                                                                                                                                                                                              |
| stage (pipeline stage) | مرحله (marhale)                                 | Standard Persian term for a pipeline/process phase.                                                                                                                                                                                                                                                                               |
| locale                 | شناسهٔ زبان و منطقه (shenâse-ye zabân o mantaqe) | Chosen over the loan **لوکال**, which collides with "local" (as in "لوکال هاست", localhost) in everyday Persian developer usage — a false-friend risk. Persian OS settings commonly label the underlying concept "تنظیمات محلی" (locale settings).                                                                                |
| placeable              | جایگذاردنی (jâygozârdani)                       | Coined calque (verb stem گذاردن + potential suffix -نی, as in خواندنی "readable"); no established Persian Fluent or CAT-tool precedent was found during research.                                                                                                                                                                 |

### Finnish (`fi`)

Netsuke's Finnish text should avoid addressing the reader directly. The
[Sailfish OS Finnish style guide](https://docs.sailfishos.org/Develop/L10n/Style_Guides/Finnish/)
recommends informal, active-voice Finnish for UI copy, and it explicitly warns
that the Finnish passive implies a human actor, so it should not be used to
describe the actions of a non-human system. Netsuke's text is almost entirely
diagnostics and status reports about what the tool did, not instructions to the
user, so the natural register is an impersonal, third-person statement of fact
— the pattern the guide itself models with "Toiminto epäonnistui" ("The action
failed") rather than a passive or a "sinä"-form imperative. CLI help text that
does address the reader (flag descriptions, prompts) may use the informal
second person, consistent with the guide's stated preference for informal
Finnish over the formal "Te" form.

Few Netsuke terms survive as bare English loans in Finnish technical writing.
Official Microsoft Finnish localization renders build, cache, template,
allowlist, stage, and pipeline as native compounds — koonti, välimuisti,
mallipohja, sallittujen luettelo, vaihe, and putki — rather than as loans (see
the
[Microsoft Fabric CI/CD documentation](https://learn.microsoft.com/fi-fi/fabric/cicd/manage-deployment)
and the
[Azure Pipelines release docs](https://learn.microsoft.com/fi-fi/azure/devops/pipelines/release/releases?view=azure-devops)).
This matches the seed hypothesis that Finnish agglutination discourages loans:
a term such as *build* would need case endings (buildin, buildissa) that read
awkwardly in running prose, so short, easily inflected native words win out.
The main exception is macro, kept as the established loan `makro` (makron,
makroa), which inflects cleanly. Identifiers such as Ninja, Fluent, dyndep, and
stdout stay in Latin script and take Finnish case endings directly, with no
apostrophe (Ninjan, dyndepin).

The clearest false friend is manifest: Finnish manifesti almost always means a
political manifesto (kommunistinen manifesti), so a diagnostic naming the
Netsuke manifest file needs enough surrounding context — file, path, or
extension — that a reader does not parse it as a political text. Target is a
second trap: casual Finnish sometimes calques it as rakennuskohde, but that
word means a construction site or building project (confirmed across multiple
Finnish construction-industry sources), not a build target, so Netsuke uses the
neutral kohde instead. Pipeline is a false-loan risk in the other direction:
English-influenced developer slang sometimes keeps pipeline untranslated, but
official Microsoft localization always renders it putki, so stage-related text
should not import the English word. The worked example keeps the register
impersonal, following the guide's own "epäonnistui" pattern:

```text
Manifestin lataaminen polusta { $path } epäonnistui.
```

Table 16: Finnish terminology

| en-US                  | preferred               | notes                                                                                                                                                                                                                                                                             |
| ---------------------- | ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifesti               | Loanword; false friend = political manifesto, see hazards above.                                                                                                                                                                                                                  |
| target                 | kohde                   | Neutral; avoid rakennuskohde ("construction site").                                                                                                                                                                                                                               |
| action                 | toiminto                | Microsoft UI convention, e.g. [Power Pages fi-fi docs](https://learn.microsoft.com/fi-fi/power-pages/configure/create-code-site-using-claude-code) ("Toiminnot").                                                                                                                 |
| rule                   | sääntö                  | Standard (ESLint-säännöt, palomuurisäännöt).                                                                                                                                                                                                                                      |
| dependency             | riippuvuus              | [lokalisointi.fi sanakirja](https://lokalisointi.fi/sanakirja/).                                                                                                                                                                                                                  |
| order-only dependency  | järjestysriippuvuus     | Proposed calque; no attested source found.                                                                                                                                                                                                                                        |
| build (noun)           | koonti                  | [Microsoft Fabric CI/CD fi-fi](https://learn.microsoft.com/fi-fi/fabric/cicd/manage-deployment): koontiputki, koontiympäristö.                                                                                                                                                    |
| build (verb)           | koota                   | Infinitive matching koonti.                                                                                                                                                                                                                                                       |
| build graph            | koontigraafi            | Proposed calque on koonti; not independently attested.                                                                                                                                                                                                                            |
| phony target           | näennäiskohde           | Proposed calque (näennäis- = "pseudo-"); not independently attested.                                                                                                                                                                                                              |
| artefact               | artefakti               | Attested in Finnish CI/CD theses ([Theseus](https://www.theseus.fi/bitstream/handle/10024/781312/Opinnaytetyo_Pajari_Taavi.pdf?sequence=2), [JYX](https://jyx.jyu.fi/bitstreams/f6f41a50-b7dd-4100-9df7-38c0208b3f24/download)).                                                  |
| working directory      | työhakemisto            | Standard CLI term, e.g. [FLOSS Manuals: Komentorivin perusteet](https://ia802800.us.archive.org/10/items/SuomenkielinenFlossManuals/komentorivin-perusteet.pdf).                                                                                                                  |
| workspace root         | työtilan juurihakemisto | työtila per [Microsoft AL Tool fi-fi docs](https://learn.microsoft.com/fi-fi/dynamics365/release-plan/2026wave1/smb/dynamics365-business-central/discover-new-altool-features-al-mcp-server-command-line-agentic-al-coding-commands-working-workspaces) ("Työtilan kääntäminen"). |
| cache                  | välimuisti              | [lokalisointi.fi sanakirja](https://lokalisointi.fi/sanakirja/).                                                                                                                                                                                                                  |
| allowlist              | sallittujen luettelo    | [Kaspersky Finland](https://www.kaspersky.fi/partners/allowlist-program).                                                                                                                                                                                                         |
| blocklist              | estettyjen luettelo     | Parallel to allowlist; cf. official "estolista" in [Traficom regulation](https://www.finlex.fi/api/media/authority-regulation/537119/media/02_Maarays_teletoiminnan_tietoturvasta_perustelumuistio.pdf).                                                                          |
| template               | mallipohja              | [lokalisointi.fi sanakirja](https://lokalisointi.fi/sanakirja/) also lists malli, malline, pohja.                                                                                                                                                                                 |
| macro                  | `makro`                 | Established loanword.                                                                                                                                                                                                                                                             |
| environment variable   | ympäristömuuttuja       | Widely attested, e.g. [Linux From Scratch fi (Theseus)](https://www.theseus.fi/bitstream/handle/10024/135233/LFS-final.pdf?sequence=1&isAllowed=y).                                                                                                                               |
| exit status            | poistumiskoodi          | [FLOSS Manuals: Komentorivin perusteet](https://ia802800.us.archive.org/10/items/SuomenkielinenFlossManuals/komentorivin-perusteet.pdf); "paluukoodi" is a common synonym.                                                                                                        |
| stage (pipeline stage) | vaihe                   | Attested in Finnish CI/CD pipeline documentation and theses.                                                                                                                                                                                                                      |
| locale                 | lokaali                 | [linux.fi](https://linux.fi/wiki/Locale) and [lokalisointi.fi sanakirja](https://lokalisointi.fi/sanakirja/) agree.                                                                                                                                                               |
| placeable              | sijoiteltava            | Proposed; no established Fluent-specific Finnish term found — verify before shipping.                                                                                                                                                                                             |

### French (`fr`)

Netsuke's French locale addresses the reader with the formal **vous** form
throughout, matching Microsoft's French (France) style guide, which specifies
"vous" (masculine singular agreement) for product UI and reserves "tu" for
narrow, explicitly casual contexts such as example conversational prompts. This
matches established practice for developer tooling: Kubernetes' French
documentation and GNOME's French glossary both use "vous" in instructions and
keep sentences impersonal where possible. Sources:

- [Microsoft French style guide](https://learn.microsoft.com/fr-fr/globalization/reference/microsoft-style-guides)
- [French style guide summary](https://chatscontrol.com/learn/style-guides/microsoft-fr-fr)
- [Kubernetes fr docs](https://kubernetes.io/fr/docs/concepts/overview/working-with-objects/)
- [GNOME French glossary](https://wiki.gnome.org/GnomeFr(2f)Glossaire.html)

Several Netsuke terms stay as English loans in French technical writing:
"build" is widely used as an invariable masculine loan in developer contexts
even though Microsoft's own Visual Studio UI prefers the calque "génération"
("Annuler la build" appears alongside "Annuler la génération" in Microsoft's
own visual-language dictionary), so Netsuke follows the loan-word convention
favoured by open-source and CLI tooling rather than the more bureaucratic
Microsoft Learn calque
([Microsoft Visual Studio dictionary](https://learn.microsoft.com/fr-fr/visualstudio/extensibility/ux-guidelines/visual-language-dictionary-for-visual-studio)).
"Cache", "template" (as "modèle"), and "macro" also follow local convention:
"cache" stays an unmarked masculine loan, "macro" stays as-is, and "modèle" is
the established calque for "template" in French developer glossaries (GNOME,
traduc.org). "Allowlist" and "blocklist" are calqued rather than loaned,
following current Microsoft and Mozilla practice, which replaced the older
"liste blanche" / "liste noire" pair with `liste d'autorisation` and
`liste de blocage`. Sources:

- [Mozilla Transvision](https://transvision.mozfr.org/consistency/)
- [Microsoft inclusive-terminology discussion](https://www.developpez.com/actu/306328/Black-Lives-Matter-des-developpeurs-souhaitent-debarrasser-le-monde-informatique-de-termes-juges-racistes-ou-violents-comme-whitelist-blacklist-master-slave-et-kill/)

Two hazards apply. First, "manifeste" carries a political-pamphlet sense in
everyday French, but it is already the settled technical term for a build or
deployment descriptor: Kubernetes' French docs use "manifeste" for YAML/JSON
object descriptions without qualification, so Netsuke follows suit rather than
inventing an unfamiliar calque
([Kubernetes objects, fr](https://kubernetes.io/fr/docs/concepts/overview/working-with-objects/)).
Second, French typography treats punctuation marks differently depending on
whether they are "simple" or "double": the colon (`:`) takes a full no-break
space (` `, U+00A0) before it, while the semicolon, exclamation mark, and
question mark take a narrower no-break space (` `, U+202F) where the font
supports it, falling back to the ordinary no-break space otherwise. This is a
mechanical hazard for any message that ends in a colon, such as
`cli.help.actions_heading = Actions:`, which must render as `Actions :` with a
genuine no-break space before the colon, not a translator-typed ordinary space
that a line-wrap could split. Sources:

- [Lexique des règles typographiques, via Wikipédia](https://fr.wikipedia.org/wiki/Espace_fine_ins%C3%A9cable)
- [OQLF spacing table](https://vitrinelinguistique.oqlf.gouv.qc.ca/22039/la-typographie/espacement/espacement-avant-et-apres-les-signes-de-ponctuation-et-les-symboles)
Guillemets (`« »`) likewise take a no-break space immediately inside each mark,
which affects any quoted value embedded in a diagnostic.

Worked example, preserving the placeable exactly:

```text
Échec du chargement du manifeste à { $path }.
```

Table 17: French terminology

| en-US                  | preferred                         | notes                                                                                                                                                                                                                                                               |
| ---------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifeste                         | Political sense exists but is not a practical hazard; established in [Kubernetes fr docs](https://kubernetes.io/fr/docs/concepts/overview/working-with-objects/).                                                                                                   |
| target                 | cible                             | Distinct from action/rule; per [traduc.org glossary](https://glossaire.traduc.org/download.php?f=csv) and [GNOME glossary](https://wiki.gnome.org/GnomeFr(2f)Glossaire.html).                                                                                       |
| action                 | action                            | English/French cognate, used as-is; kept distinct from "cible" and "règle".                                                                                                                                                                                         |
| rule                   | règle                             | The Ninja construct a target references; standard technical calque.                                                                                                                                                                                                 |
| dependency             | dépendance                        | Confirmed in [GNOME glossary](https://wiki.gnome.org/GnomeFr(2f)Glossaire.html) and [traduc.org](https://glossaire.traduc.org/download.php?f=csv).                                                                                                                  |
| order-only dependency  | dépendance sans ordre `implicite` | Descriptive calque; no fixed term found, phrase to disambiguate from a plain dependency.                                                                                                                                                                            |
| build (noun)           | build                             | Loan word, invariable masculine; common in CLI/dev-tool French even though Microsoft's own UI prefers "génération" ([VS dictionary](https://learn.microsoft.com/fr-fr/visualstudio/extensibility/ux-guidelines/visual-language-dictionary-for-visual-studio)).      |
| build (verb)           | compiler / générer                | Use "compiler" for source-to-artefact steps, "générer" for plan/graph creation; both attested in Microsoft fr-fr docs.                                                                                                                                              |
| build graph            | graphe de compilation             | Compound calque; no single fixed term, follows "graphe de dépendances" pattern used in French CS writing.                                                                                                                                                           |
| phony target           | cible fictive                     | Attested in translated technical literature (e.g. Eckel, *Penser en C++*, fr); "cible factice" also seen but less consistent.                                                                                                                                       |
| artefact               | artefact                          | Standard French spelling (no "c"); used unchanged in French dev docs.                                                                                                                                                                                               |
| working directory      | répertoire de travail             | Standard calque, confirmed in SUSE/Novell fr documentation.                                                                                                                                                                                                         |
| workspace root         | racine de l'`espace` de travail   | Compositional calque; consistent with "`espace` de travail" for workspace.                                                                                                                                                                                          |
| cache                  | cache                             | Masculine loan word, invariable; unmarked in French technical writing.                                                                                                                                                                                              |
| allowlist              | liste d'`autorisation`            | Current Microsoft/Mozilla term, replacing "liste blanche" ([Transvision](https://transvision.mozfr.org/consistency/)).                                                                                                                                              |
| blocklist              | liste de blocage                  | Current term, replacing "liste noire" ([Transvision](https://transvision.mozfr.org/consistency/); [Google Cloud fr docs](https://docs.cloud.google.com/contact-center/ccai-platform/docs/consumer-management?hl=fr)).                                               |
| template               | modèle                            | Established calque in GNOME/traduc.org glossaries; not loaned.                                                                                                                                                                                                      |
| macro                  | macro                             | Loan word, invariable; standard in French technical writing.                                                                                                                                                                                                        |
| environment variable   | variable d'`environnement`        | Standard, fixed calque throughout French developer documentation.                                                                                                                                                                                                   |
| exit status            | code de sortie                    | Standard calque ("code" not "statut"); confirmed in [traduc.org glossary](https://glossaire.traduc.org/download.php?f=csv) and [Starship fr docs](https://starship.rs/fr-fr/config/); older Sun-derived glossaries also list "état de sortie".                      |
| stage (pipeline stage) | étape                             | Generic calque; "stage" as a loan is CI-specific (e.g. GitLab CI) and less general than "étape".                                                                                                                                                                    |
| locale                 | paramètres régionaux / locale     | "Paramètres régionaux" is the formal Microsoft term; "locale" is the common loan in developer contexts — use "locale" (lowercase, invariable) for Netsuke's technical audience.                                                                                     |
| placeable              | placeable / `espace` réservé      | Fluent's French tooling has no single fixed term in wide use; "placeable" is kept as a loan in Mozilla/Weblate contexts ([Weblate fr docs](https://docs.weblate.org/fr/latest/user/translating.html)) — verify against final Fluent-fr terminology before shipping. |

### Scottish Gaelic (`gd`)

Netsuke's Scottish Gaelic strings should address the reader with the informal
second-person singular *thu*, not the polite plural *sibh*. The
[Microsoft Scottish Gaelic Style Guide](https://download.microsoft.com/download/0/6/A/06A8E943-9546-4230-AD42-D1F592B9276E/gla-gbr-StyleGuide.pdf)
states this explicitly: "Address the user directly using *thu* (not *sibh*)…
Forms of *sibh*… should not be used. The tone should be moderately informal as
is common in software applications in Western Europe and existing localizations
in Scottish Gaelic." *Sibh* is reserved for sworn legal correspondence and high
formal ceremony, per the
[ChatsControl summary of the same guide](https://chatscontrol.com/learn/style-guides/microsoft-gd).
This overturns an earlier working assumption that *sibh* was the
software-localization norm; it is not, and using it in Netsuke's diagnostics
would read as stilted or archaic to a native reader.

Most of Netsuke's core build vocabulary already has native or nativized Gaelic
terms, verified against [Am Faclair Beag](https://www.faclair.com/) (the
dictionary the Microsoft guide names as its normative IT-terminology source)
and, for *cache*, against live usage on Gaelic Wikipedia. `togail`/`tog`
(build), `tasgadan` (cache), and `teamplaid` (template) are attested computing
senses, so none of these stay as English loans. `macro` is itself the attested
loan in Am Faclair Beag ("macro (in computing)"), inflecting as a masculine
noun (plural `macrothan`) rather than being calqued. `glob`, `stdlib`,
`dyndep`, and `pipeline` have no attested Gaelic computing sense and
conventionally stay as unadapted English technical loans, written in Latin
script exactly as in English, until Gaelic corpus planning catches up. `phony`,
by contrast, is rendered with the native element `breige` (genitive of `breug`,
"lie, falsehood"), already used adjectivally in established compounds such as
`dia-bréige` ("false god") and `saidheans-bréige` ("pseudoscience").

The main hazard is `manifest`: Am Faclair Beag has no computing sense for this
word at all, only the shipping/cargo sense `cunntas luchd luinge` ("ship's
manifest, shipping manifest, cargo document"). A translator reaching for a
native word here risks either that false friend or an overly literal calque on
`foillsich`/`taisbeanadh` ("reveal, disclose"), which reads as a philosophical
or religious "manifestation," not a config file. Netsuke keeps `manifest` as an
unadapted loan, taking regular masculine grammar (article, lenition, genitive)
like any other loan noun. Gaelic also lenites and mutates initial consonants
and forms compounds with the genitive; this applies to ordinary vocabulary but
must **not** apply to invariant identifiers and product names (Netsuke, Ninja,
Fluent, Jinja, YAML, `stdout`, `stderr`, UTF-8, `dyndep`, `foreach`, `vars`,
`command_available`), which stay fixed regardless of surrounding case or
lenition context. The worked example shows ordinary lenition/genitive applied
to the loan `manifest` while `{ $path }` is preserved verbatim:

```text
Dh'fhàillig luchdadh a' mhanifest aig { $path }.
```

Table 18: Scottish Gaelic terminology

| en-US                  | preferred                    | notes                                                                                                                                                                                                                                                          |
| ---------------------- | ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest                     | Unadapted loan; no computing sense in [Am Faclair Beag](https://www.faclair.com/?txtSearch=manifest), only the shipping-document false friend `cunntas luchd luinge`. Masculine, ordinary lenition/genitive apply (see worked example).                        |
| target                 | amas                         | Attested computing sense via `pasgan-amais`, "target folder (in computing)" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=amas)). Distinct from `targaid`, the loan used for target/goal in general media Gaelic.                                      |
| action                 | gnìomh                       | "Act, action, deed, task" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=gn%C3%AComh)). Kept distinct from `amas` (target) and `riaghailt` (rule) per Netsuke's three-way distinction.                                                                  |
| rule                   | riaghailt                    | "Rule, rule (device), regulation" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=riaghailt)); also used in `riaghailt togail`, "building regulation," showing the same compounding pattern.                                                             |
| dependency             | eisimeileachd                | Standard Gaelic word for dependence/dependency, attested in general and technical corpora (e.g. [OpenTran EN–GD examples](https://opentran.net)).                                                                                                              |
| order-only dependency  | eisimeileachd òrdugh a-mhàin | Coined compound (`eisimeileachd` + "order only"); no attested prior art, follows normal noun-phrase order.                                                                                                                                                     |
| build (noun)           | togail                       | "Building, construction" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=togail)).                                                                                                                                                                       |
| build (verb)           | tog                          | "Build, construct!" imperative/root form of `togail` ([Am Faclair Beag](https://www.faclair.com/?txtSearch=togail)).                                                                                                                                           |
| build graph            | graf togail                  | `graf` (graph) is a well-attested, freely compounding loan (`graf-loidhne`, `graf bloca`, etc.) ([Am Faclair Beag](https://www.faclair.com/?txtSearch=graf)); genitive compound with `togail` follows the same pattern.                                        |
| phony target           | amas brèige                  | `brèige` (genitive of `breug`, "lie") is the established adjectival element for "fake/false" in compounds like `dia-bréige` ([Am Faclair Beag](https://www.faclair.com/?txtSearch=breige)); applied here to `amas` by analogy.                                 |
| artefact               | toradh                       | "Outcome, result, output, product"; Am Faclair Beag glosses `toradh digiteach` as "digital product" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=toradh)).                                                                                            |
| working directory      | eòlaire-obrach               | `eòlaire` glossed plainly as "directory" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=e%C3%B2laire)), chosen over `pasgan` ("folder"), which Am Faclair Beag ties to GUI file-manager usage (`pasgan-amais`, "target folder").                        |
| workspace root         | bun-eòlaire an ionaid-obrach | Coined: `bun-` ("base/root," as in attested `bun-ìre`, "basic level") + `eòlaire`; not directly attested, flagged for review.                                                                                                                                  |
| cache                  | tasgadan                     | Directly attested: "cache (in computing)," with `tasgadan-seisein`, "session cache" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=tasgadan)); also live on [Gaelic Wikipedia](https://gd.wikipedia.org) ("Purgaidich an tasgadan," "purge the cache"). |
| allowlist              | liosta cheadaichte           | Coined from `ceadaichte`, "permitted, allowed" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=ceadaichte)); not directly attested as a compound.                                                                                                        |
| blocklist              | liosta bhacte                | Coined, parallel to `liosta cheadaichte`; not directly attested.                                                                                                                                                                                               |
| template               | teamplaid                    | Attested: "template" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=teamplaid)).                                                                                                                                                                        |
| macro                  | macro                        | Attested loan: "macro (in computing)," masculine, plural `macrothan` ([Am Faclair Beag](https://www.faclair.com/?txtSearch=macro)).                                                                                                                            |
| environment variable   | caochladair àrainneachd      | `caochladair`, "variable (in science)," already compounds this way in `caochladair réiteachaidh`, "configuration variable" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=caochladair)); `àrainneachd` is the standard word for "environment."          |
| exit status            | inbhe fàgail                 | Coined by analogy with the attested computing sense of `inbhe`, "status" (e.g. `inbhe-dhiùltaidh`, "bounce status (in computing)") ([Am Faclair Beag](https://www.faclair.com/?txtSearch=inbhe)).                                                              |
| stage (pipeline stage) | ìre                          | "Grade, degree, progress, stage" ([Am Faclair Beag](https://www.faclair.com/?txtSearch=%C3%ACre)); e.g. `ìre fàis`, "growth stage."                                                                                                                            |
| locale                 | sgeama ionadail              | Directly attested: "locale (in computing)," alongside the synonym `dreach ionadail` ([Am Faclair Beag](https://www.faclair.com/?txtSearch=locale)).                                                                                                            |
| placeable              | placeable                    | No attested Gaelic term for this Fluent-specific concept; recommend the unadapted English loan rather than coining one.                                                                                                                                        |

### Hebrew (`he`)

Hebrew software localization does not use a T–V distinction; the hazard is
grammatical gender, not formality. Verbs, adjectives, and the second-person
pronoun all inflect for the addressee's gender, and Hebrew has no
gender-neutral "they" equivalent that reads naturally in short UI or CLI
strings. The
[Microsoft Hebrew localization style guide](https://aka.ms/hebrew-styleguide)
(interim gender-inclusivity policy, dated December 2022) recommends
gender-neutral nouns and verbs where they exist, plural address for generalized
help text, and — specifically for software commands, menu items, and buttons —
the gerund form (e.g. שמירה, "saving", rather than the imperative שמור,
"save"), because the gerund carries no gender. Independently,
[Unbabel's Hebrew language guidelines](https://help.unbabel.com/hc/en-us/articles/4413927494167-Language-Guidelines-Hebrew)
converge on the same toolkit for instructions: impersonal construction (סתמי),
the infinitive, or — only for mass-audience text such as manuals — plural
imperative, all in preference to singular imperative. Netsuke's diagnostics and
CLI help never address "you" directly; they report tool state. That sidesteps
most of the gender problem, so the register chosen here is impersonal/nominal
reporting (gerund and third-person verb forms), matching both sources' guidance
for software strings without inventing a Netsuke-specific voice.

Several Netsuke terms stay as English loan words, transliterated into Hebrew
letters or kept in Latin script depending on register. `Build` (noun) is
commonly left in Latin script in developer-facing Microsoft Hebrew
documentation (e.g.
[Power Platform build tools](https://learn.microsoft.com/he-il/power-platform/alm/devops-build-tools),
which uses "Build" repeatedly in body text even though its own title uses the
calque בנייה); this table therefore keeps `Build` as a loan for the noun and
uses the native verb לבנות for "to build". `macro` (מאקרו) and `manifest`
(מניפסט) are established transliterated loans — the manifest loan is attested
in Google's Hebrew Android developer docs (e.g.
[App manifest overview, he](https://developer.android.com/guide/topics/manifest/manifest-intro?hl=he)).
By contrast, `cache` (מטמון) and `template` (תבנית) are native calques, not
loans, and are used consistently across Microsoft and SAP Hebrew technical
documentation.

Three hazards are worth flagging. First, מניפסט is a false friend: in general
Hebrew prose it primarily means a political manifesto, so Netsuke diagnostics
should keep it inside the fixed phrase קובץ המניפסט ("the manifest file")
rather than using מניפסט bare. Second, "directory" has two competing Hebrew
words: ספרייה (technical/POSIX register, attested in SAP's Hebrew
documentation) and תיקייה (Windows Explorer's "folder", a casual register).
Netsuke is a POSIX-facing CLI tool for developers and CI operators, so this
table uses ספרייה throughout and avoids תיקייה. Third, `workspace` (סביבת
עבודה) and `environment` (סביבה, as in משתנה סביבה, "environment variable")
share the root סביבה; a sentence that mentions both concepts risks reading as
if they were the same thing, so Netsuke prose should always spell out
"environment variable" in full and never elide it to "the environment"
mid-sentence. Separately, per the Microsoft style guide's orthography rules, a
Hebrew clitic (ל, ב, מ, ו, ש, ה) placed directly before a Latin identifier or
number must be separated with a hyphen (e.g. ל-Netsuke, ב-YAML) to prevent the
bidirectional-text renderer from reordering the boundary incorrectly; this
recurs constantly in diagnostics that interpolate paths or flags next to Hebrew
prepositions.

Worked example, preserving the placeable `{ $path }` exactly and using the
table's own terms (מניפסט for manifest):

```text
טעינת קובץ המניפסט בנתיב { $path } נכשלה.
```

Table 19: Hebrew terminology

| en-US                  | preferred                    | notes                                                                                                                                                                                                                                                                                                                                      |
| ---------------------- | ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| manifest               | מניפסט (manifest)            | Loan, transliterated; attested in [Android manifest docs, he](https://developer.android.com/guide/topics/manifest/manifest-intro?hl=he). False friend: also means political manifesto — keep inside קובץ המניפסט.                                                                                                                          |
| target                 | יעד (ya'ad)                  | Distinct from action/rule per Netsuke's model; general Hebrew word for "goal/destination", standard in Hebrew IT usage.                                                                                                                                                                                                                    |
| action                 | פעולה (pe'ula)               | Distinct from target/rule; standard Hebrew for "action/operation".                                                                                                                                                                                                                                                                         |
| rule                   | כלל (klal)                   | Distinct from target/action; standard Hebrew for "rule".                                                                                                                                                                                                                                                                                   |
| dependency             | תלות (tlut)                  | Standard Hebrew software term for "dependency/reliance".                                                                                                                                                                                                                                                                                   |
| order-only dependency  | תלות מסדר-בלבד               | Coined calque on תלות; no established Hebrew rendering of Ninja's concept found in research — verify with a bilingual reviewer before shipping.                                                                                                                                                                                            |
| build (noun)           | Build (loan, Latin script)   | Kept in Latin script in developer-facing docs; see [Power Platform build tools, he](https://learn.microsoft.com/he-il/power-platform/alm/devops-build-tools). בנייה is used only in generic prose (e.g. "כלי בנייה").                                                                                                                      |
| build (verb)           | לבנות (livnot)               | Standard Hebrew infinitive "to build"; pairs with gerund בנייה in nominal contexts.                                                                                                                                                                                                                                                        |
| build graph            | גרף הבנייה                   | Coined calque (בנייה + graph); no dedicated Hebrew source found — verify.                                                                                                                                                                                                                                                                  |
| phony target           | יעד פיקטיבי                  | Coined; פיקטיבי is the standard Hebrew adjective for "fictitious/dummy" in technical Hebrew.                                                                                                                                                                                                                                               |
| artefact               | תוצר בנייה                   | תוצר ("product/output") is broadly used in Hebrew engineering prose; the compound is a calque, not independently sourced.                                                                                                                                                                                                                  |
| working directory      | ספריית עבודה                 | POSIX/technical register (SAP ECTR he-IL docs, glosbe corpus). Avoid תיקיית עבודה — the Windows-GUI "folder" register.                                                                                                                                                                                                                     |
| workspace root         | ספריית השורש של סביבת העבודה | See the workspace/environment collision hazard in the prose above.                                                                                                                                                                                                                                                                         |
| cache                  | מטמון (matmon)               | Native Hebrew calque, not a loan; pervasive in Microsoft and SAP he-IL documentation.                                                                                                                                                                                                                                                      |
| allowlist              | רשימת היתרים                 | Coined, following Microsoft's general move away from whitelist/blacklist wording; no direct Hebrew developer-doc source found — verify.                                                                                                                                                                                                    |
| blocklist              | רשימת חסימה                  | Coined pairing with allowlist above; verify before shipping.                                                                                                                                                                                                                                                                               |
| template               | תבנית (tavnit)               | Native calque, well established (Aspose and SAP he-IL docs).                                                                                                                                                                                                                                                                               |
| macro                  | מאקרו (loan)                 | Transliterated loan, standard across Hebrew office/software documentation.                                                                                                                                                                                                                                                                 |
| environment variable   | משתנה סביבה                  | Standard; confirmed in SAP he-IL docs and general technical corpora. Always spell out in full — see collision hazard above.                                                                                                                                                                                                                |
| exit status            | קוד יציאה                    | "Exit code"; standard in Hebrew CLI/cloud docs, e.g. [Google Cloud GKE troubleshooting, he](https://docs.cloud.google.com/kubernetes-engine/docs/troubleshooting/crashloopbackoff-events?hl=he) and the [Zabbix he translation](https://translate.zabbix.com/he/documentation-70/manual/appendix/command_execution.xliff/?status=missing). |
| stage (pipeline stage) | שלב (shlav)                  | Standard Hebrew "stage/step"; qualify as שלב בצינור if pipeline context is not otherwise clear.                                                                                                                                                                                                                                            |
| locale                 | לוקאל (loan)                 | Dev-facing API docs keep the loan, e.g. [Chrome i18n API, he](https://developer.chrome.com/docs/extensions/reference/api/i18n?hl=he). Consumer settings use the calque אזור ("region") instead — avoid that calque here to prevent confusion with geographic region.                                                                       |
| placeable              | פלייסאבל (loan, coined)      | No established Fluent/Project Fluent Hebrew rendering found in research; transliteration is an editorial judgement call — verify with a Fluent-literate reviewer.                                                                                                                                                                          |

### Hindi (`hi`)

Netsuke's Hindi output uses the formal आप (aap) register with the corresponding
polite verb forms (करें, नहीं मिला, विफल रहा), never the familiar तुम or intimate
तू. Both the
[Mozilla Hindi (hi-IN) localizer style guide](https://mozilla-l10n.github.io/styleguides/hi-IN/)
and the [Microsoft Hindi style guide](https://aka.ms/hindi-styleguide)
prescribe honorific pronouns for software strings, and Mozilla's guide is
explicit that imperative forms such as ढूँढो or करो read as rude; the polite
imperative ढूँढें/करें is required instead. This matches CLI and diagnostic
conventions in GNOME- and KDE-derived Hindi translations, where system messages
consistently address the user formally.

Hindi technical writing keeps a substantial layer of English loan words
transliterated into Devanagari rather than coining Sanskritic equivalents,
particularly for concepts with no settled native term. `build` (बिल्ड), `cache`
(कैश), and `template` (टेम्पलेट) are established loans confirmed across Debian,
GNOME, and general Hindi computing literature; Netsuke follows this practice
rather than inventing calques such as निर्माण or प्रारूप, which would read as
unfamiliar or overly literary. Where a term already has a stable native form in
developer tooling — `dependency` as निर्भरता (used throughout Debian/apt Hindi
translations), `target` as लक्ष्य (used in KDE Hindi strings for build/output
targets), and `rule` as नियम — Netsuke uses that native form for consistency
with the existing Hindi FOSS corpus rather than a loan word. Nukta consonants
(फ़, ज़, क़) are used for retained Perso-Arabic and English sounds — for example
फ़ाइल (file) rather than फाइल — following standard Hindi computing orthography;
Netsuke's Hindi strings use the nukta forms consistently.

Two hazards deserve attention. First, `environment variable` is conventionally
पर्यावरण चर in real-world Hindi technical content (Windows GPO tutorials,
developer blogs), even though पर्यावरण's primary sense is the natural
environment; the less common alternative परिवेश चर avoids that collision but is
rarer in practice, so Netsuke follows the dominant, attested usage (पर्यावरण चर)
and relies on context to prevent misreading. Second, `manifest` has no risky
Hindi cognate collision, but a literal calque (घोषणापत्र,
"declaration/manifesto") would suggest a political or legal document rather
than a build input file, so Netsuke keeps `manifest` untranslated as an
identifier-like term is avoided with the transliteration मेनिफ़ेस्ट instead of a
native calque, matching the loan-word convention used for other
Netsuke-specific nouns.

Worked example:

```text
{ $path } पर मेनिफ़ेस्ट लोड करने में विफल रहा।
```

Table 20: Hindi terminology

| en-US                  | preferred       | notes                                                                                                                                                        |
| ---------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| manifest               | मेनिफ़ेस्ट          | Transliteration; a calque (घोषणापत्र) risks reading as "manifesto/declaration".                                                                               |
| target                 | लक्ष्य            | Established in KDE Hindi strings for build/output targets ([KDE po](https://github.com/KDE/kpty/blob/master/po/hi/kpty6.po)).                                |
| action                 | क्रिया           | Distinct from लक्ष्य/नियम; क्रिया avoids overlap with प्रशासनिक कार्रवाई ("administrative action").                                                               |
| rule                   | नियम            | Standard Hindi technical term, consistent with firewall/build "rule" usage.                                                                                  |
| dependency             | निर्भरता         | Used throughout Debian/apt Hindi translations ([apt.git po](https://git.bingner.com/apt.git/diff/po?h=0.9.7.7&id=549da1a850f0fd50e0a55415452ecaa735d49451)). |
| order-only dependency  | क्रम-केवल निर्भरता | Compound on निर्भरता; qualifies ordering without triggering rebuilds.                                                                                         |
| build (noun)           | बिल्ड            | Loan word (nirbhartā-style term unattested for the noun); widely used unchanged in Hindi computing.                                                          |
| build (verb)           | बिल्ड करना       | Loan + light verb, per common Hindi tech usage (e.g. "प्रोजेक्ट बिल्ड करना").                                                                                    |
| build graph            | बिल्ड ग्राफ़       | Loan compound; ग्राफ़ (graph) itself is an established loan with nukta.                                                                                        |
| phony target           | फोनी लक्ष्य       | लक्ष्य qualified by the transliterated adjective; no established native calque found.                                                                          |
| artefact               | आर्टिफ़ैक्ट         | Transliteration; preferred over उत्पाद ("product"), which is too generic.                                                                                     |
| working directory      | कार्य निर्देशिका   | Attested in Hindi terminal/CLI tutorials ([terminalcheatsheet.com](https://terminalcheatsheet.com/hi/guides/navigate-terminal)).                             |
| workspace root         | कार्यक्षेत्र मूल     | कार्यक्षेत्र (workspace) + मूल (root); parallels कार्य निर्देशिका.                                                                                                   |
| cache                  | कैश              | Loan word, consistently unchanged with nukta-free spelling in Hindi computing texts.                                                                         |
| allowlist              | अनुमति सूची       | Native compound: अनुमति (permission) + सूची (list); mirrors blocklist below.                                                                                   |
| blocklist              | अवरोध सूची       | Native compound: अवरोध (block) + सूची (list).                                                                                                                 |
| template               | टेम्पलेट           | Loan word, standard across Hindi office/software documentation.                                                                                              |
| macro                  | मैक्रो            | Loan word; no native alternative in common use.                                                                                                              |
| environment variable   | पर्यावरण चर      | Dominant real-world usage despite the natural-environment sense of पर्यावरण; see hazards above.                                                               |
| exit status            | निकास स्थिति     | Attested in Hindi Linux/process discussions ([testbook.com](https://testbook.com/objective-questions/hn/mcq-on-process--5eea6a1539140f30f369f334)).          |
| stage (pipeline stage) | चरण             | Standard Hindi word for a stage/phase; used generically across Hindi technical writing.                                                                      |
| locale                 | लोकेल            | Transliteration; a calque (स्थान-विशेष सेटिंग) would be unwieldy for a UI/diagnostic term.                                                                       |
| placeable              | प्लेसहोल्डर        | Transliteration of the Fluent-adjacent concept; no established native term found.                                                                            |

Sources consulted:

- [Microsoft Hindi style guide](https://aka.ms/hindi-styleguide)
- [Mozilla Hindi (hi-IN) localizer style guide](https://mozilla-l10n.github.io/styleguides/hi-IN/)
- [KDE kpty Hindi po file](https://github.com/KDE/kpty/blob/master/po/hi/kpty6.po)
- [apt.git Hindi po diff (Debian/apt translations)](https://git.bingner.com/apt.git/diff/po?h=0.9.7.7&id=549da1a850f0fd50e0a55415452ecaa735d49451)
- [terminalcheatsheet.com Hindi terminal navigation guide](https://terminalcheatsheet.com/hi/guides/navigate-terminal)
- [Testbook Hindi Process MCQ (exit status usage)](https://testbook.com/objective-questions/hn/mcq-on-process--5eea6a1539140f30f369f334)

### Hungarian (`hu`)

Hungarian technical writing defaults to an impersonal third-person register
and, where direct address is unavoidable, the formal singular "önözés" form; it
avoids the informal "tegezés" in software text. Both the Microsoft Hungarian
style guide's pronoun guidance ("avoid using a pronoun in the translation if
possible. If not, use the polite third-person singular … imperative,
declarative, or inquisitive mood ('önözés')";
[Microsoft Hungarian style guide](https://aka.ms/hungarian-styleguide)) and the
FSF.hu FOSS translators' handbook agree: "alapvetően személytelen egyes szám
harmadik személyű megszólítást használunk … a felhasználót nem tegezzük sem a
felhasználói felületeken, sem a dokumentációkban" (the reader is addressed
impersonally in the third person, and never with "tegezés", in interfaces or
documentation; [FSF.hu Fordítás HOGYAN](http://forditas.fsf.hu/)). This matches
Netsuke's calm, professional register: diagnostics stay impersonal and state
the condition rather than addressing "you" directly, and any CLI help that does
address the operator uses "önözés" consistently. The Ubuntu Hungarian team's
own guidance is looser for command-line tools, tolerating "tegezés" there
([Ubuntu Hungarian team](https://wiki.ubuntu.com/HungarianTeam/Translation));
Netsuke follows the stricter Microsoft/FSF.hu line instead, since its
diagnostics read as professional tooling output rather than a casual script.

Netsuke keeps a small set of terms as English loans, following current
Hungarian developer usage rather than inventing calques. "Build" itself is
commonly left as an unadapted noun and takes Hungarian verb-forming suffixes
directly, without a hyphen: "buildelés" (building, noun) and "buildelt" (built,
participle)
([itszotar.hu build glossary](https://itszotar.hu/build-a-fogalom-jelentese-a-szoftverfejlesztesi-folyamatban/));
the older calque "összeállítás" also appears in some FOSS glossaries and
remains understandable
([OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)).
"Macro" and the build-output sense of "artefact" are adapted loans in active
use ("makró", "artefaktum"), while "cache" ("gyorsítótár") and "template"
("sablon") already have well-established native equivalents and are not left as
loans. Capitalized product and file names such as Netsukefile, Ninja, and Jinja
stay as English identifiers and take a hyphen before a Hungarian case suffix,
per the standard product-name declension rule (for example "Skype-ban";
[Microsoft Hungarian style guide](https://aka.ms/hungarian-styleguide)): a
diagnostic reads "a Netsukefile-t nem sikerült beolvasni", never "a
Netsukefilet".

The main hazard is "manifest": general Hungarian only knows it as "kiáltvány"
(political manifesto) or, in shipping and customs contexts,
"árujegyzék"/"rakományjegyzék"; neither fits a build-time input file. Hungarian
software text sidesteps this by keeping "manifest" as an English loan paired
with "fájl" ("manifest fájl"), the pattern already used for browser-extension
manifests ([Linux Mint Hungary](https://linuxmint.hu/cimkek/letoltes?page=87));
the worked example below follows the same pattern. "Action" is not "akció":
that word means a commercial sale or promotion in everyday Hungarian, a classic
false friend, so Netsuke uses "művelet" instead, confirmed by the OpenScope
FOSS software dictionary
([OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)).
Because Netsuke treats target, action, and rule as three distinct concepts,
"cél", "művelet", and "szabály" must stay separate and never substitute for one
another in diagnostics, even though "cél" also carries an everyday sense of
"goal/aim" that context resolves without difficulty. There is no casing or
script trap for Hungarian, since it uses the Latin script with diacritics that
do not affect identifier casing or sort order.

Worked example, preserving the placeable exactly:

```text
Nem sikerült betölteni a manifest fájlt itt: { $path }.
```

Table 21: Hungarian terminology

| en-US                  | preferred            | notes                                                                                                                                                                                                                                                                   |
| ---------------------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest             | Loan, paired with "fájl" in running text; general Hungarian sense is "kiáltvány" (political manifesto) — see hazards above. [Linux Mint Hungary](https://linuxmint.hu/cimkek/letoltes?page=87)                                                                          |
| target                 | cél                  | Distinct from action/rule. [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)                                                                                                                                                                       |
| action                 | művelet              | Implicitly phony target; not "akció" (false friend, "sale/promotion"). [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)                                                                                                                           |
| rule                   | szabály              | The Ninja construct a target references. [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)                                                                                                                                                         |
| dependency             | függőség             | Standard term across FOSS glossaries. [Drupal Hungarian glossary](https://localize.drupal.org/node/204)                                                                                                                                                                 |
| order-only dependency  | sorrendi függőség    | Qualifies "függőség"; not independently attested, built on the established base term                                                                                                                                                                                    |
| build (noun)           | build                | Common unadapted loan in current Hungarian dev writing. [itszotar.hu](https://itszotar.hu/build-a-fogalom-jelentese-a-szoftverfejlesztesi-folyamatban/); older calque "összeállítás" also seen ([OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)) |
| build (verb)           | buildel / buildelés  | Loan + native verb-forming suffix, no hyphen. [itszotar.hu](https://itszotar.hu/build-a-fogalom-jelentese-a-szoftverfejlesztesi-folyamatban/)                                                                                                                           |
| build graph            | buildgráf            | Compositional; short compound written as one word per the Microsoft syllable-count compounding rule                                                                                                                                                                     |
| phony target           | álcél                | Compositional, "ál-" (pseudo-) prefix as in "álnév", "álkód"; maps to Netsuke's "action" concept                                                                                                                                                                        |
| artefact               | artefaktum           | Adapted loan, standard in Hungarian CI/CD writing. [itszotar.hu](https://itszotar.hu/build-a-fogalom-jelentese-a-szoftverfejlesztesi-folyamatban/)                                                                                                                      |
| working directory      | munkakönyvtár        | [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)                                                                                                                                                                                                  |
| workspace root         | munkaterület gyökere | Compound of "munkaterület" + "gyökér"; no single established short form found                                                                                                                                                                                           |
| cache                  | gyorsítótár          | Native compound, not a loan. [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)                                                                                                                                                                     |
| allowlist              | engedélyezési lista  | Official Microsoft Hungarian term. [Microsoft Learn hu-hu](https://learn.microsoft.com/hu-hu/defender-office-365/connection-filter-policies-configure)                                                                                                                  |
| blocklist              | tiltólista           | Official Microsoft Hungarian term, same source                                                                                                                                                                                                                          |
| template               | sablon               | [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)                                                                                                                                                                                                  |
| macro                  | makró                | Adapted loan. [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)                                                                                                                                                                                    |
| environment variable   | környezeti változó   | [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/)                                                                                                                                                                                                  |
| exit status            | kilépési állapot     | GNU coreutils Hungarian man-page translation. [manpages-l10n](https://fossies.org/linux/manpages-l10n/po/hu/man1/chroot.1.po)                                                                                                                                           |
| stage (pipeline stage) | szakasz              | Common in Hungarian GitLab CI writing. [EventHub thesis](https://edu.codespring.ro/wp-content/uploads/2025/08/EventHub.pdf)                                                                                                                                             |
| locale                 | területi beállítások | [OpenScope dictionary](https://bkil.gitlab.io/openscope-dict-eng-hun/); the raw locale tag (e.g. `hu-HU`) is left unchanged in identifiers                                                                                                                              |
| placeable              | helyőrző             | No dedicated Fluent term found; reuses the established UI "placeholder" term. [Microsoft support](https://support.microsoft.com/hu-hu/powerpoint/add-edit-or-remove-a-placeholder-on-a-slide-layout)                                                                    |

### Indonesian (`id`)

Indonesian technical writing addresses the reader with the second-person
pronoun **Anda**, capitalized wherever it occurs, rather than the informal
**kamu** or an impersonal construction. The
[Microsoft Indonesian localization style guide](https://aka.ms/indonesian-styleguide)
sets this as the norm for Microsoft voice: instructions and status text speak
to the user directly with "Anda" ("Bila Anda tersambung ke Internet…"), and
this convention is echoed across major vendor documentation translated into
Indonesian (Kubernetes, AWS, Google Cloud). Netsuke's diagnostics and CLI help
should follow the same register: calm, direct, and formal-but-plain, never
using "kamu" or slang. Where a message need not name the reader at all (most
diagnostics), Netsuke uses an impersonal, agentless phrasing, in line with the
guide's error-message patterns.

A large share of Netsuke's vocabulary already circulates as English loan words
in Indonesian developer writing, written in unmodified Latin script: `build`,
`cache`, `template` (or its KBBI-listed spelling `templat`), `stage`, and
`pipeline` are commonly left untranslated or only lightly adapted in practice,
even though KBBI's computing glossary offers native or Indonesianized
alternatives for several of them. `dependensi` (an Indonesianized loan, not a
native calque) dominates real developer usage over the native `ketergantungan`,
which reads as awkwardly formal or essayistic in a CLI context. This
loan-versus-coinage tension — official KBBI/Pusat Bahasa spellings competing
with raw English loans that practitioners actually use — is the central style
question for this locale; Netsuke follows established developer usage over KBBI
purism whenever the two diverge, and notes each case in the table below.

The chief hazard is **manifes**, the Indonesian shipping/customs term for a
cargo manifest — a good semantic match for Netsuke's build manifest and the
term used here — but it must not be confused with **manifesto** (a political
manifesto), a visually similar but unrelated word. Two more traps: `target`,
`aksi`/`tindakan`, and `aturan` must stay three distinct, consistently used
words even though everyday Indonesian could blur "action" and "rule" in looser
prose; and `cache`, when Indonesianized, has two competing coinages
(`tembolok`, the original KBBI term, and `singgahan`, a later proposal) that
are both now rare in practice next to the plain English loan.

Table 22: Indonesian terminology

| en-US                  | preferred                    | notes                                                                                                                                                                                           |
| ---------------------- | ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifes                      | Cargo/shipping-document cognate, good semantic fit; do not confuse with "manifesto" (political manifesto).                                                                                      |
| target                 | target                       | Loan word; standard in Indonesian developer usage ([Microsoft style guide](https://aka.ms/indonesian-styleguide)).                                                                              |
| action                 | tindakan                     | Distinct from `aturan` (rule) and `target`; keep consistent throughout.                                                                                                                         |
| rule                   | aturan                       | Distinct from `tindakan` (action); matches general Indonesian software usage.                                                                                                                   |
| dependency             | dependensi                   | Indonesianized loan; dominant over native `ketergantungan` in package-manager and build-tool docs (Ubuntu/Debian, GNOME).                                                                       |
| order-only dependency  | dependensi hanya-urutan      | Compound built on `dependensi`; qualifier clarifies ordering-only semantics.                                                                                                                    |
| build (noun)           | build                        | Left as an English loan in Indonesian developer writing (AWS, IBM Indonesian docs use `build` untranslated).                                                                                    |
| build (verb)           | membangun                    | Native verb for "to build/construct", widely used for CI/build actions ("membangun proyek").                                                                                                    |
| build graph            | graf build                   | `build` kept as loan noun; `graf` is the standard Indonesian term for graph.                                                                                                                    |
| phony target           | target semu                  | `semu` ("false"/"apparent") is the standard Indonesian qualifier for "phony" in this sense.                                                                                                     |
| artefact               | artefak                      | Standard Indonesian spelling; used in software/build contexts.                                                                                                                                  |
| working directory      | direktori kerja              | Established term across Indonesian CLI and shell documentation.                                                                                                                                 |
| workspace root         | akar ruang kerja             | `ruang kerja` for workspace, `akar` for root, consistent with filesystem-root usage.                                                                                                            |
| cache                  | cache                        | Kept as English loan; KBBI's `tembolok` and the later coinage `singgahan` are both rarely used in practice ([discussion](https://ivanlanin.wordpress.com/2010/05/15/tembolok-atau-singgahan/)). |
| allowlist              | daftar izin                  | Literally "permission list"; parallels `blocklist` below.                                                                                                                                       |
| blocklist              | daftar blokir                | Literally "block list"; `blokir` is an established Indonesian verb/noun for blocking.                                                                                                           |
| template               | `templat`                    | KBBI-listed Indonesianized spelling, in active use (Wikipedia Bahasa Indonesia, JobStreet ID).                                                                                                  |
| macro                  | `makro`                      | Standard Indonesianized spelling used across Indonesian technical writing.                                                                                                                      |
| environment variable   | `variabel` lingkungan        | Established term (Kubernetes, AWS, Google Cloud, Microsoft Indonesian docs).                                                                                                                    |
| exit status            | status keluar                | Established term (Debian manpages, AWS Indonesian docs).                                                                                                                                        |
| stage (pipeline stage) | tahap                        | Native word for a pipeline phase; `stage` as a loan is also seen but `tahap` reads more naturally in prose.                                                                                     |
| locale                 | `lokal` / lokalisasi konteks | `locale` itself is often kept as a loan in technical contexts; `lokal` used only when unambiguous from context.                                                                                 |
| placeable              | placeable                    | Fluent-specific technical term; left as an English loan, as no established Indonesian translation exists in developer literature.                                                               |

Worked example, preserving the placeable `{ $path }` exactly:

```text
Gagal memuat manifes di { $path }.
```

This follows the guide's standard pattern for "Failed to…" diagnostics ("Gagal
…"), keeps `manifes` (not "manifesto"), and uses the impersonal diagnostic
register rather than addressing "Anda" directly, since this message states a
condition rather than instructing the user.

### Italian (`it`)

Italian technical writing consistently avoids addressing the reader directly.
The
[tp.linux.it "Regole per la buona traduzione"](https://tp.linux.it/buona_traduzione.html)
is the reference style guide for Italian free-software translators, drawing on
GNOME, KDE and Sun/Oracle house styles. It states that while English text
speaks to the user directly, Italian prefers impersonal constructions or the
passive voice, and that a program referring to itself in the first person ("I'm
going to…") should switch to an impersonal or passive form ("Verranno poste
alcune domande"). The
[Microsoft Italian localization style guide](https://aka.ms/italian-styleguide)
likewise favours the infinitive for instructions and a neutral, unadorned tone
over the more solicitous English convention of "please". Netsuke's diagnostics
therefore use impersonal or passive constructions ("`Impossibile` caricare…",
"Il file non è stato trovato") rather than addressing the operator as "tu" or
"Lei"; imperative infinitives are reserved for suggested corrective actions
("Verificare il percorso").

Several Netsuke terms stay as unmarked English loans, following established
Italian developer-tool usage: `cache` (invariant, feminine, "la cache"), and
`target` in the build-system sense — Italian localizations of Android, AWS,
Godot and CMake documentation all use "target di build" rather than a calque
such as "destinazione", even though Microsoft's general-purpose glossary maps
"target" (as in "target language") to "destinazione" in non-technical contexts.
`build` (noun) is likewise conventionally left as a loan — "la build" — in
Italian technical prose (see Italian Wikipedia's "Software" article and Red
Hat/IBM developer documentation); "compilazione" is reserved for the narrower
compile step, which matters for Netsuke because a Netsuke build is not strictly
compilation. `placeable` (the Fluent term) has no established Italian rendering
and is used as a bare loan in Italian CAT-tool literature. All English loans
keep Latin script and are not inflected for plural, per the tp.linux.it rule
that foreign terms stay in their unflected singular form.

The chief hazard is "manifesto": the Italian cognate of "manifest" collides
with the everyday and politically loaded senses "manifesto" (political
manifesto; also the name of a well-known Italian newspaper, *il manifesto*)
and, less critically, "poster". Italian technical writing sidesteps this by
keeping "manifest" as an invariant loan (e.g. "file manifest") rather than
translating it. Netsuke follows the same practice below. A secondary hazard is
over-literal translation of gerunds ("building" → "Compilazione in corso", not
a fabricated Italian gerund) and of "and/or", which tp.linux.it flags as a
source of mistranslation; Netsuke diagnostics avoid "e/o" and use plain "o".

```text
Impossibile caricare il manifest in { $path }.
```

Table 23: Italian terminology

| en-US                  | preferred                      | notes                                                                                                                                                                                                                                                                                                    |
| ---------------------- | ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest                       | Invariant loan; avoid "manifesto" (political/newspaper sense). See [flavio.tordini.org](https://flavio.tordini.org/dirette-raitv-senza-silverlight-o-moonlight) for established Italian tech usage of "file manifest".                                                                                   |
| target                 | target                         | Invariant loan in build-tool contexts; confirmed by Italian [Android SBOM docs](https://source.android.com/docs/setup/create/create-sbom?hl=it), [Godot docs (it)](https://docs.godotengine.org/it/4.x/community/asset_store/what_is_asset_store.html), AWS CMake docs. Distinct from `azione`/`regola`. |
| action                 | azione                         | Standard translation; matches GitHub Actions' Italian localization ("Azioni"). Distinct from `target`/`regola`.                                                                                                                                                                                          |
| rule                   | regola                         | Standard translation of the Ninja/Make construct. Distinct from `target`/`azione`.                                                                                                                                                                                                                       |
| dependency             | dipendenza                     | Well established, e.g. .NET "Dependency Injection" → "Iniezione delle dipendenze" (Microsoft docs).                                                                                                                                                                                                      |
| order-only dependency  | dipendenza order-only          | Compound; "order-only" has no idiomatic Italian equivalent and is kept as a loan qualifier, mirroring how "target" is treated.                                                                                                                                                                           |
| build (noun)           | build                          | Invariant loan ("la build"); see [it.wikipedia.org/wiki/Software](https://it.wikipedia.org/wiki/Software). "Compilazione" is narrower (compile step only) and would mislead for Netsuke, which is not strictly a compiler.                                                                               |
| build (verb)           | `generare` / eseguire la build | `Compilare` is avoided for the same reason as above; `generare` or "eseguire la build" used instead.                                                                                                                                                                                                     |
| build graph            | grafo di build                 | Compositional; "build" kept as loan per above.                                                                                                                                                                                                                                                           |
| phony target           | target fittizio                | "Fittizio" ("fictitious"/"dummy") is the standard qualifier for a non-file target; "target" itself stays a loan.                                                                                                                                                                                         |
| artefact               | artefatto                      | Established in Italian CI/CD usage, e.g. Azure DevOps pipeline docs ("artefatti di build").                                                                                                                                                                                                              |
| working directory      | directory di lavoro            | Confirmed across Italian technical literature (e.g. university course notes, Stata/Python guides); "directory" is itself an invariant loan in Italian.                                                                                                                                                   |
| workspace root         | radice dell'area di lavoro     | "Area di lavoro" is Microsoft's standard rendering of "workspace" ([Visual Studio docs, it-it](https://learn.microsoft.com/it-it/visualstudio/extensibility/workspaces?view=visualstudio)); "radice" adds "root".                                                                                        |
| cache                  | cache                          | Invariant loan, feminine ("la cache").                                                                                                                                                                                                                                                                   |
| allowlist              | elenco elementi consentiti     | Microsoft's standard Italian rendering, used consistently across Microsoft Learn (Defender, Edge, Entra docs).                                                                                                                                                                                           |
| blocklist              | elenco elementi bloccati       | Paired with allowlist; same Microsoft Learn sources.                                                                                                                                                                                                                                                     |
| template               | modello                        | Microsoft Italian style guide gives "modello" for "template" (e.g. "Template Wizard" → "Creazione guidata modello"); the loan "template" also occurs in casual dev writing but "modello" is preferred for consistency with Microsoft terminology.                                                        |
| macro                  | macro                          | Invariant loan; unchanged in Italian in both general and technical use.                                                                                                                                                                                                                                  |
| environment variable   | variabile di ambiente          | Compositional; "ambiente" is Microsoft's standard translation of "environment" (e.g. "development environment" → "ambiente di sviluppo").                                                                                                                                                                |
| exit status            | stato di uscita                | Standard across Italian man pages (Debian, Arch, IBM AIX docs); "codice di uscita" is a secondary variant sometimes seen but "stato di uscita" is the man-page norm.                                                                                                                                     |
| stage (pipeline stage) | `fase`                         | Standard Italian CI/CD term for a pipeline stage; compositional, no loan needed.                                                                                                                                                                                                                         |
| locale                 | locale                         | Invariant loan in Italian software/l10n contexts; not to be confused with the unrelated Italian adjective "locale" (local), which is disambiguated by context.                                                                                                                                           |
| placeable              | placeable                      | No established Italian rendering; used as a bare loan in Italian CAT-tool/l10n literature (see [intralinea.org](http://www.intralinea.org/monographs/beraldin/cattm.html)).                                                                                                                              |

### Japanese (`ja`)

Netsuke's Japanese output should use teineigo throughout: the Desu-masu style
(です・ます調) for any prose addressing the reader — diagnostics, explanatory
text, and instructions to take action (e.g. "…してください") — per the
[Microsoft Japanese style guide](https://aka.ms/japanese-styleguide), which
prescribes Desu-masu for message bodies and explanatory text while reserving
Dearu style (である調) or noun-ending 体言止め for dialog-box titles, labels,
and command buttons. This matches Netsuke's own split between prose diagnostics
and terse CLI labels/status lines. The
[JTF Japanese Standard Style Guide](https://www.jtf.jp/tips/styleguide) (12
basic rules, rule 1) independently requires that a single register — 敬体
(ですます) or 常体（である） — be used consistently throughout a document, so
Netsuke should not mix the two within one message.

Katakana loans dominate for concepts borrowed wholesale from build tooling:
ターゲット (target), アクション (action), ルール (rule), ビルド (build, noun
and verb), キャッシュ (cache), テンプレート (template), マクロ (macro),
ステージ (stage), ロケール (locale). Established Japanese translations of GNU
Make and MSBuild use ターゲット and ルール consistently as distinct concepts,
which supports keeping `target`, `action`, and `rule` as three separate
katakana words in Netsuke rather than collapsing them. Native compounds are
preferred where Japanese developer documentation already has a settled calque:
依存関係 (dependency), 作業ディレクトリ (working directory), 環境変数
(environment variable), and 終了ステータス (exit status) all appear in the
[Japanese translation of the GNU Make 4.4 manual](https://www.ecoop.net/coop/translated/GNUMake4.4/ch04.jp.html).
`allowlist`/`blocklist` follow Microsoft's and Splunk's inclusive-terminology
guidance: 許可リスト / ブロックリスト, a calque-plus-loan blend rather than a
direct translation of the old "white/black" pair. Sources:

- [Microsoft Learn](https://learn.microsoft.com/ja-jp/defender-office-365/tenant-allow-block-list-urls-configure)
- [Splunk](https://www.splunk.com/ja_jp/blog/security/blacklist-whitelist-inclusivity.html)

Hazards: マニフェスト ("manifest") is a strong false friend in Japanese — since
the early 2000s it has been the everyday word for a political election
manifesto (政権公約), so a bare マニフェスト in a diagnostic can read as a
policy document rather than a build input; keep it anchored to "file"/"YAML"
context where space allows. Mechanically, JTF rule 5 forbids dropping the
long-vowel mark on katakana loans (サーバー, not サーバ), a policy also adopted
by Microsoft since 2008 and codified in
[JIS Z 8301:2019](https://kikakurui.com/z8/Z8301-2019-01.html); none of
Netsuke's own loan terms currently drop it, but future additions must keep it.
JTF rule 10 forbids a half-width space between half-width (Latin/digit) and
full-width (Japanese) characters, so identifiers and placeables such as
`Netsuke`, `Ninja`, and `{ $path }` sit directly against the surrounding
Japanese text with no space, and punctuation stays full-width (。、). Neither
"phony target" nor "order-only dependency" has one fixed, universally used
Japanese term: the GNU Make Japanese translation uses both 偽りのターゲット and
疑似ターゲット for "phony", and 順序のみの前提条件 for "order-only"; Netsuke's
table below follows that manual's pattern, substituting the project's own
依存関係 for "prerequisite" to stay consistent with the `dependency` row.

```text
マニフェスト `{ $path }` の読み込みに失敗しました。
```

Table 24: Japanese terminology

| en-US                  | preferred              | notes                                                                                                                                                                                                                                                            |
| ---------------------- | ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | マニフェスト           | Loan; false friend with political "manifesto" (政権公約) — see hazards above.                                                                                                                                                                                    |
| target                 | ターゲット             | Loan; matches [MSBuild ja glossary](https://learn.microsoft.com/ja-jp/visualstudio/msbuild/msbuild-glossary?view=visualstudio) and GNU Make ja.                                                                                                                  |
| action                 | アクション             | Loan; kept distinct from target/rule per Netsuke's own semantics.                                                                                                                                                                                                |
| rule                   | ルール                 | Loan; matches GNU Make ja usage of ルール for the `rule` construct.                                                                                                                                                                                              |
| dependency             | 依存関係               | Native; used in [GNU Make 4.4 ja](https://www.ecoop.net/coop/translated/GNUMake4.4/ch04.jp.html) (that manual now prefers 前提条件, "prerequisite", but 依存関係 is the established general term).                                                               |
| order-only dependency  | 順序のみの依存関係     | Compound of 順序のみ ("order-only", per GNU Make ja) + 依存関係.                                                                                                                                                                                                 |
| build (noun)           | ビルド                 | Loan.                                                                                                                                                                                                                                                            |
| build (verb)           | ビルドする             | Loan + する verbaliser, standard pattern for loan verbs.                                                                                                                                                                                                         |
| build graph            | ビルドグラフ           | Loan compound; no established native calque found.                                                                                                                                                                                                               |
| phony target           | 疑似ターゲット         | Loan/calque blend; GNU Make ja also uses 偽りのターゲット, and フォニーターゲット (pure loan) appears in Japanese blog explanations.                                                                                                                             |
| artefact               | 成果物                 | Native; standard term for a build's output/deliverable in Japanese dev docs.                                                                                                                                                                                     |
| working directory      | 作業ディレクトリ       | Native; used in GNU Make 4.4 ja.                                                                                                                                                                                                                                 |
| workspace root         | ワークスペースのルート | Loan compound; ワークスペース is standard (e.g. Microsoft Fabric ja docs).                                                                                                                                                                                       |
| cache                  | キャッシュ             | Loan.                                                                                                                                                                                                                                                            |
| allowlist              | 許可リスト             | Calque; matches [Microsoft ja](https://learn.microsoft.com/ja-jp/defender-office-365/tenant-allow-block-list-urls-configure) and current inclusive-terminology guidance.                                                                                         |
| blocklist              | ブロックリスト         | Loan/calque blend; paired with 許可リスト per the same sources.                                                                                                                                                                                                  |
| template               | テンプレート           | Loan; long vowel retained per JTF rule 5.                                                                                                                                                                                                                        |
| macro                  | マクロ                 | Loan.                                                                                                                                                                                                                                                            |
| environment variable   | 環境変数               | Native; used in GNU Make 4.4 ja.                                                                                                                                                                                                                                 |
| exit status            | 終了ステータス         | Native; used verbatim in GNU Make 4.4 ja (`.DELETE_ON_ERROR` section); distinct from 終了コード ("exit code"), the term Microsoft's .NET/Windows docs use for a different, process-level concept.                                                                |
| stage (pipeline stage) | ステージ               | Loan.                                                                                                                                                                                                                                                            |
| locale                 | ロケール               | Loan.                                                                                                                                                                                                                                                            |
| placeable              | プレースホルダー       | Loan; no Fluent-specific Japanese term found — this is the general localization-industry term (e.g. [Phrase ja docs](https://support.phrase.com/hc/ja/articles/5822510498332-%E3%83%97%E3%83%AC%E3%83%BC%E3%82%B9%E3%83%9B%E3%83%AB%E3%83%80%E3%83%BC-Strings)). |

### Korean (`ko`)

Korean technical writing for developer tools uses the formal, impersonal polite
register, commonly called 하십시오체 (habsioche) or, by its verb ending,
합니다체. Sentences end in `-습니다`/`-ㅂ니다` for statements and `-십시오` for
instructions, and the text never adopts a first- or second-person
conversational stance. This is the register the
[Microsoft Localization Style Guide family](https://learn.microsoft.com/en-us/globalization/reference/microsoft-style-guides)
and community guidance such as the
[Unbabel Korean language guidelines](https://help.unbabel.com/hc/en-us/articles/360008767534-Language-Guidelines-Korean)
identify as standard for formal, professional, business and technical content,
as opposed to the more conversational 해요체 used in casual consumer-facing
copy. Netsuke's diagnostics and CLI help are exactly this kind of formal,
impersonal technical text, so every full-sentence message should use
하십시오체/합니다체 endings; short labels and table headers use bare noun forms
with no verb ending at all.

Several Netsuke terms stay as English loan words, written in Hangul according
to Korean loanword orthography (외래어 표기법): 빌드 (build), 캐시 (cache),
템플릿 (template), and 매크로 (macro) are all established loans in Korean
developer documentation, confirmed by Microsoft Learn Korean pages for
[environment variables](https://learn.microsoft.com/ko-kr/windows-hardware/drivers/devtest/binplace-macros-and-environment-variables)
and general usage across Korean-language build-tool docs. "Build" also
functions as a verb, 빌드하다. By contrast, `target`, `action`, and `rule` are
not loans: Bazel's official Korean documentation - a major developer tool with
the same target/rule/action model as Netsuke - translates them as
[규칙 for rule and 작업 for action](https://bazel.build/extending/rules?hl=ko)
("규칙은 Bazel이 입력에 대해 실행하여 출력 집합을 생성하는 일련의 **작업**을
정의합니다"), and as
[타겟 for target](https://bazel.build/rules/testing?hl=ko). This document
follows that precedent for consistency with the wider Korean build-tool
ecosystem.

Two hazards deserve attention. First, `dependency` sits on a known terminology
fault line: Microsoft's documentation consistently uses 종속성 (see
[.NET dependency injection](https://learn.microsoft.com/ko-kr/dotnet/core/extensions/dependency-injection/overview),
"종속성 주입"), and Bazel's official Korean docs independently agree, titling
the concept [종속 항목](https://bazel.build/concepts/dependencies?hl=ko) ("대상
A는 대상 B에 종속됩니다"). The FOSS/developer community more often prefers
의존성 (see the
[Korean Spring User Group discussion](https://groups.google.com/g/ksug/c/v9hYDfiXEwM)).
Because two independent major developer-tool sources (Microsoft, Bazel)
converge on 종속성, this section adopts 종속성 as the preferred term, but
의존성 should be recognized as a common synonym in community-authored Korean
docs. Second, `target`'s loanword spelling is contested: the
[National Institute of Korean Language's official transliteration is 타깃](https://x.com/urimal365/status/166399809903792128),
per the 외래어 표기법, while developers overwhelmingly write 타겟, and Bazel's
own Korean localization uses 타겟. This document follows the developer-tool
convention (타겟) rather than the prescriptive spelling, and flags the
divergence so reviewers are not surprised by a "non-standard" spelling. A
further mechanical hazard is 띄어쓰기 (word spacing) in compound loans and noun
phrases such as 작업 디렉터리 and 환경 변수: Korean requires a space between
the modifying noun and the head noun in these compounds, and dropping it
(작업디렉터리) is a common but incorrect contraction. Note also that 작업 is
used for both `action` (this table) and the general sense of "task/job"
elsewhere in Korean technical prose; Netsuke's Korean text should keep 작업 for
`action` only when the immediate context (a rule producing outputs) makes the
build-system sense unambiguous, and prefer 작업 항목 or rephrasing if a passage
could be misread as a generic task list.

The worked example, preserving the placeable exactly:

```text
{ $path }에서 매니페스트를 불러오는 데 실패했습니다.
```

Table 25: Korean terminology

| en-US                  | preferred        | notes                                                                                                                                                                                                           |
| ---------------------- | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | 매니페스트       | Loan; avoid 성명서 (political manifesto) or 적하목록 (shipping manifest) - both are false-friend senses in general Korean.                                                                                      |
| target                 | 타겟             | Developer-tool convention (Bazel Korean docs); NIKL prescribes 타깃 (외래어 표기법) - see [National Institute of Korean Language](https://x.com/urimal365/status/166399809903792128).                           |
| action                 | 작업             | Matches [Bazel's Korean usage](https://bazel.build/extending/rules?hl=ko); collides with the everyday "task/job" sense - keep context unambiguous.                                                              |
| rule                   | 규칙             | Matches [Bazel's Korean usage](https://bazel.build/extending/rules?hl=ko).                                                                                                                                      |
| dependency             | 종속성           | Preferred per Microsoft and Bazel Korean docs; 의존성 is a common FOSS-community alternative, see fault-line note above.                                                                                        |
| order-only dependency  | 순서 전용 종속성 | No fixed precedent found; compositional calque of 종속성, glossed on first use.                                                                                                                                 |
| build (noun)           | 빌드             | Established loan.                                                                                                                                                                                               |
| build (verb)           | 빌드하다         | Loan + native verb suffix -하다.                                                                                                                                                                                |
| build graph            | 빌드 그래프      | Loan compound; space between 빌드 and 그래프.                                                                                                                                                                   |
| phony target           | 가짜 타겟        | Descriptive calque (가짜 = "fake"); no single fixed term found in Korean make/ninja discussion.                                                                                                                 |
| artefact               | 산출물           | Native term, standard in Korean build/CI documentation for build outputs.                                                                                                                                       |
| working directory      | 작업 디렉터리    | Microsoft spells 디렉터리, not 디렉토리; see [Microsoft .NET Korean docs](https://learn.microsoft.com/ko-kr/dotnet/api/system.io.directory.getcurrentdirectory?view=net-10.0). Space required: 작업 디렉터리.   |
| workspace root         | 작업 영역 루트   | 작업 영역 for workspace, 루트 as established loan for filesystem root.                                                                                                                                          |
| cache                  | 캐시             | Established loan.                                                                                                                                                                                               |
| allowlist              | 허용 목록        | Compositional native term, standard in Korean security/cloud docs.                                                                                                                                              |
| blocklist              | 차단 목록        | Compositional native term, parallel to 허용 목록.                                                                                                                                                               |
| template               | 템플릿           | Established loan.                                                                                                                                                                                               |
| macro                  | 매크로           | Established loan; see [Microsoft Korean docs](https://learn.microsoft.com/ko-kr/windows-hardware/drivers/devtest/binplace-macros-and-environment-variables).                                                    |
| environment variable   | 환경 변수        | Established compound; see [Microsoft Korean docs](https://learn.microsoft.com/ko-kr/azure/databricks/jobs/environment-variables). Space required.                                                               |
| exit status            | 종료 상태        | Matches [Korean Wikipedia](https://ko.wikipedia.org/wiki/%EC%A2%85%EB%A3%8C_%EC%83%81%ED%83%9C) and Microsoft C++ docs; 종료 코드 (exit code) is a close synonym, keep 종료 상태 for the POSIX sense.           |
| stage (pipeline stage) | 단계             | Native term, standard for pipeline/process stages; 스테이지 loan is used only for CI-platform-specific UI labels.                                                                                               |
| locale                 | 로캘             | Standard Korean i18n term (as opposed to 로케일, a less common variant).                                                                                                                                        |
| placeable              | 자리표시자       | Descriptive calque after MDN Korean's [자리 표시자 for "placeholder"](https://developer.mozilla.org/ko/docs/Glossary/Placeholder_names); no fixed Fluent-specific Korean term found - flagged as a coined term. |

### Norwegian Bokmål (`nb`)

Bokmål technical writing addresses the reader with the informal second person
`du`; the formal `De` is obsolete outside archaic or ceremonial registers. The
[Microsoft Norwegian (Bokmål) style guide](https://aka.ms/Norwegian-bokmal-styleguide)
instructs translators to "address the user directly, using the second-person
pronoun" (`du`), and its worked examples use `du` throughout even for warnings
and confirmations. Diagnostics and CLI help for Netsuke should follow the same
convention: direct, calm, `du`-form sentences, with no politeness marking
beyond that.

Most Netsuke vocabulary translates rather than borrows. `mal` (template),
`makro` (macro), `avhengighet` (dependency), and `miljøvariabel` (environment
variable) are all long established in Norwegian developer and
desktop-localization writing, confirmed by the
[Skolelinux/l10n.no common data-term glossary](https://l10n.no/nb/Fellesordl.eng-no.html).
`manifest`, by contrast, is a genuine loan word written exactly as in English
(neuter gender, `et manifest`), as seen in Norwegian Kubernetes and
cloud-native writing; Norwegian already uses `manifest` for a political
manifesto and a shipping manifest, so the software sense adds a fourth reading
rather than colliding with a false friend. `pipeline` is likewise commonly
loaned in current Norwegian DevOps prose (e.g. "CI/CD-pipeline"), even though
older Microsoft material translates it as `kommandokø`; Netsuke's own
vocabulary does not need the word `pipeline` itself, only `stage`, which does
translate (`fase`). `cache` is not treated as a bare loan in careful technical
Bokmål: Microsoft's software localization consistently renders it
`hurtigbuffer` (seen even for software-level caches, not just hardware caches),
which this table follows; the FOSS glossary's `mellomlager` is a legitimate
alternative worth recognizing in the notes.

Two hazards matter most. First, Norwegian compounds are written solid, with no
space and often a linking `-s-` or `-e-` (særskriving — writing an English open
compound as two Norwegian words — is a recognized style error per the Microsoft
guide's compounding section). `arbeidskatalog` risks this: Bokmål convention
(confirmed by both the l10n.no glossary and general desktop usage) uses one
word, `mappe`, for both "folder" and "directory", so "working directory" is
`arbeidsmappe`, never a two-word or `katalog`-based calque. Second, `mål`
(target) is an extremely common everyday word meaning "goal", "measure", or
"language/dialect" as well as the build-system sense; this table uses it only
because Microsoft and GNOME translation memories (e.g. `drop target` →
`slippmål`) establish it as the standard rendering, but authors should keep
target/action/rule (`mål`/`handling`/`regel`) visibly distinct in running prose
to avoid the reader conflating "mål" as target-the-noun with "mål" as
goal-the-concept.

```text
Kunne ikke laste manifestet fra { $path }.
```

Sources:

- [Microsoft Norwegian (Bokmål) style guide](https://aka.ms/Norwegian-bokmal-styleguide)
- [Skolelinux/l10n.no Fellesordliste for dataord på bokmål](https://l10n.no/nb/Fellesordl.eng-no.html)
- [GNOME Norwegian Bokmål translation team](https://l10n.gnome.org/languages/nb/)
- Microsoft nb-NO product help for `hurtigbuffer` (e.g.
  [support.microsoft.com SMB2 article](https://support.microsoft.com/nb-no/servicing/os/windows-server/2018/09/data-corruption-when-multiple-users-perform-read-and-write-operations-to-a-shared-file-in-the-smb2-e))
- nb-NO allowlist/blocklist usage in
  [OpenAI API IP allowlisting help](https://help.openai.com/nb-no/articles/20001201-ip-allowlisting-for-openai-api)
  and
  [Dell removable-media allowlist guidance](https://www.dell.com/support/kbdoc/no-no/000131004/dell-kryptering-ekstern-media-dell-data-beskyttelse-ekstern-media-utgave-hvitlisting-veiledning)

Table 26: Norwegian Bokmål terminology

| en-US                  | preferred             | notes                                                                                                                                  |
| ---------------------- | --------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest              | Loan word, neuter (`et manifest`); confirmed in Norwegian Kubernetes/cloud-native usage.                                               |
| target                 | mål                   | Distinct from `handling` (action) and `regel` (rule); Microsoft/GNOME memory (`drop target` = `slippmål`).                             |
| action                 | handling              | Standard Norwegian rendering of "action" in software UI; confirmed in Norwegian data-term glossaries and GNOME strings.                |
| rule                   | regel                 | Generic, well-established (`sorteringsregel` = sorting rule). Kept distinct from `mål`/`handling`.                                     |
| dependency             | avhengighet           | [l10n.no glossary](https://l10n.no/nb/Fellesordl.eng-no.html).                                                                         |
| order-only dependency  | rekkefølgeavhengighet | Calque, solid compound; no established Norwegian dev-tool source found — coin transparently rather than loan.                          |
| build (noun)           | bygg                  | Naturalised via Visual Studio nb-NO (`Bygg løsning` = Build Solution); `et bygg` also used for CI build artefacts context.             |
| build (verb)           | bygge                 | Same source as above; imperative `Bygg` seen in Visual Studio menus.                                                                   |
| build graph            | byggegraf             | Calque of `bygg` + `graf`; no attested prior use, but transparent and solidly compounded.                                              |
| phony target           | fiktivt mål           | Calque; no established translation for this Ninja-specific concept, kept consistent with `mål`.                                        |
| artefact               | artefakt              | Confirmed in Norwegian CI/CD discussion (`byggeartefakt`, `en artefakt`).                                                              |
| working directory      | arbeidsmappe          | `mappe`, not `katalog`, per l10n.no convention of using one word for folder/directory.                                                 |
| workspace root         | arbeidsområderot      | Solid compound of `arbeidsområde` (workspace, l10n.no) + `rot` (root); avoids særskriving.                                             |
| cache                  | hurtigbuffer          | Microsoft's consistent nb-NO rendering, including for software caches; `mellomlager` (FOSS/l10n.no) is a recognized alternative.       |
| allowlist              | tillatelsesliste      | Confirmed across multiple nb-NO vendor help pages (OpenAI, Dell, Salesforce, Webex).                                                   |
| blocklist              | blokkeringsliste      | Confirmed in nb-NO security documentation (Norton, Entra ID compliance tooling).                                                       |
| template               | mal                   | [l10n.no glossary](https://l10n.no/nb/Fellesordl.eng-no.html).                                                                         |
| macro                  | `makro`               | Direct orthographic loan, standard in Norwegian dictionaries and l10n.no.                                                              |
| environment variable   | miljøvariabel         | [l10n.no glossary](https://l10n.no/nb/Fellesordl.eng-no.html).                                                                         |
| exit status            | avslutningsstatus     | Combines l10n.no's listed `avslutnings-` prefix with `-status` suffix option.                                                          |
| stage (pipeline stage) | `fase`                | Confirmed in Norwegian CI/CD discussion of Jenkins pipeline stages.                                                                    |
| locale                 | `lokale`              | [l10n.no glossary](https://l10n.no/nb/Fellesordl.eng-no.html) lists `lokale` first among options.                                      |
| placeable              | plassholder           | Adapted from l10n.no's `placeholder` = `plassholder`; no Fluent-specific nb-NO term found, so the nearest established concept is used. |

### Dutch (`nl`)

Netsuke's Dutch strings should address the reader with the formal pronoun "u"
rather than the informal "je". Microsoft's Dutch localization guidance notes
that products historically used "u" and that "je" has only grown common in
consumer software, adding that technical texts favour a formal, informative tone
([Microsoft Dutch style guide](https://aka.ms/dutch-styleguide), §2.2.1 and
§5.6.2). Established Dutch technical writing for developers, such as the Dutch
translation of Karl Fogel's *Producing Open Source Software*, likewise uses "u"
throughout
([Dutch edition](https://producingoss.com/nl/open-source-software-produceren.pdf)).
Netsuke's audience is developers and CI operators reading diagnostics, so "u"
(or an impersonal construction where "u" would be awkward, e.g. passive voice
in status output) is the safer default; a future decision to target a more
casual audience could revisit "je", but nothing in current guidance supports it
for build-tool diagnostics.

Several Netsuke terms stay as English loans in Dutch developer writing: "build"
(noun; "samenstelling" is not used), "cache", "macro", and "locale" in
technical contexts. This matches attested usage such as Debian's Dutch FAQ,
which keeps "Build-Dependencies" and pairs it with the native verb "bouwen"
([Debian FAQ, nl](https://www.debian.org/doc/manuals/debian-faq/debian-faq.nl.txt)),
and Microsoft's SQL Server documentation, which uses "een locale" directly as
a loan noun in developer-facing text
([Microsoft Learn, Collation and Unicode support, nl-NL](https://learn.microsoft.com/nl-nl/sql/relational-databases/collations/collation-and-unicode-support?view=sql-server-ver17)).
All loans are written in the Latin script Dutch already uses, so no script
switch is needed.

The main hazard is "de Engelse ziekte" ("the English disease"): writing Dutch
compounds as separate words under English influence, e.g. "build graaf" instead
of the solid compound "buildgraaf", or "werk map" instead of "werkmap". Netsuke
strings and their translations must keep compounds solid. A second hazard is
"manifest": besides Netsuke's build manifest, the same Dutch word means a
political manifesto and, in shipping and customs contexts, a cargo manifest
("scheepsmanifest"); it can also function as an adjective meaning "evident".
Diagnostics naming a manifest file should keep enough surrounding context (a
path, an extension) that the sense is unambiguous. A third hazard is "regel",
the preferred term for a Ninja rule: the same word is the everyday Dutch word
for a "line" (of text), so error messages that mention both a rule and a line
number should name each explicitly rather than relying on "regel" alone. The
worked example:

```text
Kon het manifest op { $path } niet laden.
```

Table 27: Dutch terminology

| en-US                  | preferred                     | notes                                                                                                                                                                                                                                                           |
| ---------------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest                      | Same spelling; false friend with "manifest" (political manifesto, or the adjective "evident") and "scheepsmanifest" (cargo document).                                                                                                                           |
| target                 | doel                          | Build output or named build entry; kept distinct from action and rule.                                                                                                                                                                                          |
| action                 | `actie`                       | Implicitly phony target.                                                                                                                                                                                                                                        |
| rule                   | regel                         | Also the everyday word for "line" (of text) — disambiguate near line numbers.                                                                                                                                                                                   |
| dependency             | afhankelijkheid               | Standard term; cf. Debian's "Build-Dependencies" → afhankelijkheden.                                                                                                                                                                                            |
| order-only dependency  | ordergebonden afhankelijkheid | Calque; no established Dutch precedent found.                                                                                                                                                                                                                   |
| build (noun)           | build                         | Established loan; "samenstelling" is not used.                                                                                                                                                                                                                  |
| build (verb)           | bouwen                        | "Compileren" for the compile step specifically.                                                                                                                                                                                                                 |
| build graph            | buildgraaf                    | Loan + native compound, written solid — avoid "build graaf" (Engelse ziekte).                                                                                                                                                                                   |
| phony target           | schijndoel                    | Calque ("schijn" = sham); no established term found.                                                                                                                                                                                                            |
| artefact               | artefact                      | Dutch spelling of `artifact`; standard IT usage.                                                                                                                                                                                                                |
| working directory      | werkmap                       | [Microsoft Learn, PowerShell nl-NL](https://learn.microsoft.com/nl-nl/powershell/module/microsoft.powershell.core/about/about_environment_variables?view=powershell-7.6); [Linguee](https://www.linguee.nl/engels-nederlands/vertaling/working+directory.html). |
| workspace root         | hoofdmap van de werkruimte    | Analytic form preferred over a dense compound for clarity.                                                                                                                                                                                                      |
| cache                  | cache                         | Established loan, "de cache"; "buffergeheugen" is archaic.                                                                                                                                                                                                      |
| allowlist              | acceptatielijst               | [Microsoft Dutch style guide](https://aka.ms/dutch-styleguide), inclusive-language table.                                                                                                                                                                       |
| blocklist              | blokkeringslijst              | Same source as allowlist.                                                                                                                                                                                                                                       |
| template               | sjabloon                      | [Microsoft Learn nl-NL](https://learn.microsoft.com/nl-nl/cli/azure/acr?view=azure-cli-latest); [Lenovo glossary, nl](https://www.lenovo.com/nl/nl/glossary/template/).                                                                                         |
| macro                  | macro                         | Established loan/cognate.                                                                                                                                                                                                                                       |
| environment variable   | omgevingsvariabele            | [Microsoft Learn, PowerShell nl-NL](https://learn.microsoft.com/nl-nl/powershell/module/microsoft.powershell.core/about/about_environment_variables?view=powershell-7.6).                                                                                       |
| exit status            | exitstatus                    | One word; attested in Dutch shell/Bash guides.                                                                                                                                                                                                                  |
| stage (pipeline stage) | `fase`                        | General Dutch process term; some CI writing keeps "stage" as a loan.                                                                                                                                                                                            |
| locale                 | locale                        | Developer-facing loan; Microsoft's UI-facing term is "landinstelling", but technical docs use "locale" directly (see collation source above).                                                                                                                   |
| placeable              | tijdelijke aanduiding         | Adapted from Microsoft's term for "placeholder" ([Linguee](https://www.linguee.nl/engels-nederlands/vertaling/placeholder.html)); no Fluent-specific Dutch precedent found.                                                                                     |

### Polish (`pl`)

Netsuke's Polish diagnostics and CLI help favour impersonal constructions over
direct address. The
[Microsoft Polish Localization Style Guide](https://aka.ms/polish-styleguide)
recommends second-person informal ("Ty", capitalized) when the product guides
the user through an action, but its own error-message examples avoid that
address form entirely, preferring impersonal phrasing such as "Nie można
odnaleźć pobranych plików…" ("The downloaded files cannot be found…"). The
[Sailfish OS Polish style guide](https://docs.sailfishos.org/Develop/L10n/Style_Guides/Polish/)
and [GNOME/Aviary.pl translators](https://pl.wikipedia.org/wiki/Aviary.pl)
similarly default to the informal singular ("ty") for interactive prompts but
keep status and error text impersonal. Because Netsuke's user-facing text is
diagnostics, CLI help, and status output for developers and CI operators rather
than interactive dialogue, this section uses impersonal, subjectless
constructions throughout ("nie znaleziono", "nie można wczytać") and reserves
second-person informal verbs only for imperative CLI help text (e.g. "Uruchom
`netsuke build`").

Several Netsuke terms stay as English loan words in Polish technical writing:
`build` is conventionally rendered as *kompilacja* (noun) in Microsoft's own
DevOps documentation, but the bare loan *build* remains common in spoken and
informal Polish DevOps usage; this table uses *kompilacja* for consistency with
Azure Pipelines precedent. `cache`, `template`, and `macro` all have
long-established native equivalents (*pamięć podręczna*, *szablon*, `makro`)
and are not left as loans. Netsuke Rust identifiers, filenames, and CLI flags
(`Netsukefile`, `--verbose`, `stdout`) stay in Latin script and are not
inflected; when Polish grammar would demand a case ending, the identifier is
set in fixed-width or code formatting and left in the nominative, with the
required case expressed by a preceding or following Polish word instead of a
suffix (e.g. "w pliku `Netsukefile`" rather than *Netsukefile'u*). All loan
terms and identifiers are written in the Latin script Netsuke already uses;
Polish introduces no additional script hazard.

Two hazards stand out. First, *manifest* is a false friend in the everyday
sense (a political manifesto), but Polish IT usage already borrows *plik
manifestu* for Android and other software manifests, so the technical sense is
unambiguous in context. Second, GNU Make's Polish translation, by Jakub Bogusz
([the PO file](https://translationproject.org/PO-files/pl/make-4.4.0.90.pl.po)),
renders *target* as *obiekt* (object) or *obiekt docelowy* (target object),
never as *cel*, because *cel* is reserved for "goal" (`.DEFAULT_GOAL` → *cel
domyślny*). Reusing *cel* for Netsuke's `target` would collide with that
goal/default-goal sense and should be avoided.

Worked example, preserving the placeable exactly:

```text
Nie można wczytać manifestu w lokalizacji { $path }.
```

Table 28: Polish terminology

| en-US                  | preferred                           | notes                                                                                                                                                                             |
| ---------------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest (plik manifestu)           | Loan; false friend with "political manifesto" resolved by context. See worked example.                                                                                            |
| target                 | obiekt docelowy                     | Follows GNU Make's Polish translation (*obiekt*/*obiekt docelowy*), not *cel* (reserved for "goal"). [make.pl.po](https://translationproject.org/PO-files/pl/make-4.4.0.90.pl.po) |
| action                 | działanie                           | Distinct from *akcja* (stock/share, campaign) to avoid ambiguity; distinct from *reguła* and *obiekt docelowy*.                                                                   |
| rule                   | reguła                              | Consistent with GNU Make's Polish translation throughout.                                                                                                                         |
| dependency             | zależność                           | Matches GNU Make's *prerequisite* → *zależność*.                                                                                                                                  |
| order-only dependency  | zależność tylko od kolejności       | Calques Make's "order-only prerequisite" phrasing used in the same PO file.                                                                                                       |
| build (noun)           | kompilacja                          | Matches Azure Pipelines/DevOps Polish docs; bare loan "build" also heard informally.                                                                                              |
| build (verb)           | kompilować / budować                | *Kompilować* for compiling manifests to a build graph; *budować* acceptable for the overall run.                                                                                  |
| build graph            | graf kompilacji                     | *Graf* is the standard Polish graph-theory loan; no established build-tool precedent found.                                                                                       |
| phony target           | obiekt niejawny                     | Direct precedent from GNU Make's Polish translation (`.PHONY` → *obiekt niejawny*).                                                                                               |
| artefact               | artefakt                            | Standard IT loan, unambiguous.                                                                                                                                                    |
| working directory      | katalog roboczy                     | Well established, confirmed in technical PL–EN glossaries.                                                                                                                        |
| workspace root         | katalog główny obszaru roboczego    | *Obszar roboczy* is VS Code's Polish term for "workspace"; *przestrzeń robocza* also seen (Jira, Vitest) but less common in dev tooling.                                          |
| cache                  | pamięć podręczna                    | Standard, no loan used.                                                                                                                                                           |
| allowlist              | lista dozwolonych                   | Confirmed via [Microsoft Defender docs](https://learn.microsoft.com/pl-pl/defender-office-365/tenant-allow-block-list-email-spoof-configure).                                     |
| blocklist              | lista zablokowanych                 | Same source; paired consistently with *lista dozwolonych*.                                                                                                                        |
| template               | szablon                             | Standard, no loan used.                                                                                                                                                           |
| macro                  | `makro`                             | Standard, no loan used.                                                                                                                                                           |
| environment variable   | zmienna środowiskowa                | Confirmed across multiple Microsoft Learn PL pages (vcpkg, PowerShell, Power Platform).                                                                                           |
| exit status            | kod zakończenia                     | Confirmed via Microsoft Learn PL (PowerShell, xcopy, Service Fabric docs); *kod wyjścia* also seen but less consistent.                                                           |
| stage (pipeline stage) | etap                                | Matches Azure Pipelines Polish "Etapy" for pipeline stages.                                                                                                                       |
| locale                 | ustawienia regionalne / lokalizacja | *Ustawienia regionalne* for the language/region setting; *lokalizacja* only when referring to the localization process, to avoid confusion with "location".                       |
| placeable              | obiekt do podstawienia              | No established Polish Fluent glossary found; descriptive calque chosen over a bare loan since the concept is not widely documented in Polish.                                     |

Sources consulted:

- [Microsoft Polish Localization Style Guide](https://aka.ms/polish-styleguide)
- [Sailfish OS Polish style guide](https://docs.sailfishos.org/Develop/L10n/Style_Guides/Polish/)
- [Aviary.pl (Wikipedia overview of the Polish GNOME/Mozilla translation group)](https://pl.wikipedia.org/wiki/Aviary.pl)
- [GNU Make Polish translation, translationproject.org](https://translationproject.org/PO-files/pl/make-4.4.0.90.pl.po)
- [Microsoft Learn: Azure Pipelines "Create your first pipeline" (pl-pl)](https://learn.microsoft.com/pl-pl/azure/devops/pipelines/create-first-pipeline?view=azure-devops)
- [Microsoft Learn: Azure Pipelines "Stages" (pl-pl)](https://learn.microsoft.com/pl-pl/azure/devops/pipelines/process/stages?view=azure-devops)
- [Microsoft Learn: Defender for Office 365 allow/block list (pl-pl)](https://learn.microsoft.com/pl-pl/defender-office-365/tenant-allow-block-list-email-spoof-configure)
- [Microsoft Learn: about_PowerShell_exe (pl-pl)](https://learn.microsoft.com/pl-pl/powershell/module/microsoft.powershell.core/about/about_powershell_exe?view=powershell-5.1)
- [Microsoft Learn: xcopy exit codes (pl-pl)](https://learn.microsoft.com/pl-pl/windows-server/administration/windows-commands/xcopy)
- [Microsoft Learn: vcpkg environment variables (pl-pl)](https://learn.microsoft.com/pl-pl/vcpkg/users/config-environment)

### Portuguese, Brazil (`pt-BR`)

Brazilian Portuguese technical writing addresses the reader directly with the
second-person pronoun "você" (frequently left implicit and carried by verb
inflection), in an informal-but-professional register that avoids both
excessive formality and slang. This is Microsoft's documented "Microsoft voice"
guidance for pt-BR: it explicitly favours direct second-person address over
impersonal or passive constructions, and lists "unnecessarily formal" phrasing
(e.g. "`ser` capaz de" for "to be able to") as something to avoid in favour of
the plain equivalent ("poder"). See the
[Microsoft Portuguese (Brazil) style guide](https://aka.ms/portuguese-brazil-styleguide).
Netsuke diagnostics should therefore read as direct, calm statements to "você"
rather than as impersonal notices, matching the house style's plain,
professional tone.

Several Netsuke terms stay as English loans in Brazilian technical writing:
`build` (o build) is the normal noun in Brazilian developer prose, and `cache`
(o cache) is likewise unchanged, per the
[Drupal pt-BR translator's glossary](https://localize.drupal.org/node/664),
which lists `cache` as a retained loan while translating `template` as "modelo".
`macro` is effectively a native word (a macro) and needs no adaptation. By
contrast, `allowlist` and `blocklist` have settled Brazilian calques rather
than staying as loans: Microsoft Learn's pt-BR documentation consistently
renders them as "lista de permissões" and "lista de bloqueio" (see, e.g.,
[Azure IP access lists, pt-BR](https://learn.microsoft.com/pt-br/azure/databricks/security/network/front-end/ip-access-list)).
All English identifiers and product names (Netsuke, Ninja, Fluent, Jinja,
YAML, stdout, stderr, UTF-8, dyndep, foreach, vars, command_available) are left
invariant; Portuguese has no casing or bidirectional-script traps for these,
but plural loanwords take a regular Portuguese "-s" (builds, caches), which can
look unfamiliar to readers expecting invariant English plurals.

The main hazard is `target`: general-purpose Portuguese dictionaries and
Microsoft's own generic terminology often render "target" as "destino" (as seen
throughout Microsoft's localization-product documentation, e.g.
[Azure AI video translation docs](https://learn.microsoft.com/pt-br/rest/api/aiservices/videotranslation/translation-operations/get-translation)),
but that sense is about a target *language* or *destination*, not a build
target. Brazilian build-tool and systems documentation instead uses "alvo" for
this sense, as seen in the Debian/Arch pt_BR manual pages for `ln(1)` and
`alpm-hooks(5)` and in Godot's pt-BR docs. Netsuke therefore uses "alvo" for
`target`, reserving "destino" for genuinely destination-flavoured concepts if
any arise. A second hazard is `manifest`: "manifesto" is a faithful cognate,
but in general Portuguese "manifesto" primarily means a political manifesto, so
diagnostics should keep enough surrounding context (e.g. "manifesto do
Netsuke") that readers do not misread it as a political or declarative
document. Finally, `build` (verb) should not be translated as "compilar":
Netsuke's build graph runs arbitrary actions, not just compilation, so
"compilar" would misrepresent what Netsuke does; "gerar" is used instead for
the verb, reserving "compilar" for genuinely compiling actions.

Worked example, preserving the placeable `{ $path }` exactly:

```text
Falha ao carregar o manifesto em { $path }.
```

Table 29: Portuguese, Brazil terminology

| en-US                  | preferred                   | notes                                                                                                                                                                                     |
| ---------------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifesto                   | Faithful cognate; false friend with "manifesto" (political); keep context, e.g. "manifesto do Netsuke".                                                                                   |
| target                 | alvo                        | Build-tool convention ([Debian/Arch manpages](https://man.archlinux.org/man/ln.1.pt_BR), Godot pt-BR docs); Microsoft's generic "destino" is for localization targets, not build targets. |
| action                 | ação                        | Distinct from alvo/regra to preserve Netsuke's three-way distinction.                                                                                                                     |
| rule                   | regra                       | Standard translation of the Make/Ninja "rule" concept.                                                                                                                                    |
| dependency             | dependência                 | Standard term across Brazilian dev docs.                                                                                                                                                  |
| order-only dependency  | dependência apenas de ordem | Coined calque; no established pt-BR term found for this Ninja-specific concept — verify before shipping widely.                                                                           |
| build (noun)           | build                       | Loan, "o build" ([Drupal pt-BR glossary](https://localize.drupal.org/node/664)); "compilação" avoided since Netsuke builds are not strictly compilation.                                  |
| build (verb)           | gerar                       | Avoids "compilar", which would misstate that every build compiles; "gerar o build" is the verb form used.                                                                                 |
| build graph            | grafo de build              | Calque "grafo de" + retained loan "build", consistent with the noun row.                                                                                                                  |
| phony target           | alvo fictício               | Calque built from alvo + "fictício"; no directly attested Brazilian source located — verify against user feedback.                                                                        |
| artefact               | artefato                    | Standard Brazilian spelling (no "c" as in pt-PT "artefacto").                                                                                                                             |
| working directory      | diretório de trabalho       | Standard, unambiguous.                                                                                                                                                                    |
| workspace root         | raiz do espaço de trabalho  | "workspace" = "espaço de trabalho" per seed term list; "raiz" is the standard qualifier for a root path.                                                                                  |
| cache                  | cache                       | Loan, "o cache" ([Drupal pt-BR glossary](https://localize.drupal.org/node/664)).                                                                                                          |
| allowlist              | lista de permissões         | Established Microsoft Learn pt-BR calque, not a loan.                                                                                                                                     |
| blocklist              | lista de bloqueio           | Established Microsoft Learn pt-BR calque, not a loan.                                                                                                                                     |
| template               | modelo                      | Microsoft/Drupal glossary convention; informal Brazilian dev speech often says "template", but "modelo" is used for consistency with formal docs.                                         |
| macro                  | macro                       | Native word, unchanged spelling; feminine gender ("a macro").                                                                                                                             |
| environment variable   | variável de ambiente        | Standard, unambiguous.                                                                                                                                                                    |
| exit status            | status de saída             | Attested in Brazilian dev Q&A and vendor docs (e.g. [IBM AIX pt-BR docs](https://www.ibm.com/docs/pt-br/aix/7.2.0?topic=w-who-command)); "código de saída" is a common near-synonym.      |
| stage (pipeline stage) | estágio                     | Attested in Brazilian CI/CD docs (GitLab/Oracle/UiPath pt-BR usage); "etapa" is a valid but less pipeline-specific alternative.                                                           |
| locale                 | localidade                  | Consistent with Microsoft Learn pt-BR usage for locale/target-locale concepts.                                                                                                            |
| placeable              | placeable                   | Retained as an English technical loan (Fluent-specific jargon); no established pt-BR translation found — verify before shipping widely.                                                   |

### Portuguese, Portugal (`pt-PT`)

European Portuguese technical writing avoids the second-person pronoun "você",
which can read as blunt or regionally marked (it is standard in Brazilian
Portuguese but avoided in Portugal); Microsoft's own guidance says "você"
should be sidestepped by rephrasing, including with the passive voice, and
recommends addressing the user directly only through the imperative for
instructions and impersonal or third-person constructions elsewhere
([Microsoft Portuguese (Portugal) style guide](https://aka.ms/portuguese-portugal-styleguide)).
Netsuke's messages are diagnostics and status reports rather than
instructions, so this section uses impersonal, subjectless constructions ("Não
foi possível carregar…") that state the condition without addressing "you" at
all. Continuous or in-progress operations follow the same guide's rule for the
English gerund: render it as "a" plus the infinitive rather than a Portuguese
gerund, e.g. "A compilar…" for "Building…".

Several Netsuke terms stay as English loanwords in European Portuguese
technical prose: `cache` (feminine, written in italics per Priberam and the
[translatewiki.net Portuguese cheatsheet](https://translatewiki.net/wiki/Portal:Pt/Cheatsheet/pt))
and `macro` (feminine, unchanged). Product and syntax identifiers (Netsuke,
Ninja, Fluent, Jinja, YAML, `stdout`, `stderr`, `dyndep`) are invariant and
carry no diacritics or gender marking.

The clearest hazard is "manifesto": Portuguese tech translations do use
"manifesto" as a calque for "manifest file" (see the
[AWS glossary](https://docs.aws.amazon.com/pt_br/glossary/latest/reference/glos-chap.html)),
but in everyday Portuguese "manifesto" primarily means a political or
ideological manifesto, so the term must always appear with a qualifying noun
("o manifesto do Netsuke" or "ficheiro de manifesto") to avoid ambiguity. A
second hazard is spelling drift from the Acordo Ortográfico de 1990 (AO90):
Portugal dropped silent consonants that Brazil had already dropped decades
earlier, so pre-2009 Portuguese sources still show "acção" and "directório"
where current usage requires "ação" and "diretório"
([Wikipedia: Acordo Ortográfico de 1990](https://pt.wikipedia.org/wiki/Acordo_Ortogr%C3%A1fico_de_1990));
words where the consonant is still pronounced, such as "artefacto", keep it. A
third hazard is "alvo" versus "destino" for `target`: general localization
glossaries such as translatewiki.net explicitly prefer "destino" over "alvo"
for generic senses of "target", but build-tool translations (the community
Portuguese translation of the GNU Make manual, and KDE's Kate/CMake build
settings) consistently use "alvo" for a build target. Netsuke follows the
build-tool convention.

Worked example, translating `Failed to load manifest at { $path }.`:

```text
Não foi possível carregar o manifesto em { $path }.
```

Table 30: Portuguese, Portugal terminology

| en-US                  | preferred                   | notes                                                                                                                                                                                                                                                                       |
| ---------------------- | --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifesto                   | False friend: also means a political manifesto in general use; qualify as "manifesto do Netsuke" or "ficheiro de manifesto" ([AWS glossary](https://docs.aws.amazon.com/pt_br/glossary/latest/reference/glos-chap.html)).                                                   |
| target                 | alvo                        | Build-tool domain term, not the generic "destino" preferred by [translatewiki.net](https://translatewiki.net/wiki/Portal:Pt/Cheatsheet/pt); matches the [GNU Make Portuguese manual](https://www.ecoop.net/coop/translated/GNUMake4.4/ch09.pt.html) and KDE build settings. |
| action                 | ação                        | AO90 spelling (not "acção"); distinct word from "alvo" and "regra" per Netsuke's three-way distinction.                                                                                                                                                                     |
| rule                   | regra                       | Matches the [GNU Make Portuguese manual](https://www.ecoop.net/coop/translated/GNUMake4.4/ch09.pt.html) ("regra").                                                                                                                                                          |
| dependency             | dependência                 | Standard software term.                                                                                                                                                                                                                                                     |
| order-only dependency  | dependência apenas de ordem | Compositional; no established Portuguese precedent found for this Ninja-specific concept.                                                                                                                                                                                   |
| build (noun)           | compilação                  | Use "build" as an unmarked loan only when referring to a specific numbered build artefact, not the process.                                                                                                                                                                 |
| build (verb)           | compilar                    | "Gerar" is acceptable when the build does not compile source code, mirroring the same ambiguity in English.                                                                                                                                                                 |
| build graph            | grafo de compilação         | Compositional from verified "grafo" (graph) and "compilação" (build).                                                                                                                                                                                                       |
| phony target           | alvo `falso`                | Direct match with the [GNU Make Portuguese manual](https://www.ecoop.net/coop/translated/GNUMake4.4/ch09.pt.html), "Alvos falsos (Phony)".                                                                                                                                  |
| artefact               | artefacto                   | AO90 keeps the pronounced "c"; European "artefacto" versus Brazilian "artefato".                                                                                                                                                                                            |
| working directory      | diretório de trabalho       | AO90 spelling "diretório", not "directório"; confirmed in the [Arch Wiki (Português)](https://wiki.archlinux.org/title/Environment_variables_(Portugu%C3%AAs)).                                                                                                             |
| workspace root         | raiz do espaço de trabalho  | "Espaço de trabalho" confirmed in [Microsoft Learn pt-PT](https://learn.microsoft.com/pt-pt/visualstudio/releases/2026/release-notes).                                                                                                                                      |
| cache                  | cache                       | Loanword, feminine, conventionally italicized ([translatewiki.net](https://translatewiki.net/wiki/Portal:Pt/Cheatsheet/pt); Priberam).                                                                                                                                      |
| allowlist              | lista de permissões         | Confirmed in Citrix and Chrome Enterprise Portuguese documentation.                                                                                                                                                                                                         |
| blocklist              | lista de bloqueios          | Confirmed in Chrome Enterprise Portuguese documentation.                                                                                                                                                                                                                    |
| template               | modelo                      | [translatewiki.net](https://translatewiki.net/wiki/Portal:Pt/Cheatsheet/pt): "o modelo"; note MediaWiki uses "predefinição" for its own wiki-template feature, which does not apply here.                                                                                   |
| macro                  | macro                       | Loanword, feminine, unchanged.                                                                                                                                                                                                                                              |
| environment variable   | variável de ambiente        | Standard term, confirmed in the [Arch Wiki (Português)](https://wiki.archlinux.org/title/Environment_variables_(Portugu%C3%AAs)) and the GNU Make Portuguese manual.                                                                                                        |
| exit status            | código de saída             | Confirmed across multiple [Microsoft Learn pt-PT](https://learn.microsoft.com/pt-pt/powershell/module/microsoft.powershell.core/about/about_powershell_exe) pages; prefer this over the loan "status" used loosely in some informal Portuguese sources.                     |
| stage (pipeline stage) | `fase`                      | Standard pipeline vocabulary; "etapa" is an acceptable synonym.                                                                                                                                                                                                             |
| locale                 | definições regionais        | Confirmed in [Microsoft Support pt-PT](https://support.microsoft.com/pt-pt/teams/notifications-settings/change-settings-in-microsoft-teams) ("Definições regionais").                                                                                                       |
| placeable              | marcador de posição         | No Fluent-specific Portuguese precedent found; adapted from Microsoft's established term for "placeholder" ([Microsoft Support pt-PT](https://support.microsoft.com/pt-pt/powerpoint/add-edit-or-remove-a-placeholder-on-a-slide-layout)).                                  |

### Romanian (`ro`)

Romanian technical writing addresses the reader with the formal second-person
plural, **dumneavoastră** (abbreviated **dvs.**), carried through matching
plural verb forms (verificați, selectați, rulați). This is the register
Microsoft's Romanian localization style guide prescribes for software UI and
documentation, and it matches the practice documented by the GNOME Romanian
translation team: imperative mood for commands ("Deschide"), second person for
user decisions ("Doriți să salvați?"), and impersonal or first-person-plural
machine voice for automatic actions ("Nu am putut deschide fișierul %s.").
Netsuke's diagnostics, which report what the tool did or found rather than
address the user directly, should default to this impersonal, third-person
descriptive style and reserve **dvs.**-form imperatives for CLI help text that
instructs the user. Avoid the informal **tu** register entirely; it reads as
inappropriately casual for a build tool's diagnostics.

Several Netsuke terms conventionally stay as English loan words in Romanian
technical writing: **build** (as a noun, e.g. "un build reușit"), **cache**,
**template** (though "șablon" is also common and preferred for Netsuke's Jinja
templates), **stdlib**, **dyndep**, **phony**, and **pipeline**.
**Allowlist**/**blocklist** are calqued, not borrowed, per Microsoft's
inclusive-language table (see below). **Macro** and **glob** are established
loans that need no gloss. Loans are written in Latin script with standard
Romanian orthography; plurals of English loans typically take Romanian suffixes
with a hyphen when the word is not yet fully assimilated (e.g. "cache-ul",
"build-uri"), but "cache" and "build" are frequent enough in Romanian developer
usage that hyphenation may be dropped for the invariable singular use as in the
table below.

Two hazards deserve attention. First, **manifest** is a false friend: in
general Romanian usage a "manifest" is a political or public declaration
(compare the 1918 "Manifest către popoarele lumii"), not a data file. Netsuke
diagnostics must rely on context (file paths, `.yaml`/`.yml` extensions) to
prevent misreading, and prose introducing the term should gloss it on first
use, e.g. "fișierul manifest (fișierul YAML de `configurare` a build-ului)".
Second, correct Romanian diacritics use the comma-below forms **ș** (U+0219)
and **ț** (U+021B), not the Turkish-style cedilla forms **ş** (U+015F) and
**ţ** (U+0163). Legacy Windows-1250/ISO-8859-2 fonts and some older
localization files substituted the cedilla glyphs, and mixed usage still
appears in older GNOME/KDE translation archives; Netsuke's Fluent resources
must use the comma-below code points consistently, as confirmed by the Unicode
reference for Ț and font-vendor guidance on the distinction.

```text
Nu s-a putut încărca manifestul de la { $path }.
```

Table 31: Romanian terminology

| en-US                  | preferred                           | notes                                                                                                                                    |
| ---------------------- | ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest                            | False friend: "manifest" ordinarily means a political/public declaration; gloss on first use.                                            |
| target                 | țintă                               | Distinct from acțiune/regulă; standard in Romanian Makefile/build tutorials.                                                             |
| action                 | acțiune                             | Netsuke's implicitly phony target; keep distinct from țintă and regulă.                                                                  |
| rule                   | regulă                              | The Ninja rule construct a target references.                                                                                            |
| dependency             | dependență (pl. dependențe)         | Standard Romanian computing term.                                                                                                        |
| order-only dependency  | dependență doar-de-ordine           | Calque; gloss as "dependență ce `impune` doar ordinea" on first use if needed.                                                           |
| build (noun)           | build                               | Loan word; widely used untranslated in Romanian developer writing, e.g. "un build reușit".                                               |
| build (verb)           | a compila / a construi              | `Compilare` for source-to-binary steps; "a construi" for the overall build graph execution.                                              |
| build graph            | graful de build                     | "Build" kept as loan within the compound.                                                                                                |
| phony target           | țintă fictivă                       | "Fictivă" ("fictitious") is the established gloss for Make/Ninja phony targets.                                                          |
| artefact               | artefact (de build)                 | Established loan from archaeology/general Romanian; "artefact de build" is current DevOps usage.                                         |
| working directory      | director de lucru                   | Confirmed by official Git and GNU tool Romanian localizations.                                                                           |
| workspace root         | rădăcina spațiului de lucru         | "Spațiu de lucru" is Microsoft's standard Romanian term for workspace.                                                                   |
| cache                  | cache                               | Loan word; "`memorie` cache" is used for hardware caches, but plain "cache" is standard for build/tool caches.                           |
| allowlist              | listă de `elemente` `permise`       | Per Microsoft's Romanian inclusive-terminology table (blocklist/allowlist entry).                                                        |
| blocklist              | listă de `elemente` blocate         | Per the same Microsoft table; avoid the older "listă neagră".                                                                            |
| template               | șablon                              | Standard Romanian term for template; avoid the English loan for prose, though it appears informally.                                     |
| macro                  | macro                               | Established invariable loan in Romanian technical writing.                                                                               |
| environment variable   | variabilă de mediu                  | Standard, widely attested term.                                                                                                          |
| exit status            | stare de ieșire                     | Confirmed by the official Romanian translations of bash and man-db ("STARE DE IEȘIRE").                                                  |
| stage (pipeline stage) | etapă                               | Used for pipeline/build stages; keep distinct from "stadiu" (used for medical/developmental stages).                                     |
| locale                 | localizare / configurație regională | "Configurație regională" is Microsoft's term for the language/region setting; "localizare" often used for the process.                   |
| placeable              | substituent                         | Calque for Fluent placeables; not a fixed term in Romanian l10n, so gloss it as "substituent (marcaj înlocuit la afișare)" on first use. |

Sources consulted: the
[Microsoft Romanian localization style guide](https://aka.ms/romanian-styleguide)
(summarized with direct quotation of its pronoun, register, and
inclusive-terminology tables at
[chatscontrol.com/learn/style-guides/microsoft-ro](https://chatscontrol.com/learn/style-guides/microsoft-ro));
the
[GNOME Romanian translation team's style notes](https://gnomero.sourceforge.net/sugestii.txt)
(Dan Damian, GNOME Romanian Translation Project); the official Romanian
localization of GNU bash, showing "stare de ieșire" for exit status
([po file](https://gitea.psi.ch/pmodules/bash/src/commit/6794b5478f660256a1023712b5fc169196ed0a22/po/ro.po))
and of man-db, showing the "STARE DE IEȘIRE" manual-page heading
([Debian manpages](https://manpages.debian.org/bullseye/man-db/man.1.ro.html));
Git's Romanian localization output showing "directorul de lucru" (observed in
community usage at
[reddit.com/r/git](https://www.reddit.com/r/git/comments/np208y/hi_all_i_cant_remove_new_file_from_staged_files/?tl=ro));
Microsoft Learn's Romanian documentation showing "spațiu de lucru" for
workspace (e.g.
[learn.microsoft.com/ro-ro/power-pages](https://learn.microsoft.com/ro-ro/power-pages/getting-started/customize-pages));
the Wikipedia article on [Ț](https://en.wikipedia.org/wiki/%C8%9A) and
[Brandient's Romanian diacritics guide](https://brandient.com/kit-on-romanian-diacritics)
for the comma-below versus cedilla code-point distinction; and
[dexonline.ro](https://dexonline.ro/definitie/romana) confirming the
political/declaratory sense of "manifest" in general Romanian usage.

### Russian (`ru`)

Russian technical writing addresses the reader with the formal plural вы
("vy"), written lowercase in software UI and error text; the capitalized Вы is
reserved for personal correspondence, not interface strings. This is confirmed
by the Microsoft Russian localization guidance (summarized at the
[Microsoft-derived Russian style guide](https://chatscontrol.com/learn/style-guides/microsoft-ru),
which cites `learn.microsoft.com/globalization`) and by community practice
such as the
[Unbabel Russian language guidelines](https://help.unbabel.com/hc/en-us/articles/360006329414-Language-Guidelines-Russian).
Netsuke's diagnostics are impersonal (they describe a condition and a
correction, not a request), so the вы/ты choice rarely surfaces directly; where
second-person phrasing is unavoidable (for example CLI help text), use
lowercase вы. Error messages follow the standard Russian technical pattern of
"не удалось" ("failed to") plus an infinitive, rather than a personified or
first-person construction; this is documented in the same Microsoft-derived
guide under "Error messages" and is visible throughout Microsoft's own product
strings (for example `Не удалось найти скачанные файлы...`).

Several Netsuke terms stay close to their English form. `build` (сборка,
собрать/собирать) is a naturalised loan-calque, not a raw loanword, and is the
term used throughout Russian CMake and CI/CD documentation. `cache` (кэш) and
`template` (шаблон) are likewise established in developer Russian, as is
`macro` (макрос). Structural Ninja/Make concepts translate rather than loan:
`target` is `цель`, confirmed by Russian CMake documentation
([ps-group CMake cheatsheet](http://ps-group.github.io/cxx/cmake_cheatsheet))
and by Microsoft's own CMake-in-Visual-Studio pages; `dependency` is
`зависимость` (same sources); and `phony target` is the long-established GNU
Make calque `фиктивная цель`, attested in Russian-language Make troubleshooting
threads. `allowlist`/`blocklist` are unsettled: some Russian technical writing
keeps the English forms verbatim (as in the Russian coverage of Go's
whitelist/blacklist rename, via [opennet.ru](https://opennet.ru/53109-golang)),
while other material uses the descriptive calques `список разрешений`/
`список блокировок`; this section uses the calques for readability, noting the
loanword alternative. `glob`, `dyndep`, `stdlib`, `pipeline`, and `stage` (as a
pipeline term) commonly stay as lightly adapted English loans in Russian
developer writing rather than gaining native calques.

Three hazards deserve attention. First, `манифест` (manifest) is a false friend
hazard shared with English: in general Russian usage it means a political
manifesto (as in "Манифест коммунистической партии") far more saliently than
"list of build inputs", so first use in longer prose should carry
disambiguating context. Second, `кэш` versus `кеш` (cache) is a live
orthographic dispute: Microsoft and most software use `кэш`, but the Russian
Academy of Sciences' orthographic dictionary, per
[gramota.ru](https://gramota.ru/poisk?query=%D0%9A%D0%AD%D0%A8&mode=spravka),
prescribes `кеш`; this section follows the dominant technical-industry spelling
`кэш` and flags the dispute rather than silently picking a side. Third,
`environment variable` splits between the Microsoft-preferred
`переменная среды` (used throughout `learn.microsoft.com/ru-ru`, including the
.NET API docs) and the developer-community form `переменная окружения`; this
section follows Microsoft's `переменная среды` for consistency with other
Microsoft-sourced terms, but either form is intelligible to Russian developers.
`working directory` (рабочий каталог) and `workspace` (рабочая область) both
follow Microsoft usage; `директория` and `папка` are understood synonyms but
`каталог` is the Microsoft and POSIX-manual norm.

Table 32: Russian terminology

| en-US                  | preferred                     | notes                                                                                                                            |
| ---------------------- | ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | манифест                      | Loan; false friend — usually means "political manifesto" in general Russian.                                                     |
| target                 | цель                          | Distinct from действие/правило; per [CMake ru docs](http://ps-group.github.io/cxx/cmake_cheatsheet).                             |
| action                 | действие                      | Distinct word from цель/правило (see Netsuke usage rule).                                                                        |
| rule                   | правило                       | Distinct word from цель/действие.                                                                                                |
| dependency             | зависимость                   | Confirmed via CMake/Kaspersky ru build docs.                                                                                     |
| order-only dependency  | зависимость только по порядку | No established single-word calque; constructed, verify before shipping.                                                          |
| build (noun)           | сборка                        | Established (e.g. "сборка проекта").                                                                                             |
| build (verb)           | собрать / собирать            | Perfective/imperfective pair; use per aspect.                                                                                    |
| build graph            | граф сборки                   | Compound of established terms.                                                                                                   |
| phony target           | фиктивная цель                | Established GNU Make calque.                                                                                                     |
| artefact               | артефакт                      | Loan; standard in Azure DevOps ru docs (build artefacts).                                                                        |
| working directory      | рабочий каталог               | Microsoft norm; not директория/папка.                                                                                            |
| workspace root         | корень рабочей области        | рабочая область confirmed by Visual Studio/VS Code ru localization.                                                              |
| cache                  | кэш                           | Industry-dominant spelling; gramota.ru orthographic dictionary prefers кеш — see hazard note.                                    |
| allowlist              | список разрешений             | Calque chosen over the English loanword for readability.                                                                         |
| blocklist              | список блокировок             | Calque; loanword also seen in Russian dev text.                                                                                  |
| template               | шаблон                        | Established (Jinja/Django templates → шаблоны).                                                                                  |
| macro                  | макрос                        | Established loan-calque.                                                                                                         |
| environment variable   | переменная среды              | Microsoft norm; переменная окружения also common among developers.                                                               |
| exit status            | код завершения                | Confirmed via Debian ru manpages and bash guide translations.                                                                    |
| stage (pipeline stage) | этап                          | Standard pipeline vocabulary ("этап конвейера").                                                                                 |
| locale                 | локаль                        | Well-established loan, no viable calque.                                                                                         |
| placeable              | плейсхолдер                   | No official Russian Fluent term found; constructed on the placeholder loan, verify with Fluent l10n community if adopted widely. |

Worked example, preserving the placeable exactly:

```text
Не удалось загрузить манифест по пути { $path }.
```

### Swedish (`sv`)

Swedish technical writing addresses the reader with the informal second-person
pronoun **du** throughout, including in error and diagnostic text; there is no
working T–V distinction left in modern Swedish IT prose (the 1960s–70s
"du-reformen" retired formal address almost everywhere). The
[Microsoft Swedish style guide](https://aka.ms/swedish-styleguide) confirms
this directly: its pronoun guidance pairs "You can change when new updates get
installed." with "**Du** kan ändra när uppdateringar installeras," and
explicitly recommends against impersonal or passive phrasing. The same guide
adds a Swedish-specific wrinkle for machine-originated text: error and status
messages should avoid personifying the computer ("kan", "kunde") and instead
use fixed impersonal constructions such as "Det gick inte att …" for "Failed to
…". Netsuke's diagnostics follow that pattern: they are stated as impersonal
facts about the operation, not addressed to "you", while any CLI help or prose
that does address the operator directly uses **du**.

Several Netsuke terms stay as unlocalized English loans in Swedish technical
writing: **cache**, **build** (as a stand-alone noun in casual developer
speech, though Netsuke's own noun sense is calqued as *bygge*, see below), and
**manifest**. Loans are written in the Latin script with normal Swedish
inflection endings attached directly, without an apostrophe or space (for
example "cachen", "cachar"), following the Microsoft guide's rule of
integrating established loans into ordinary Swedish noun and verb classes once
an article and plural form are settled by precedent.

Two hazards are worth flagging. First, **manifest** is a false-friend risk:
besides the technical "manifest file" sense (well attested in Swedish developer
writing, e.g. Microsoft's own Azure docs and academic software theses using
"manifestfil"), Swedish "manifest" primarily denotes a political or artistic
manifesto (Svenska Akademiens ordlista's first sense: "skriftligt
tillkännagivande av program eller ståndpunkt av parti, konstriktning etc.").
Diagnostics that name a Netsuke manifest should keep it unambiguous with
context such as "manifestfilen" rather than bare "manifestet" where a reader
might default to the political sense. Second, Swedish writes compounds solidly:
splitting them (**särskrivning**) is the classic and heavily mocked Swedish
spelling error, and it can silently change meaning (e.g. the stock joke "rök
fritt" — "smoke freely" — versus "rökfritt" — "smoke-free"). Every compound
term in the table below (*byggkatalog*-style forms, *arbetskatalog*,
*miljövariabel*, *byggartefakt*) must be written as one unbroken word; never
insert a hyphen or space between the elements.

Table 33: Swedish terminology

| en-US                  | preferred        | notes                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---------------------- | ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest         | Loan noun ("ett manifest"/"manifestet"); compound as "manifestfil" when naming the file. False friend: primarily means a political/artistic manifesto in general Swedish ([SAOL](https://svenska.se/?activeTab=alla&q=manifest)). Confirmed loan use: [Microsoft Learn sv-se](https://learn.microsoft.com/sv-se), academic theses on [DiVA](https://www.diva-portal.org/smash/get/diva2:1674878/FULLTEXT01.pdf). |
| target                 | mål              | Established in Swedish build-tool writing; confirmed by the [tp-sv](https://www.softwolves.pp.se/old/2005/tp-sv/13/6553.html) GNU Make translation review ("målet", "målfilen") and the [W3C Swedish glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html).                                                                                                           |
| action                 | åtgärd           | Ubuntu-provenance term in the [W3C glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html); kept distinct from *mål* (target) and *regel* (rule).                                                                                                                                                                                                                       |
| rule                   | regel            | Confirmed in the tp-sv Make review ("regel", "implicit regel") and in Swedish ESLint/lint documentation ("regel"/"regeln"). Distinct word from *mål* and *åtgärd*.                                                                                                                                                                                                                                               |
| dependency             | beroende         | Standard in Swedish software writing; confirmed by academic and developer sources (e.g. [DiVA thesis](https://www.diva-portal.org/smash/get/diva2:1451724/FULLTEXT01.pdf), IT glossaries).                                                                                                                                                                                                                       |
| order-only dependency  | ordningsberoende | Calque from *ordning* + *beroende*; not a fixed industry term, so gloss it on first use as "beroende `som` endast styr ordning, utlöser inte ombyggnad". Write solid, never "ordnings beroende".                                                                                                                                                                                                                 |
| build (noun)           | bygge            | [W3C/GNOME-provenance glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html) entry.                                                                                                                                                                                                                                                                                    |
| build (verb)           | bygga            | Standard Swedish verb "att bygga", used throughout Swedish Azure/DevOps documentation for compiling a project.                                                                                                                                                                                                                                                                                                   |
| build graph            | byggraf          | Calque (*bygge* + *graf*); no fixed term found, but transparent and compounded solidly.                                                                                                                                                                                                                                                                                                                          |
| phony target           | låtsasmål        | Preferred over GNU Make's own "falskt mål": the [tp-sv review](https://www.softwolves.pp.se/old/2005/tp-sv/13/6553.html) flags "falskt" (false, deceptive) as the wrong connotation and proposes "låtsasmål" (pretend/dummy target) instead.                                                                                                                                                                     |
| artefact               | byggartefakt     | Confirmed in official [Microsoft Learn sv-se Azure Pipelines docs](https://learn.microsoft.com/sv-se/azure/devops/pipelines/artifacts/build-artifacts?view=azure-devops) ("byggartefakter").                                                                                                                                                                                                                     |
| working directory      | arbetskatalog    | [W3C/GNOME glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html) entry, matches "The directory in which the user's commands take place".                                                                                                                                                                                                                              |
| workspace root         | arbetsytans rot  | *Workspace* = *arbetsyta* ([W3C/GNOME glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html)); genitive "arbetsytans rot" avoids an awkward triple compound.                                                                                                                                                                                                           |
| cache                  | cache            | Loan, unchanged ([W3C/GNOME glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html)); inflects as "cachen", "cacha" (verb) in Swedish developer usage.                                                                                                                                                                                                                  |
| allowlist              | tillåtlista      | Confirmed in official [Microsoft Learn sv-se](https://learn.microsoft.com/sv-se/azure/databricks/data-governance/unity-catalog/manage-privileges/allowlist) documentation.                                                                                                                                                                                                                                       |
| blocklist              | blockeringslista | Confirmed in Swedish OpenWrt/LuCI localization (banip package, tp-sv-adjacent community translation).                                                                                                                                                                                                                                                                                                            |
| template               | mall             | [W3C/GNOME glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html) entry, standard across Swedish office and developer software.                                                                                                                                                                                                                                        |
| macro                  | `makro`          | [W3C/GNOME glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html) entry.                                                                                                                                                                                                                                                                                               |
| environment variable   | miljövariabel    | [W3C/GNOME glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html) entry; write solid, never "miljö `variabel`".                                                                                                                                                                                                                                                        |
| exit status            | slutstatus       | [W3C/Linux-provenance glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html) entry; noticeably shorter than the English source, consistent with Swedish's tendency to contract status labels.                                                                                                                                                                          |
| stage (pipeline stage) | steg             | General Swedish word for "step"; attested for CI/CD pipeline stages in Swedish developer usage (e.g. GitLab "stage" rendered as "steg").                                                                                                                                                                                                                                                                         |
| locale                 | `lokal`          | [W3C/Linux-provenance glossary](http://www.w3c.se/resources/office/translations/translators/dictionaries/unified.html) entry.                                                                                                                                                                                                                                                                                    |
| placeable              | placeable        | No established Swedish equivalent for this Fluent-syntax term; kept as an unlocalized technical loan, per [Mozilla's localizer documentation](https://mozilla-l10n.github.io/localizer-documentation/tools/fluent/basic_syntax.html), which itself does not translate the term.                                                                                                                                  |

Worked example, preserving the placeable exactly:

```text
Det gick inte att läsa in manifestet vid { $path }.
```

### Thai (`th`)

Thai text addresses the reader with the neutral-polite second-person pronoun คุณ
(khun) rather than a T–V pair; Thai marks social distance mainly through
pronoun and particle choice rather than verb conjugation, and คุณ is the default
that Microsoft's Thai localization style guide
([aka.ms/thai-styleguide](https://aka.ms/thai-styleguide)) recommends for all
consumer- and developer-facing text, explicitly steering translators away from
the informal เธอ and from the masculine-only ผม as a default "I". Every worked
UI example in that guide omits the spoken politeness particles ครับ (male
speaker) and ค่ะ (female speaker), confirming the seed assumption: because these
particles encode the speaker's gender, written technical and UI Thai drops them
and either addresses the user implicitly (imperative verb with no stated
subject) or uses คุณ explicitly when a subject is needed. Netsuke diagnostics
should follow the same pattern: state the condition plainly, drop ครับ/ค่ะ, and
use คุณ only where a directive needs an explicit addressee.

Several Netsuke terms are conventional English loans in Thai developer writing
rather than native calques: `manifest` stays in Latin script untranslated
(matching Android, Chrome extension, and Web App Manifest Thai developer docs,
all of which keep "Manifest" as-is rather than translating it), `cache` is แคช,
`template` is เทมเพลต, and `locale` is โลแคล (attested in Debian/Ubuntu
localizer Theppitak Karoonboonyanan's Thai technical blog). "build" as a
countable artefact is often the colloquial loan บิลด์, though the process itself
is rendered natively as การสร้าง/สร้าง, attested in the Thai Wikipedia article on
continuous integration. `target`, `action`, and `rule` are kept as three
distinct native words (เป้าหมาย, การกระทำ, กฎ) so Netsuke's target/action/rule
distinction survives translation; กฎ for "rule" matches usage in Thai Linux
documentation (thaitux.info). Ninja- and Netsuke-specific compounds with no
prior Thai rendering (order-only dependency, build graph, phony target,
workspace root, placeable) are flagged as coined calques in the table notes
rather than presented as established terms.

The Thai cognate of "manifest" most readers reach for first is แถลงการณ์ (a
political manifesto or official statement), and a shipping/customs manifest is
ใบตราส่ง or บัญชีสินค้า — neither is Netsuke's build manifest, which is why keeping
`manifest` in Latin script, as major Thai developer docs already do, avoids the
collision outright. Thai script also has no word-final spaces and no letter
case, so an embedded Latin identifier (a flag name, a filename, `manifest`
itself) must carry an explicit space on each side or it visually fuses with the
surrounding Thai syllables; compare attested strings such as "ไฟล์ Manifest" and
"USB แฟลชไดรฟ์", where the space is load-bearing, not decorative. Because Thai
has no case, a reader also cannot fall back on capitalization to spot where an
identifier starts or ends, unlike in English. Finally, there is a live tension
between Royal Institute (ราชบัณฑิตยสภา) formal coinages — the normative reference
Microsoft itself cites — and the shorter loans developers actually use day to
day (บิลด์, เทมเพลต, แคช); this table follows attested practical usage over
untested formal coinages, and says so wherever a term is a best-effort calque
rather than a verified community choice.

```text
ไม่สามารถโหลด manifest ที่ { $path } ได้
```

Table 34: Thai terminology

| en-US                  | preferred                        | notes                                                                                                                                              |
| ---------------------- | -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest (English, Latin script) | Kept untranslated per Android/Chrome/Web App Manifest Thai docs; false friend แถลงการณ์ = political manifesto, ใบตราส่ง/บัญชีสินค้า = shipping manifest. |
| target                 | เป้าหมาย                          | Distinct from action/rule by design.                                                                                                               |
| action                 | การกระทำ                         | Netsuke-specific sense: implicit phony target. Distinct word from target/rule.                                                                     |
| rule                   | กฎ                               | Attested in Thai Linux documentation (thaitux.info).                                                                                               |
| dependency             | สิ่งที่ต้องพึ่งพา                       | Root การพึ่งพา attested (Thai Wikipedia CI article; dependency management).                                                                          |
| order-only dependency  | สิ่งที่ต้องพึ่งพาแบบลำดับเท่านั้น           | Coined calque; Ninja-specific concept has no prior Thai rendering.                                                                                 |
| build (noun)           | บิลด์                              | Colloquial loan for a build artefact/version; process itself uses การสร้าง.                                                                         |
| build (verb)           | สร้าง                             | Attested (th.wikipedia.org CI article: "การสร้างที่ช้า" = slow build).                                                                                 |
| build graph            | กราฟการสร้าง                      | Coined calque combining attested กราฟ (graph) and การสร้าง (build).                                                                                 |
| phony target           | เป้าหมายเสมือน                     | Coined calque ("pseudo target"); avoids the pejorative "หลอก" (fake/deceive).                                                                      |
| artefact               | สิ่งที่สร้างขึ้น                        | Coined calque ("the thing produced"); loan อาร์ติแฟกต์ also seen in DevOps writing.                                                                   |
| working directory      | ไดเรกทอรีทำงาน                    | Attested in Thai developer usage (e.g. r/rstats, r/gameenginedevs Thai threads).                                                                   |
| workspace root         | รากของพื้นที่ทำงาน                   | Coined compound; พื้นที่ทำงาน (workspace) is attested office-software Thai UI vocabulary.                                                              |
| cache                  | แคช                              | Attested; dedicated Thai Wikipedia article.                                                                                                        |
| allowlist              | รายการที่อนุญาต                     | Attested (Google Workspace/Android permissions Thai docs).                                                                                         |
| blocklist              | รายการที่ปฏิเสธ                     | Coined parallel to allowlist; older synonym บัญชีดำ (blacklist) still common but pairs poorly with the neutral allowlist phrasing.                   |
| template               | เทมเพลต                          | Loan, attested (Microsoft Adoption, Word Thai template docs); native แม่แบบ seen in general/educational contexts only.                              |
| macro                  | แมโคร                            | Long-established loan (e.g. spreadsheet macros); not independently re-verified here, treated as common developer knowledge.                        |
| environment variable   | ตัวแปรสภาพแวดล้อม                  | ตัวแปร (variable) is standard CS Thai; สภาพแวดล้อม (environment) attested in Thai Wikipedia CI/Jupyter articles.                                     |
| exit status            | สถานะการออก                      | Attested across Thai developer forum threads (CodeGym, ssdnodes systemd article).                                                                  |
| stage (pipeline stage) | ขั้นตอน                            | Native, standard "step/stage"; loan สเตจ also seen for a literal CI config keyword.                                                                |
| locale                 | โลแคล                            | Attested loan (Theppitak Karoonboonyanan's Thai Debian/Ubuntu blog; Thai Next.js i18n community).                                                  |
| placeable              | ค่าที่แทรกได้                        | Coined calque ("insertable value"); no established Thai Fluent glossary term was found, so treat this row as unverified.                           |

### Turkish (`tr`)

Netsuke's Turkish output should address the reader with the formal second
person plural, `siz`, rather than the informal "sen". The
[Microsoft Turkish style guide](https://aka.ms/turkish-styleguide) states that
"in Turkish when addressing the user, formal 'you' (`siz`) is used in general
context," reserving "sen" for younger audiences or marketing copy — neither
applies to a build tool's CLI and diagnostics. Imperative CLI prompts should
use the plain formal imperative suffix ("-in"/"-yin", as in "tamamlayın",
"belirtin"), matching the guide's own worked examples, rather than the more
bureaucratic "-iniz" form (e.g. "seçiniz") seen on government forms and older
signage. Diagnostic messages, which state a condition rather than instruct the
user, read more naturally in the impersonal or passive voice; the guide
endorses passive phrasing "in informational messages that we don't want to
blame the user," which matches Netsuke's plain, blame-free diagnostic style.

Some Netsuke terms stay as English loan words in Turkish technical writing
rather than being translated. `glob` is one: Azure CLI's Turkish release notes
describe "glob işlemi" (a glob operation) without translating the word itself,
so the table below keeps it as a loan, written in the Latin script unmodified.
`manifest` is a second case, and a deliberate one: Google publishes
AndroidManifest.xml documentation in Turkish under the heading "Manifest
dosyası" (manifest file), keeping "manifest" as a loan rather than using the
native word "manifesto". This choice is not stylistic but protective — see the
hazard below. `cache`, `template`, and `macro`, by contrast, have
long-established native or fully adapted renderings ("önbellek", "şablon",
`makro`) confirmed across Microsoft's Turkish developer documentation, so
Netsuke should use those rather than the English loans.

The most serious hazard is a false friend: Turkish "manifesto" is the ordinary
word for both a political manifesto and a shipping/customs manifest (a cargo
document), as confirmed by Turkish logistics glossaries and customs regulation
text. Translating Netsuke's build manifest as "manifesto" would read as either
a political tract or a freight document, not a build description file. Keeping
the English loan "manifest" (as Android's own Turkish documentation does)
avoids this collision entirely. A second hazard is orthographic: Turkish has a
dotted/dotless "i" split (lowercase "i" capitalizes to "İ", not "I"; uppercase
"I" lowercases to "ı", not "i"), so any case-transformation of an English
identifier or key under Turkish casing rules ("Netsuke" → "NETSUKE" done
Turkish-locale-aware could corrupt "i" to "İ") will corrupt it — identifiers
such as `Netsuke`, `Ninja`, `stdout`, `dyndep`, and `command_available` must
never be case-folded using Turkish rules. Third, Turkish attaches
possessive/case suffixes to loanwords and identifiers with a preceding
apostrophe, e.g. "Netsukefile'ı yükleyin" (load the Netsukefile) or "Ninja'nın
çıkış durumu" (Ninja's exit status); this convention should be followed
wherever an identifier takes a Turkish suffix, and the apostrophe must not be
mistaken for a typo and removed.

The example diagnostic, preserving the `{ $path }` placeable and the `manifest`
loan exactly:

```text
{ $path } konumundaki manifest yüklenemedi.
```

Table 35: Turkish terminology

| en-US                  | preferred                     | notes                                                                                                                                                                                                                                                                                                                                 |
| ---------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | manifest                      | English loan, kept to avoid the "manifesto" false friend (political/shipping manifest); see [Android Turkish docs](https://developer.android.com/guide/topics/manifest/manifest-intro?hl=tr) ("Manifest dosyası").                                                                                                                    |
| target                 | hedef                         | Established in Microsoft's NMAKE Turkish docs, e.g. [Inference rules](https://learn.microsoft.com/tr-tr/cpp/build/reference/inference-rules?view=msvc-170). Distinct from `rule` and `action`.                                                                                                                                        |
| action                 | eylem                         | Avoids "işlem" (process/operation/transaction — collision hazard). Confirmed usage in [Power Automate tr docs](https://learn.microsoft.com/tr-tr/power-automate/desktop-flows/how-to/run-macros-excel) ("…eylemini dağıtın").                                                                                                         |
| rule                   | kural                         | Confirmed in Microsoft's NMAKE ["Description blocks"](https://learn.microsoft.com/tr-tr/cpp/build/reference/description-blocks?view=msvc-170) ("Bu kural kümesini…"). Distinct from `target` and `action`.                                                                                                                            |
| dependency             | bağımlılık                    | Widely established, e.g. [NuGet package references](https://learn.microsoft.com/tr-tr/nuget/consume-packages/package-references-in-project-files).                                                                                                                                                                                    |
| order-only dependency  | yalnızca sıralama bağımlılığı | No established Turkish build-tool term found; compositional calque built from `bağımlılık` (dependency) plus "yalnızca sıralama" (ordering-only) to keep the distinction explicit.                                                                                                                                                    |
| build (noun)           | derleme                       | Confirmed in [Azure Pipelines CMake task docs](https://learn.microsoft.com/tr-tr/azure/devops/pipelines/tasks/reference/cmake-v1?view=azure-pipelines) ("Derlemenin çalıştırıldığı çalışma dizini"). Also means "compilation"; accept the overlap rather than invent a native calque.                                                 |
| build (verb)           | derlemek                      | Verb pair of `derleme`; used the same way across Microsoft's Turkish build-tool docs.                                                                                                                                                                                                                                                 |
| build graph            | derleme grafiği               | Compositional calque from `derleme` (build) + `grafik` (graph); no dedicated Ninja/Netsuke Turkish precedent found.                                                                                                                                                                                                                   |
| phony target           | sahte hedef                   | Used in Turkish developer discussion of Makefile `.PHONY` targets, e.g. [Reddit r/programming (tr)](https://www.reddit.com/r/programming/comments/n0hnq4/using_a_makefile_with_phonyonly_targets_use_a/?tl=tr) ("SAHTE hedef").                                                                                                       |
| artefact               | derleme çıktısı               | "Build output"; confirmed in [SSMS DevOps docs](https://learn.microsoft.com/tr-tr/ssms/database-devops) ("Derleme çıktısı…").                                                                                                                                                                                                         |
| working directory      | çalışma dizini                | Confirmed across Microsoft Turkish docs, e.g. [Azure Pipelines CMake task](https://learn.microsoft.com/tr-tr/azure/devops/pipelines/tasks/reference/cmake-v1?view=azure-pipelines).                                                                                                                                                   |
| workspace root         | çalışma alanı kökü            | `çalışma alanı` (workspace) confirmed in [Azure Pipelines phases](https://learn.microsoft.com/tr-tr/azure/devops/pipelines/process/phases?view=azure-devops) ("çalışma alanı dizinine"); `kökü` (root) is a standard compositional suffix.                                                                                            |
| cache                  | önbellek                      | Confirmed throughout Microsoft Turkish docs, e.g. [Azure Cache for Redis](https://learn.microsoft.com/tr-tr/azure/templates/microsoft.cache/redis/accesspolicies).                                                                                                                                                                    |
| allowlist              | izin verilenler listesi       | Confirmed in [Defender for Office 365 tr docs](https://learn.microsoft.com/tr-tr/defender-office-365/tenant-allow-block-list-about).                                                                                                                                                                                                  |
| blocklist              | engellenenler listesi         | Confirmed in the same [Defender/Entra tr docs](https://learn.microsoft.com/tr-tr/entra/architecture/5-secure-access-b2b).                                                                                                                                                                                                             |
| template               | şablon                        | Long established, e.g. [SQL Server template caching](https://learn.microsoft.com/tr-tr/sql/relational-databases/sqlxml-annotated-xsd-schemas-xpath-queries/caching-templates-xml-schemas/template-caching-sqlxml-4-0).                                                                                                                |
| macro                  | `makro`                       | Adapted loan, standard across Microsoft Office/Power Automate Turkish docs (e.g. Excel `Makro`).                                                                                                                                                                                                                                      |
| environment variable   | ortam değişkeni               | Confirmed in [Speech Service quickstart](https://learn.microsoft.com/tr-tr/azure/ai-services/speech-service/get-started-text-to-speech) ("…ortam değişkeni ekleyin").                                                                                                                                                                 |
| exit status            | çıkış durumu                  | Confirmed in [Fabric Spark troubleshooting](https://learn.microsoft.com/tr-tr/fabric/data-engineering/troubleshoot-spark) ("çıkış durumu").                                                                                                                                                                                           |
| stage (pipeline stage) | aşama                         | Confirmed in [Azure Pipelines predefined variables](https://learn.microsoft.com/tr-tr/azure/devops/pipelines/build/variables?view=azure-devops) ("Bu aşama…").                                                                                                                                                                        |
| locale                 | yerel ayar                    | Confirmed across platforms, e.g. [Android localization guide](https://developer.android.com/guide/topics/resources/localization?hl=tr) and [Arch Wiki (tr)](https://wiki.archlinux.org/title/Locale_(T%C3%BCrk%C3%A7e)).                                                                                                              |
| placeable              | yer tutucu                    | No dedicated Turkish Fluent glossary found; extended from Microsoft's established "yer tutucu" (placeholder) convention, e.g. [Power Apps text input](https://learn.microsoft.com/tr-tr/power-apps/maker/canvas-apps/controls/modern-controls/modern-control-text-input). Treat as provisional pending a Fluent-specific tr glossary. |

### Ukrainian (`uk`)

Ukrainian technical writing addresses the reader with the formal plural "ви"
(lowercase in running UI text), matching general European T–V practice, and
often prefers impersonal constructions for system messages rather than
addressing the user directly. The
[Microsoft Ukrainian style guide](https://aka.ms/ukrainian-styleguide) confirms
formal "ви" (lower case) for direct address in software UI, and recommends
impersonal, third-person phrasing for errors and status messages where English
uses an imperative or first person ("Please wait" becomes "Зачекайте" or
"Виконується очікування", not a first-person construction). Ukrainian Linux
man-page translations (Arch Linux and openSUSE manual pages, translated by Yuri
Chornoivan, a long-standing KDE and GNU translator) show the same pattern:
commands and diagnostics are phrased impersonally, with "не вдалося" (literally
"it did not succeed") plus an infinitive standing in for "Failed to …".

Netsuke's core nouns stay Ukrainian rather than becoming loan words: `ціль` for
target, `дія` for action, `правило` for rule, and `залежність` for dependency
are all attested in Yuri Chornoivan's Ukrainian translation of GNU Make (see
the worked example and table notes). Genuine loan words, written in Cyrillic,
are reserved for concepts without an established native term: `кеш` (cache),
`шаблон` stays native but `макрос` (macro) is a loan, and `стек`/
`конвеєр`-style borrowings are typical for pipeline-adjacent jargon. `стадія`/
`етап` compete for "stage"; Netsuke uses `етап`, the more common choice in
Ukrainian CI/CD writing, as a native word rather than a loan.

Hazards: the 2019 Ukrainian orthography reform changed the spelling of many
Latin/Greek-root loan words that take "є" after a vowel — for example `проєкт`
(not `проект`), `проєкція`. Netsuke's own vocabulary avoids "проект", but any
UI or documentation prose that discusses "the project" (the Netsuke workspace
or manifest tree) must use `проєкт`, not the Russian-influenced pre-reform
spelling, or it will read as dated or non-standard to reviewers from
KDE/GNOME-adjacent teams. A second hazard is `ціль` versus `мета`: both
translate "target"/"goal" and Ukrainian localizers (including Chornoivan's own
GNU Make translation) mix them informally. Netsuke must fix on `ціль` alone so
that `target`, `action`, and `rule` remain three distinct, unconfused nouns;
`мета` must not appear anywhere referring to a Netsuke target. A third hazard:
Ukrainian `кеш` is a homograph of an unrelated financial term "кеш" (cash,
colloquial), but context in build diagnostics disambiguates it safely.

Worked example:

```text
Не вдалося завантажити маніфест за адресою { $path }.
```

Table 36: Ukrainian terminology

| en-US                  | preferred                   | notes                                                                                                                                                                                                                                                             |
| ---------------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | маніфест                    | Not `декларація`; false friend risk is low, but avoid `угода` (contract/agreement sense).                                                                                                                                                                         |
| target                 | ціль                        | Fixed choice over the competing synonym `мета`, attested in [GNU Make uk.po (Chornoivan, 2023)](https://translationproject.org/PO-files/uk/make-4.4.0.90.uk.po).                                                                                                  |
| action                 | дія                         | Distinct from `ціль`/`правило` per Netsuke's three-way split.                                                                                                                                                                                                     |
| rule                   | правило                     | Confirmed by [GNU Make uk.po](https://translationproject.org/PO-files/uk/make-4.4.0.90.uk.po) ("Неявні правила" = "Implicit Rules").                                                                                                                              |
| dependency             | залежність                  | Confirmed by [GNU Make uk.po](https://translationproject.org/PO-files/uk/make-4.4.0.90.uk.po) ("Залежність «%s» мети «%s»").                                                                                                                                      |
| order-only dependency  | залежність лише за порядком | Descriptive phrase; Make's own translation renders "order-only" as "лише порядком збирання" rather than a fixed noun.                                                                                                                                             |
| build (noun)           | збирання                    | The process sense, matching KDE/Make usage ("порядком збирання"); avoid the Russian-calque-flavoured `збірка` for the process.                                                                                                                                    |
| build (verb)           | зібрати                     | Perfective infinitive, paired with `збирання` as the noun.                                                                                                                                                                                                        |
| build graph            | граф збирання               | Compound, not attested as a single term; built from the confirmed `збирання`.                                                                                                                                                                                     |
| phony target           | псевдоціль                  | Attested as one word in [GNU Make uk.po](https://translationproject.org/PO-files/uk/make-4.4.0.90.uk.po) ("Псевдоціль (залежність .PHONY)"), built on the fixed `ціль`.                                                                                           |
| artefact               | артефакт                    | Loan word; standard in Ukrainian CI/CD and build-tool writing.                                                                                                                                                                                                    |
| working directory      | робочий каталог             | Confirmed across Ukrainian man pages, e.g. [Arch Linux tar(1) uk](https://man.archlinux.org/man/tar.1.uk) and [sftp(1) uk](https://man.archlinux.org/man/sftp.1.uk) (both Chornoivan translations).                                                               |
| workspace root         | корінь робочого простору    | `робочий простір` is the standard calque for "workspace"; `корінь` for root directory follows general Ukrainian filesystem usage.                                                                                                                                 |
| cache                  | кеш                         | Loan word, standard current spelling per the [2019 orthography](https://www.uzhnu.edu.ua/uk/infocentre/get/75340); not a homograph collision in build context.                                                                                                    |
| allowlist              | список дозволених           | Descriptive calque; Ukrainian security writing avoids literal colour-coded "білий список" in current Microsoft-aligned usage, see [Microsoft Style Guide: allow list](https://learn.microsoft.com/en-us/style-guide/a-z-word-list-term-collections/a/allow-list). |
| blocklist              | список заблокованих         | Parallel descriptive calque to `список дозволених`.                                                                                                                                                                                                               |
| template               | шаблон                      | Native word, not a loan; standard across Ukrainian software localization.                                                                                                                                                                                         |
| macro                  | макрос                      | Loan word, standard in Ukrainian technical writing.                                                                                                                                                                                                               |
| environment variable   | змінна середовища           | Confirmed as the Microsoft Ukrainian localization choice, e.g. [Power Platform docs uk-ua](https://learn.microsoft.com/uk-ua/power-apps/maker/data-platform/environmentvariables); prefer over `змінна оточення`.                                                 |
| exit status            | код виходу                  | Attested in Ukrainian Docker/container documentation ("код виходу"), matching common CLI usage.                                                                                                                                                                   |
| stage (pipeline stage) | етап                        | Native word preferred over `стадія` in Ukrainian CI/CD writing.                                                                                                                                                                                                   |
| locale                 | локаль                      | Loan word, standard in Ukrainian software localization.                                                                                                                                                                                                           |
| placeable              | заповнювач                  | Descriptive native compound for a substitutable placeholder slot; no single attested term found, built from `заповнювати` (to fill in).                                                                                                                           |

Sources used:

- [Microsoft Ukrainian style guide](https://aka.ms/ukrainian-styleguide)
- [GNU Make 4.4.0.90 Ukrainian translation (Yuri Chornoivan), via Translation Project](https://translationproject.org/PO-files/uk/make-4.4.0.90.uk.po)
- [Translation Project: the make textual domain](https://translationproject.org/domain/make.html)
- [Microsoft Ukrainian localization: environment variables (Power Platform)](https://learn.microsoft.com/uk-ua/power-apps/maker/data-platform/environmentvariables)
- [Microsoft Style Guide: allow list](https://learn.microsoft.com/en-us/style-guide/a-z-word-list-term-collections/a/allow-list)
- [Arch Linux manual pages, tar(1), Ukrainian](https://man.archlinux.org/man/tar.1.uk)
- [Arch Linux manual pages, sftp(1), Ukrainian (Yuri Chornoivan)](https://man.archlinux.org/man/sftp.1.uk)
- [openSUSE manual pages, mount(8), Ukrainian](https://manpages.opensuse.org/Leap-15.6/man-pages-uk/mount.8.uk.html)
- [Uzhhorod National University: правописні норми української мови (2019 orthography reference)](https://www.uzhnu.edu.ua/uk/infocentre/get/75340)
- [KEn Docker documentation (Ukrainian), "код виходу"](https://github.com/malakhovks/ken)

### Vietnamese (`vi`)

Vietnamese software localization addresses the reader as "bạn", the established
neutral-friendly second person used across major desktop and developer-tool
translations. The
[Microsoft localization style guides](https://learn.microsoft.com/en-us/globalization/reference/microsoft-style-guides)
directory, which indexes the Vietnamese guide, instructs translators to
address users with "bạn" and to avoid gendered pronouns in generic references.
The community-run [GNOME-VI](https://wiki.gnome.org/GnomeVi.html) project and
the [Ubuntu GNOME Vietnamese](https://wiki.ubuntu.com/UbuntuGNOME/Vietnamese)
localization pages follow the same convention throughout their interface and
help text. Netsuke's diagnostics, which state a condition and address the
operator directly, should keep "bạn" for any second-person phrasing and
otherwise favour impersonal, declarative sentences, matching the tool's calm,
non-conversational register.

Several Netsuke terms conventionally stay as English loan words in Vietnamese
technical writing, written in Latin script with no diacritics added: "macro"
and "template" both circulate as loans in casual developer usage, though
Bazel's official Vietnamese documentation keeps "macro" untranslated while
rendering "template" as "mẫu". "Cache" is often heard as a loan in spoken
developer Vietnamese, but the dominant written convention in major FOSS
translations (Git, util-linux, Bazel) is the calque "bộ nhớ đệm", which this
glossary prefers. Vietnamese has no case or gender marking, so loans integrate
without inflection; when a classifier is needed the pattern is to prefix a
native noun such as "bản" (an instance or copy, as in "bản dựng", a build) or
"tệp" (file) rather than borrow one.

Hazards: the everyday Vietnamese word for "manifesto" is "tuyên ngôn"
(political declaration), a false friend for the build-system sense of
"manifest". Established Vietnamese localization for Android's
AndroidManifest.xml avoids this collision by using "tệp kê khai" (literally
"declaration file"), and this glossary follows that precedent for Netsuke's
manifest files. "Mục tiêu" (target) also means "goal" or "objective" in
everyday Vietnamese, so first uses in longer documents may benefit from a
parenthetical gloss. Vietnamese diacritics are combining marks over Latin
letters; command examples and identifiers must stay unmarked ASCII, and any
copy-paste from web sources should be checked for smart quotes or non-ASCII
punctuation introduced by editors.

Worked example, preserving the placeable exactly:

```text
Không thể tải tệp kê khai tại { $path }.
```

Table 37: Vietnamese terminology

| en-US                  | preferred                      | notes                                                                                                                                                     |
| ---------------------- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | tệp kê khai                    | Avoids false friend "tuyên ngôn" (manifesto); follows [Android Vietnamese docs](https://developer.android.com/guide/topics/manifest/manifest-intro?hl=vi) |
| target                 | mục tiêu                       | Follows [Bazel Vietnamese docs](https://bazel.build/versions/7.4.0/extending/rules?hl=vi); "đích" also seen in GNU Make/manpage translations              |
| action                 | hành động                      | Follows [Bazel Action docs](https://bazel.build/rules/lib/builtins/Action?hl=vi); distinct from "thao tác" (operation)                                    |
| rule                   | quy tắc                        | Confirmed across Bazel, gcc, and Poedit `vi.po` translations                                                                                              |
| dependency             | phần phụ thuộc                 | Confirmed in Bazel, Git, and Debian apt `vi.po` translations                                                                                              |
| order-only dependency  | phần phụ thuộc chỉ theo thứ tự | Calque; Ninja's order-only concept has no established Vietnamese term                                                                                     |
| build (noun)           | bản dựng                       | Confirmed in [Bazel Vietnamese user manual](https://bazel.build/versions/8.1.0/docs/user-manual?hl=vi)                                                    |
| build (verb)           | dựng                           | Verb form derived from "bản dựng"; "build" also kept as a loan for the command name                                                                       |
| build graph            | biểu đồ bản dựng               | Calque on Bazel's "biểu đồ" (graph), e.g. "biểu đồ hành động"                                                                                             |
| phony target           | mục tiêu giả                   | Calque; "giả" = fake/pseudo, consistent with "mục tiêu"                                                                                                   |
| artefact               | cấu phần phần mềm              | Verbose but established via Bazel and Android Open Source Project Vietnamese docs                                                                         |
| working directory      | thư mục làm việc               | Confirmed across Google, Microsoft, and Firebase Vietnamese documentation                                                                                 |
| workspace root         | gốc không gian làm việc        | Calque: "gốc" (root) + "không gian làm việc" (workspace)                                                                                                  |
| cache                  | bộ nhớ đệm                     | Confirmed in Git, util-linux, and Bazel `vi.po`/docs; "cache" heard as a loan in casual speech                                                            |
| allowlist              | danh sách cho phép             | Confirmed in Zoho and TP-Link Vietnamese product docs                                                                                                     |
| blocklist              | danh sách chặn                 | Confirmed in Zoho and TP-Link Vietnamese product docs                                                                                                     |
| template               | mẫu                            | Confirmed in Microsoft Project Vietnamese docs ("Mẫu toàn cục" = Global Template)                                                                         |
| macro                  | macro                          | Loan word; kept untranslated in [Bazel Vietnamese docs](https://bazel.build/extending?hl=vi)                                                              |
| environment variable   | biến môi trường                | Confirmed in Bazel, Git, and util-linux `vi.po`/docs                                                                                                      |
| exit status            | trạng thái thoát               | Confirmed in gcc and Git `vi.po` translations                                                                                                             |
| stage (pipeline stage) | giai đoạn                      | Common in Vietnamese devops writing, e.g. "giai đoạn deploy"                                                                                              |
| locale                 | miền địa phương                | Confirmed in Java/JSP Vietnamese tutorials; "locale" also seen as a loan                                                                                  |
| placeable              | phần giữ chỗ                   | Calque similar to "placeholder"; no established Fluent-specific Vietnamese term found                                                                     |

### Chinese, Simplified (`zh-Hans`)

Netsuke's Simplified Chinese text should address the reader as 你, not 您.
Microsoft's Simplified Chinese localization style guide states this explicitly
for the modern Microsoft voice: "Use 你 (informal) instead of 您 (formal)" and
recommends omitting the pronoun entirely where meaning stays clear, since
Chinese tolerates pronoun drop far more readily than English
([Microsoft Chinese (Simplified) style guide](https://learn.microsoft.com/en-us/globalization/reference/microsoft-style-guides),
summarized at
[ChatsControl's mirror of the same guide](https://chatscontrol.com/learn/style-guides/microsoft-zh-cn)).
您 persists in narrower registers — legal notices, government-facing text, and
some enterprise B2B documentation — but Netsuke's diagnostics and CLI help are
developer-facing technical prose, which sits squarely in the 你 register that
Microsoft, and by extension most modern developer tooling, has adopted.

Most Netsuke identifiers stay as English loan words rather than acquiring
native calques: `stdout`, `stderr`, `dyndep`, `foreach`, and `vars` are written
in Latin script exactly as in en-US, per the brief's invariant list. Genuinely
technical nouns without an established one-word gloss — `allowlist`/
`blocklist` — are translated rather than borrowed, because Chinese developer
tooling (following the industry-wide 2020 shift away from black/whitelist
terminology) already has settled native equivalents; see the discussion of
Microsoft's and other vendors' adoption of 允许列表/阻止列表 at
[IT经理网](https://www.ctocio.com/ccnews/32205.html). `build` is the one
genuinely contested term: Microsoft's own Visual Studio Simplified Chinese UI
uses 生成 (confirmed in the
[Visual Studio 2026 release notes](https://learn.microsoft.com/zh-cn/visualstudio/releases/2026/release-notes),
which mixes 生成 and 构建 depending on context), while the Rust and general
open-source developer community standardizes on 构建 — visible throughout the
[Simplified Chinese Rust Book](https://kaisery.github.io/trpl-zh-cn/ch01-03-hello-cargo.html)
and in Cargo's own diagnostics. Because Netsuke is a Rust CLI aimed at
developers already immersed in Cargo's terminology, this table follows the
community standard, 构建, for both the verb and the noun, and for compounds
such as "build graph".

Hazards: `action`/操作 collides with the generic sense of "operation", which
Bazel's own Simplified Chinese rule documentation also uses for "action"
(`一系列操作`, per the
[Bazel zh-CN rules page](https://bazel.build/extending/rules?hl=zh-cn)) —
Netsuke's diagnostics must keep 操作 scoped to "implicitly phony target" and
never reuse it loosely for arbitrary program actions. `placeable`/占位符 is the
same word Fluent-based Simplified Chinese localizations already use for Fluent
placeables (see the
[grammy.dev Fluent i18n guide](https://grammy.dev/zh/plugins/i18n)), but 占位符
also denotes ordinary UI input placeholder text, so notes explaining a
placeable must state that it is a Fluent variable substitution, not a hint
string. Punctuation is full-width (，。：；) inside Chinese sentences, per
Microsoft's documentation convention (as opposed to the mixed half/full-width
convention used inside software strings); the pause mark 、 replaces the comma
between list items, and the em dash must not appear, since it is visually
indistinguishable from 一 ("one") — both rules are set out in the
[Microsoft Chinese (Simplified) style guide](https://chatscontrol.com/learn/style-guides/microsoft-zh-cn).
Microsoft's guide does not mandate a space between Chinese text and adjacent
Latin/digit runs (the "盘古之白" convention some style guides recommend); this
document follows Microsoft's own examples, which insert no such space, for
consistency with Netsuke's other Microsoft-aligned choices. Because Fluent
plural selectors collapse to the CLDR `other` category for Chinese, Netsuke's
`.ftl` messages for this locale must not branch on plural forms. Finally,
zh-Hans and zh-Hant (Taiwan) diverge on several of these very terms — Bazel's
Traditional Chinese glossary uses 動作 for "action" and 目標/依附元件 for
"target"/"dependency" with different characters and, in the dependency case, a
different compound entirely (compare the
[zh-TW Bazel glossary](https://bazel.build/reference/glossary?hl=zh-tw)) —
which is why the two scripts are kept as separate locale sections rather than
one transliterated pair.

`Failed to load manifest at { $path }.` becomes:

```text
无法加载 { $path } 处的清单。
```

Table 38: Chinese, Simplified terminology

| en-US                  | preferred    | notes                                                                                                                              |
| ---------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| manifest               | 清单         | Standard technical sense (cf. Android/Kubernetes 清单文件); context needed to avoid the generic "checklist" reading.               |
| target                 | 目标         | Matches Bazel's zh-CN/zh-TW glossary for "target".                                                                                 |
| action                 | 操作         | Matches Bazel zh-CN rules docs ("一系列操作"); keep scoped to implicitly-phony targets, not generic "operation".                   |
| rule                   | 规则         | Matches Bazel zh-CN/zh-TW glossary for "rule".                                                                                     |
| dependency             | 依赖项       | Standard in Rust Book zh-CN and Microsoft docs; 依赖 (no 项) is common informally but less precise as a noun.                      |
| order-only dependency  | 仅顺序依赖项 | Community term from the Chinese Ninja manual translation; no official body governs Ninja terminology in zh-Hans.                   |
| build (noun)           | 构建         | Rust/OSS community standard (Cargo, Rust Book); Microsoft/Visual Studio prefers 生成 — see prose.                                  |
| build (verb)           | 构建         | Same divergence and choice as build (noun).                                                                                        |
| build graph            | 构建图       | Compound of 构建; community usage is inconsistent (生成图 also seen) so this keeps 构建 for consistency.                           |
| phony target           | 伪目标       | Established convention from Chinese GNU Make translations and community docs.                                                      |
| artefact               | 产物         | Rust/community usage ("构建产物"); Microsoft/Angular docs instead use 工件 — either is defensible, 产物 chosen for Rust alignment. |
| working directory      | 工作目录     | Standard, e.g. GitHub Actions zh-CN docs ("默认工作目录").                                                                         |
| workspace root         | 工作区根目录 | Compositional: 工作区 (workspace) + 根目录 (root directory).                                                                       |
| cache                  | 缓存         | Universal loan-free technical term; no viable English loan form in this locale.                                                    |
| allowlist              | 允许列表     | Post-2020 industry-standard replacement for "whitelist" in zh-Hans tooling.                                                        |
| blocklist              | 阻止列表     | Post-2020 industry-standard replacement for "blacklist" in zh-Hans tooling.                                                        |
| template               | 模板         | Universal technical term across CLI and web tooling.                                                                               |
| macro                  | 宏           | Universal, including Rust's own 宏 for `macro_rules!`.                                                                             |
| environment variable   | 环境变量     | Standard, e.g. IBM/GitHub Actions zh-CN docs.                                                                                      |
| exit status            | 退出状态     | Distinguished from 退出代码 ("exit code"); see Qt's zh docs, which use both terms precisely.                                       |
| stage (pipeline stage) | 阶段         | Standard CI/CD usage (build/test/deploy 阶段).                                                                                     |
| locale                 | 区域设置     | Microsoft's standard term; 语言环境 is a common community alternative, kept out of the table to avoid inconsistency.               |
| placeable              | 占位符       | Term already used for Fluent placeables in zh-Hans i18n docs; overlaps with generic UI "placeholder" — see hazards.                |

### Chinese, Traditional (`zh-Hant`)

Netsuke's Traditional Chinese output should target the Taiwan software
register, which is by far the dominant convention for developer tooling in this
script. The address form is the polite second-person pronoun 您 throughout: the
[Microsoft Traditional Chinese style guide](https://aka.ms/chinese-traditional-styleguide)
states plainly, "Always use the polite form 您 for 'You' in all of the software
products," and shows it used even in short imperative UI strings such as
請問您是否繼續? ("Would you like to continue?"). This is more persistent than
in Simplified Chinese software, where 你 is common in casual or
mainland-targeted UI; 您 should be used in every Netsuke diagnostic and prompt
that addresses the operator, not reserved for formal contexts.

Several Netsuke terms are conventionally rendered as Han-script calques rather
than English loans in Taiwan technical writing: 建置 (build), 相依性
(dependency), 快取 (cache), and 範本 (template) are all established, attested
in Microsoft's own zh-tw documentation and in the bilingual
[Taiwan/mainland computing terminology table](https://hackmd.io/@SeanPeng/B1psY1s6K).
Product and protocol names (Netsuke, Ninja, Fluent, Jinja, YAML, dyndep) stay
as English loans and are written in Latin script inline with the surrounding
Chinese text; Taiwan practice inserts no extra spacing rule beyond the usual
half-width gap around embedded Latin terms and numerals, and full-width
punctuation （，。：） is used for sentence-level punctuation, not for code
identifiers.

The Traditional/Simplified divergence is not cosmetic and the two scripts must
never be collapsed into one locale. The clearest cases: 檔案 (TW) vs 文件 (CN)
for "file" — 文件 in Taiwan usage means "document," not "file," so reusing
Simplified strings would misname every file-related diagnostic. 建置 (TW) vs
构建/生成 (CN) for "build" is the same trap in reverse: 生成 in Taiwan usage
means "generate," not "build," so a mainland string would misdescribe Netsuke's
core action. 環境變數 (TW) vs 环境变量 (CN) for "environment variable" differs
in the second morpheme (變數 "variable" vs 变量), which is easy to miss when
skimming rather than reading. A further hazard is 專案/專案檔 versus the
Simplified 项目 for "project": neither appears in this table, but translators
reusing generic project-management glossaries may import the wrong register.
Finally, 巨集 (macro) has no literal-sounding Simplified cognate (mainland 宏),
so a translator working from a Simplified source may render Netsuke's macro
directive with an unfamiliar term for Taiwan readers.

The worked example, preserving the placeable exactly:

```text
無法載入位於 { $path } 的清單檔。
```

Table 39: Chinese, Traditional terminology

| en-US                  | preferred    | notes                                                                                                                                                                          |
| ---------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| manifest               | 清單檔       | "清單" alone risks the everyday sense "list"; 清單檔 ("manifest file") disambiguates. Avoid 宣言 (political manifesto sense).                                                  |
| target                 | 目標         | Confirmed by Microsoft zh-tw MSBuild docs, e.g. 建置目標 ("build target"). Must stay distinct from action/rule.                                                                |
| action                 | 動作         | Matches Microsoft's zh-tw translation of "GitHub Action" as GitHub 動作. 操作 is reserved for generic "operate/operation."                                                     |
| rule                   | 規則         | Standard, unambiguous; distinct from 目標 and 動作.                                                                                                                            |
| dependency             | 相依性       | Taiwan usage; mainland 依赖(项). See [terminology table](https://hackmd.io/@SeanPeng/B1psY1s6K).                                                                               |
| order-only dependency  | 順序相依性   | No established external term (Ninja-specific); coined from 相依性 plus "順序" (order) to keep the family consistent.                                                           |
| build (noun)           | 建置         | Also verb form; VS/MSBuild-standard on zh-tw (mainland 生成/构建).                                                                                                             |
| build (verb)           | 建置         | Same term as noun in Taiwan software usage, matching VS/MSBuild convention.                                                                                                    |
| build graph            | 建置圖       | Compound from 建置; no independent external attestation, kept consistent with build (noun).                                                                                    |
| phony target           | 假目標       | Traditional-script GNU Make manual uses 假(想) for "phony"; some texts also use 虛擬目標.                                                                                      |
| artefact               | 產出物       | Web/CLI-register choice per the terminology table (also 成品 in Win/mac UI contexts); avoids 工件 (mainland).                                                                  |
| working directory      | 工作目錄     | Standard, unambiguous.                                                                                                                                                         |
| workspace root         | 工作區根目錄 | 工作區 confirmed (mainland 工作区); "root" appended for clarity.                                                                                                               |
| cache                  | 快取         | Taiwan; mainland 缓存.                                                                                                                                                         |
| allowlist              | 允許清單     | Confirmed Microsoft-aligned usage, replacing "whitelist."                                                                                                                      |
| blocklist              | 封鎖清單     | Confirmed Microsoft-aligned usage, replacing "blacklist."                                                                                                                      |
| template               | 範本         | Taiwan; mainland 模板.                                                                                                                                                         |
| macro                  | 巨集         | Taiwan; mainland 宏. No literal Simplified cognate — see hazards above.                                                                                                        |
| environment variable   | 環境變數     | Taiwan; mainland 环境变量. Confirmed in Microsoft zh-tw docs.                                                                                                                  |
| exit status            | 退出狀態     | Used in the official zh-tw Python documentation; 結束狀態 also seen and is acceptable as a synonym.                                                                            |
| stage (pipeline stage) | 階段         | Standard, unambiguous.                                                                                                                                                         |
| locale                 | 區域設定     | Standard Microsoft zh-tw term (Windows "地區設定"/"區域設定").                                                                                                                 |
| placeable              | 可替換項     | No established external term; Fluent-specific concept, calqued from "placeholder" (佔位符) but marked distinct since Fluent placeables are expressions, not just static slots. |
