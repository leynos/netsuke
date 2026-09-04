# Lokalisatiebronnen voor de opdrachtregel van Netsuke.

runner.io.dyndep.retention = De retentie van het gegenereerde dyndep-bestand onder { $path } kon niet worden toegepast.
cli.about = Netsuke compileert YAML- en Jinja-manifesten tot Ninja-bouwplannen.
cli.long_about = Netsuke zet YAML- en Jinja-manifesten om in reproduceerbare Ninja-grafen en voert Ninja uit met veilige standaardwaarden.
cli.usage = { $usage }

# Helptekst voor algemene opties.
cli.flag.file.help = Pad naar het te gebruiken Netsuke-manifestbestand.
cli.flag.directory.help = Uitvoeren alsof er in deze map is gestart.
cli.flag.config.help = Pad naar een configuratiebestand; slaat het automatisch zoeken over.
cli.flag.jobs.help = Stel het aantal parallelle bouwtaken in.
cli.flag.verbose.help = Schakel uitgebreide diagnostische logging en tijdsoverzichten bij afronding in.
cli.flag.locale.help = Taalmarkering voor de teksten op de opdrachtregel (bijvoorbeeld: en-US, nl).
cli.flag.fetch_allow_scheme.help = Extra URL-schema's die de fetch-helper mag gebruiken.
cli.flag.fetch_allow_host.help = Hostnamen die zijn toegestaan wanneer standaardweigering aanstaat.
cli.flag.fetch_block_host.help = Hostnamen die altijd worden geblokkeerd, ook als ze elders zijn toegestaan.
cli.flag.fetch_default_deny.help = Weiger standaard alle hosts; sta alleen de opgegeven lijst toe.
cli.flag.json.help = Geef machineleesbare JSON-uitvoer.
cli.flag.no_input.help = Lees nooit interactieve invoer.
cli.flag.color.help = Beleid voor gekleurde uitvoer (auto, always, never).
cli.flag.emoji.help = Beleid voor emoji (auto, always, never).
cli.flag.progress.help = Beleid voor het tonen van voortgang (auto, always, never).
cli.flag.accessibility.help = Beleid voor toegankelijke uitvoer (auto, on, off).
cli.flag.default_targets.help = Standaarddoelen voor de bouw wanneer er geen zijn opgegeven.

# Beschrijvingen van subopdrachten.
cli.subcommand.build.about = Bouw de doelen die in het manifest zijn gedefinieerd (standaard).
cli.subcommand.build.long_about = Bouw de gevraagde doelen; zijn er geen opgegeven, gebruik dan de standaarddoelen uit het manifest.
cli.subcommand.clean.about = Verwijder bouwartefacten via Ninja.
cli.subcommand.clean.long_about = Genereer een tijdelijk Ninja-bestand en voer daarna `ninja -t clean` uit.
cli.subcommand.graph.about = Geef de afhankelijkheidsgraaf van de bouw. De standaardindeling is DOT.
cli.subcommand.graph.long_about = Zet het ingelezen Netsuke-manifest om in een canonieke bouwgraaf en schrijf die weg als Graphviz DOT, of met `--html` als zelfstandige HTML-pagina. Gebruik `--output <BESTAND>` om naar een bestand te schrijven; `-` schrijft naar stdout.
cli.subcommand.generate.about = Genereer het Ninja-manifest zonder Ninja uit te voeren.
cli.subcommand.generate.long_about = Schrijf het gegenereerde Ninja-manifest naar stdout of naar een bestand dat met `--output` is gekozen.
cli.subcommand.help.about = Druk de hulp op het hoogste niveau af, of de hulp voor een genoemd onderwerp.
cli.subcommand.help.long_about = Zonder onderwerp komt dit overeen met `--help`. Gebruik `help targets` om de catalogus van doelen en acties voor het geselecteerde manifest af te drukken.

# Help catalogue headings and markers.
cli.help.actions_heading = Acties:
cli.help.targets_heading = Doelen:
cli.help.targets.about = Doelen en acties in het geselecteerde manifest weergeven.
cli.help.default_marker = standaard
cli.help.conditional_marker = voorwaardelijk

# Helptekst voor opties van de subopdracht build.
cli.subcommand.build.flag.targets.help = Te bouwen doelen (gebruikt de standaarddoelen uit het manifest als dit ontbreekt).

# Helptekst voor opties van de subopdracht graph.
cli.subcommand.graph.flag.html.help = Geef de graaf weer als zelfstandige HTML-pagina in plaats van als DOT.
cli.subcommand.graph.flag.output.help = Schrijf het graafartefact naar BESTAND; gebruik `-` voor stdout.

# Helptekst voor opties van de subopdracht generate.
cli.subcommand.generate.flag.output.help = Schrijf het gegenereerde Ninja-manifest naar BESTAND in plaats van naar stdout.

# Validatiefouten op de opdrachtregel.
cli.validation.jobs.invalid_number = { $value } is geen geldig getal.
cli.validation.jobs.out_of_range = Het aantal taken moet tussen { $min } en { $max } liggen.
cli.validation.scheme.empty = Het schema mag niet leeg zijn.
cli.validation.scheme.invalid_start = Het schema ‘{ $scheme }’ moet met een ASCII-letter beginnen.
cli.validation.scheme.invalid = Ongeldig schema ‘{ $scheme }’.
cli.validation.locale.empty = De taalmarkering mag niet leeg zijn.
cli.validation.locale.invalid = Ongeldige taalmarkering ‘{ $locale }’.
cli.validation.color.invalid = Ongeldig kleurbeleid ‘{ $value }’. Geldige opties: auto, always, never.
cli.validation.emoji.invalid = Ongeldig emojibeleid ‘{ $value }’. Geldige opties: auto, always, never.
cli.validation.progress.invalid = Ongeldig voortgangsbeleid ‘{ $value }’. Geldige opties: auto, always, never.
cli.validation.accessibility.invalid = Ongeldig toegankelijkheidsbeleid ‘{ $value }’. Geldige opties: auto, on, off.
cli.validation.config.expected_object = De waarden van de opdrachtregel moesten naar een object worden geserialiseerd, maar gaven { $value }.

# Foutmeldingen van Clap.
clap-error-missing-argument = Verplicht argument ontbreekt: { $argument }
clap-error-missing-subcommand = Subopdracht ontbreekt. Beschikbare opties: { $valid_subcommands }
clap-error-unknown-argument = Onbekend argument: { $argument }
clap-error-invalid-value = Ongeldige waarde voor { $argument }: { $value }
clap-error-invalid-subcommand = Onbekende subopdracht: { $subcommand }
# Let op: value-validation is anders geformuleerd dan invalid-value om fouten
# van eigen validators (ErrorKind::ValueValidation) te onderscheiden van
# typeconflicten (ErrorKind::InvalidValue).
clap-error-value-validation = Validatie mislukt voor { $argument }: { $value }

# Fouten en context van de uitvoering.
runner.manifest.not_found = Manifest ‘{ $manifest_name }’ niet gevonden in { $directory }.
runner.manifest.not_found.help = Controleer of het manifest bestaat of geef `--file` met het juiste pad op.
runner.manifest.path_missing_name = Het manifestpad ‘{ $path }’ heeft geen bestandsnaam.
cli.file.non_utf8 = Het manifestpad ‘{ $path }’ is geen geldige UTF-8.
runner.manifest.directory_label = map `{ $directory }`
runner.manifest.current_directory_label = de huidige map
runner.manifest.default_not_declared = De manifeststandaard '{ $default }' benoemt geen gedeclareerde actie of doel.
runner.context.network_policy = Het netwerkbeleid kon niet worden opgebouwd.
runner.context.load_manifest = Het manifest in { $path } kon niet worden geladen.
runner.context.serialise_manifest = Het manifest kon niet worden geserialiseerd.
runner.context.build_graph = De graaf kon niet uit het manifest worden opgebouwd.
runner.context.generate_ninja = Het Ninja-manifest kon niet worden gegenereerd.
runner.context.render_graph = Het graafartefact kon niet worden weergegeven.

runner.io.create_temp_file = Het tijdelijke Ninja-bestand kon niet worden aangemaakt.
runner.io.write_temp_ninja = Het tijdelijke Ninja-bestand kon niet worden geschreven.
runner.io.flush_temp_ninja = De buffer van het tijdelijke Ninja-bestand kon niet worden geleegd.
runner.io.sync_temp_ninja = Het tijdelijke Ninja-bestand kon niet worden gesynchroniseerd.
runner.io.create_parent_dir = De bovenliggende map { $path } kon niet worden aangemaakt.
runner.io.create_ninja_file = Het Ninja-bestand in { $path } kon niet worden aangemaakt.
runner.io.write_ninja_file = Het Ninja-bestand in { $path } kon niet worden geschreven.
runner.io.flush_ninja_file = De buffer van het Ninja-bestand in { $path } kon niet worden geleegd.
runner.io.sync_ninja_file = Het Ninja-bestand in { $path } kon niet worden gesynchroniseerd.
runner.io.open_ambient_dir = De omliggende map kon niet worden geopend.
cli.directory.non_utf8 = Het pad van de werkmap is geen geldige UTF-8. ({ $path })
runner.io.no_existing_ancestor = Er bestaat geen bovenliggende map voor { $path }.
runner.io.derive_relative_path = Het relatieve Ninja-pad kon niet worden afgeleid.
runner.io.non_utf8_path = Paden die geen UTF-8 zijn, worden niet ondersteund (pad: { $path }).
runner.io.write_stdout = Het Ninja-manifest kon niet naar stdout worden geschreven.
runner.io.flush_stdout = De buffer van stdout kon niet worden geleegd.
runner.io.dyndep.create_dir = De dyndep-map { $path } kon niet worden aangemaakt.
runner.io.dyndep.read = Het gegenereerde dyndep-bestand op { $path } kon niet worden gelezen.
runner.io.dyndep.write = Het gegenereerde dyndep-bestand op { $path } kon niet worden geschreven.
runner.io.dyndep.rename = Het gegenereerde dyndep-bestand op { $path } kon niet worden hernoemd.
runner.io.dyndep.corrupt = Het gegenereerde dyndep-bestand op { $path } komt niet overeen met de verwachte inhoud; verwijder alleen dit bestand en probeer het opnieuw.
runner.io.dyndep.temp_collisions = Er kon na herhaalde naamconflicten geen uniek tijdelijk dyndep-bestand voor { $path } worden gemaakt.
runner.io.dyndep.too_large = Het gegenereerde dyndep-bestand op { $path } overschrijdt de verificatielimiet van { $limit } bytes.

# Manifestdiagnostiek.
manifest.parse = Het inlezen van het manifest is mislukt.
manifest.structure_error = Structuurfout in het manifest bij { $name }: { $details }
manifest.yaml.parse = YAML-fout op regel { $line }, kolom { $column }: { $details }
manifest.yaml.label = ongeldige YAML
manifest.yaml.hint.tabs = YAML staat geen tabs toe; gebruik spaties om in te springen.
manifest.yaml.hint.list_item = YAML-lijstitems moeten met ‘-’ beginnen en juist zijn ingesprongen.
manifest.yaml.hint.expected_colon = Dit lijkt een item in een toewijzing; na de sleutel ontbreekt een ‘:’.
manifest.yaml.hint.mapping_values = YAML-toewijzingen vereisen een waarde na ‘:’ (of een ingesprongen blok).
manifest.yaml.hint.invalid_token = Het YAML-token is ongeldig of onverwacht.
manifest.yaml.hint.escape = Escape de backslashes of verwijder ongeldige escapereeksen.
manifest.env.missing = Een vereiste omgevingsvariabele is niet ingesteld.
manifest.env.invalid_utf8 = Een omgevingsvariabele bevat ongeldige UTF-8.
manifest.vars.not_object = De `vars` van het manifest moet een toewijzing of object zijn.
manifest.vars.reserved_name = De `vars`-sleutel '{ $name }' in het manifest is gereserveerd voor een ingebouwde sjabloonfunctie; hernoem de variabele.
manifest.read_failed = Het manifest in { $path } kon niet worden gelezen.
manifest.resolve_workspace_root = De hoofdmap van de werkruimte kon niet worden bepaald.
manifest.workspace_non_utf8 = Het hoofdpad van de werkruimte ‘{ $path }’ is geen geldige UTF-8.
manifest.path_non_utf8 = Het pad van manifest ‘{ $manifest }’ is geen geldige UTF-8: { $path }.
manifest.path_missing_name = Het manifestpad ‘{ $path }’ heeft geen bestandsnaam.
manifest.open_workspace_failed = De werkruimte { $workspace } kon niet worden geopend voor manifest { $manifest }.
manifest.foreach.not_iterable = De expressie `foreach` is niet doorloopbaar.
manifest.foreach.serialise_item = Het item van `foreach` kon niet worden geserialiseerd.
manifest.when.empty = De expressie `when` mag niet leeg zijn.
manifest.when.eval_error = De expressie `when` ‘{ $expr }’ kon niet worden geëvalueerd.
manifest.when.template_error = De sjabloon `when` ‘{ $expr }’ kon niet worden weergegeven.
manifest.target.vars_not_object = De `vars` van het doel moet een object zijn, maar gaf { $value }.
manifest.vars.entry_not_object = Een `vars`-item van het manifest moet een object zijn.
manifest.field_not_string = Het veld ‘{ $field }’ moet een tekenreeks zijn.
manifest.expression.parse_error = De expressie { $name } kon niet worden ingelezen.
manifest.expression.eval_error = De expressie { $name } kon niet worden geëvalueerd.

# Diagnostiek voor manifestmacro's.
manifest.macro.signature_missing_identifier = In de macrodefinitie ontbreekt een naam.
manifest.macro.signature_missing_params = In de macrodefinitie ontbreken parameters.
manifest.macro.compile_failed = De macro { $name } kon niet worden gecompileerd.
manifest.macro.sequence_invalid = Macro's moeten worden gedefinieerd als een toewijzing van namen aan sjablonen.
manifest.macro.register_failed = De macro's van het manifest konden niet worden geregistreerd.
manifest.macro.not_initialised = De macro-omgeving is niet geïnitialiseerd.
manifest.macro.caller_invalid = De aanroeper van de macro moet een tekenreeks zijn.
manifest.macro.template_load_failed = De macrosjabloon kon niet worden geladen.
manifest.macro.init_failed = De macro-omgeving kon niet worden geïnitialiseerd.
manifest.macro.missing = De macro { $name } ontbreekt.

# Glob-fouten in het manifest.
manifest.glob.unmatched_brace = Ongeldig glob-patroon ‘{ $pattern }’: ‘{ $character }’ zonder tegenhanger op positie { $position }.
manifest.glob.invalid_pattern = Ongeldig glob-patroon ‘{ $pattern }’: { $detail }.
manifest.glob.unknown_pattern_error = onbekende patroonfout.
manifest.glob.io_failed = Glob is mislukt voor ‘{ $pattern }’: { $detail }.
manifest.glob.unknown_io_error = onbekende I/O-fout.
manifest.command_list_empty = Het veld ‘command’ mag niet leeg zijn: geef een opdrachtreeks of een niet-lege lijst op.

# Fouten in de tussenrepresentatie.
ir.rule_not_found = De regel ‘{ $rule }’ waarnaar doel ‘{ $target }’ verwijst, is niet gevonden.
ir.multiple_rules = Doel ‘{ $target }’ moet naar precies één regel verwijzen, maar gaf { $rules }.
ir.empty_rule = Doel ‘{ $target }’ moet naar een regel verwijzen.
ir.duplicate_outputs = Dubbele uitvoer aangetroffen: { $outputs }.
ir.circular_dependency = Circulaire afhankelijkheid aangetroffen: { $cycle }.
ir.action_serialisation = De actie kon niet worden geserialiseerd: { $details }.
ir.invalid_command = Ongeldige interpolatie in de opdracht: { $snippet }.

# Fouten bij het genereren van Ninja.
ninja_gen.missing_action = De actie ‘{ $id }’ waarnaar een bouwtak verwijst, ontbreekt.
ninja_gen.format = De uitvoer van het Ninja-manifest kon niet worden opgemaakt.
ninja_gen.dyndep_files_required = Deze bewerking vereist een gegenereerde Ninja-bundel; gebruik `netsuke build`, `netsuke clean` of `netsuke generate` om de dyndep-bestanden te materialiseren.
ninja_gen.reserved_output_path = Het pad '{ $path }' is gereserveerd voor de seriële afhankelijkheidsstatus van Netsuke.
ninja_gen.unsupported_path_character = Het pad '{ $path }' bevat het niet-ondersteunde Ninja-padteken '{ $character }'.

# Validatie van hostpatronen.
host_pattern.empty = Het hostpatroon mag niet leeg zijn.
host_pattern.contains_scheme = Het hostpatroon ‘{ $pattern }’ mag geen URL-schema bevatten.
host_pattern.contains_slash = Het hostpatroon ‘{ $pattern }’ mag geen ‘/’ bevatten.
host_pattern.missing_suffix = Het hostpatroon ‘{ $pattern }’ moet een achtervoegsel na ‘*.’ bevatten.
host_pattern.empty_label = Het hostpatroon ‘{ $pattern }’ bevat een leeg label.
host_pattern.invalid_chars = Het hostpatroon ‘{ $pattern }’ bevat ongeldige tekens.
host_pattern.invalid_label_edge = Labels in het hostpatroon ‘{ $pattern }’ mogen niet met ‘-’ beginnen of eindigen.
host_pattern.label_too_long = Het hostpatroon ‘{ $pattern }’ bevat een label van meer dan 63 tekens.
host_pattern.too_long = Het hostpatroon ‘{ $pattern }’ overschrijdt de limiet van 255 tekens.

# Netwerkbeleid.
network_policy.scheme.empty = Het schema mag niet leeg zijn.
network_policy.scheme.invalid = Het schema ‘{ $scheme }’ bevat ongeldige tekens.
network_policy.allowlist.empty = De lijst met toegestane hosts mag niet leeg zijn.
network_policy.scheme.not_allowed = Het schema ‘{ $scheme }’ is niet toegestaan.
network_policy.missing_host = In de URL ontbreekt een host.
network_policy.host.blocked = Host ‘{ $host }’ wordt door het beleid geblokkeerd.
network_policy.host.not_allowlisted = Host ‘{ $host }’ staat niet op de lijst met toegestane hosts.

# Configuratie van de standaardbibliotheek.
stdlib.config.default_fetch_cache_invalid = Het standaardpad van de fetch-cache moet relatief zijn.
stdlib.config.default_which_cache_invalid = De standaardcapaciteit van de which-cache moet positief zijn.
stdlib.config.workspace_root_absolute = Het hoofdpad van de werkruimte moet absoluut zijn.
stdlib.config.fetch_response_limit_positive = De antwoordlimiet van fetch moet positief zijn.
stdlib.config.command_output_limit_positive = De limiet voor vastgelegde opdrachtuitvoer moet positief zijn.
stdlib.config.command_stream_limit_positive = De streamlimiet voor opdrachten moet positief zijn.
stdlib.config.which_cache_capacity_positive = De capaciteit van de which-cache moet positief zijn.
stdlib.config.skip_dir_empty = Items voor over te slaan mappen mogen niet leeg zijn.
stdlib.config.skip_dir_navigation = Items voor over te slaan mappen mogen geen ‘..’ bevatten.
stdlib.config.skip_dir_separator = Items voor over te slaan mappen mogen geen padscheidingstekens bevatten.
stdlib.config.fetch_cache_empty = Het pad van de fetch-cache mag niet leeg zijn.
stdlib.config.fetch_cache_not_relative = Het pad van de fetch-cache moet relatief zijn, maar gaf { $path }.
stdlib.config.fetch_cache_escapes = Het pad van de fetch-cache mag de werkruimte niet verlaten: { $path }.
stdlib.config.open_workspace_root = De huidige map kon niet worden geopend als hoofdmap van de stdlib-werkruimte.
stdlib.config.resolve_cwd = De huidige map kon niet worden bepaald als hoofdmap van de stdlib-werkruimte.
stdlib.config.cwd_non_utf8 = De huidige map bevat delen die geen UTF-8 zijn: { $path }.

# Diagnostiek van de fetch-helper.
stdlib.fetch.url_invalid = Ongeldige URL ‘{ $url }’: { $details }.
stdlib.fetch.disallowed = De URL ‘{ $url }’ is niet toegestaan: { $details }.
stdlib.fetch.failed = ‘{ $url }’ kon niet worden opgehaald: { $details }.
stdlib.fetch.cache_read_failed = Het cache-item ‘{ $name }’ kon niet worden gelezen: { $details }.
stdlib.fetch.cache_open_failed = Het cache-item ‘{ $name }’ kon niet worden geopend: { $details }.
stdlib.fetch.response_read_failed = Het antwoord van ‘{ $url }’ kon niet worden gelezen: { $details }.
stdlib.fetch.response_buffer_overflow = Bufferoverloop tijdens het lezen van ‘{ $url }’.
stdlib.fetch.cache_write_failed = De cache voor ‘{ $url }’ kon niet worden geschreven: { $details }.
stdlib.fetch.response_limit_exceeded = Het antwoord van ‘{ $url }’ overschreed de limiet van { $limit } bytes.
stdlib.fetch.cache_limit_exceeded = Het gecachete antwoord ‘{ $name }’ overschreed de limiet van { $limit } bytes.
stdlib.fetch.io_failed = { $action } is mislukt voor { $path }: { $details }.
stdlib.fetch.action.sync_cache = synchroniseren van de fetch-cache
stdlib.fetch.action.create_cache_dir = aanmaken van de fetch-cachemap
stdlib.fetch.action.open_cache_dir = openen van de fetch-cachemap
stdlib.fetch.action.stat_cache = opvragen van het item in de fetch-cache
stdlib.fetch.action.open_cache_entry = openen van het item in de fetch-cache

# Diagnostiek van de opdrachthelper.
stdlib.command.location = opdracht ‘{ $command }’ in sjabloon ‘{ $template }’
stdlib.command.spawn_failed = { $location } kon niet worden gestart: { $details }.
stdlib.command.io_failed = { $location } is mislukt: { $details }.
stdlib.command.closed_input_early = De invoer sloot voordat het schrijven naar de opdracht klaar was.
stdlib.command.broken_pipe = Verbroken pipe tijdens het uitvoeren van { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } is door een signaal beëindigd.
stdlib.command.exited_with_status = { $location } is geëindigd met status { $status }.
stdlib.command.output_limit_exceeded = { $location } overschreed de { $mode }-limiet van { $limit } bytes voor { $stream }.
stdlib.command.timeout = { $location } overschreed de tijdslimiet van { $seconds } seconden.
stdlib.command.exit_status_suffix = (afsluitstatus { $status })
stdlib.command.signal_suffix = (door een signaal beëindigd)
stdlib.command.shell.empty = De shell-opdracht mag niet leeg zijn.
stdlib.command.grep.empty_pattern = Het grep-patroon mag niet leeg zijn.
stdlib.command.grep.flags_not_string = Vlaggen voor grep moeten tekenreeksen zijn.
stdlib.command.quote.invalid = { $arg } kon niet tussen aanhalingstekens worden gezet: { $details }.
stdlib.command.quote.line_break = Argumenten met een regelterugloop of regeleinde kunnen niet veilig tussen aanhalingstekens worden gezet.
stdlib.command.input_undefined = De invoerwaarde is niet gedefinieerd.
stdlib.command.tempfile.root_required = De hoofdmap van de werkruimte is vereist om tijdelijke opdrachtbestanden aan te maken.
stdlib.command.tempfile.create_failed = Het tijdelijke opdrachtbestand kon niet worden aangemaakt: { $details }.
stdlib.command.options.invalid_utf8 = De sleutel van een opdrachtoptie moet geldige UTF-8 zijn.
stdlib.command.option.mode_not_string = De uitvoermodus moet een tekenreeks zijn.
stdlib.command.options.invalid_type = Opdrachtopties moeten een object zijn.
stdlib.command.output.mode_unsupported = De uitvoermodus ‘{ $mode }’ wordt niet ondersteund.
stdlib.command.output.mode.capture = vastleggen
stdlib.command.output.mode.streaming = streamen
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostiek van de padhelper.
stdlib.path.io.failed = { $action } is mislukt voor { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } is mislukt voor { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } is mislukt voor { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = niet gevonden
stdlib.path.io.permission_denied = toegang geweigerd
stdlib.path.io.already_exists = bestaat al
stdlib.path.io.invalid_input = ongeldige invoer
stdlib.path.io.invalid_data = ongeldige gegevens
stdlib.path.io.timed_out = tijdslimiet verstreken
stdlib.path.io.interrupted = onderbroken
stdlib.path.io.would_block = zou blokkeren
stdlib.path.io.write_zero = nul bytes geschreven
stdlib.path.io.unexpected_eof = onverwacht einde van bestand
stdlib.path.io.broken_pipe = verbroken pipe
stdlib.path.io.connection_refused = verbinding geweigerd
stdlib.path.io.connection_reset = verbinding opnieuw ingesteld
stdlib.path.io.connection_aborted = verbinding afgebroken
stdlib.path.io.not_connected = niet verbonden
stdlib.path.io.addr_in_use = adres al in gebruik
stdlib.path.io.addr_not_available = adres niet beschikbaar
stdlib.path.io.out_of_memory = geen geheugen meer
stdlib.path.io.unsupported = niet ondersteund
stdlib.path.io.file_too_large = bestand te groot
stdlib.path.io.resource_busy = bron bezet
stdlib.path.io.executable_busy = uitvoerbaar bestand bezet
stdlib.path.io.deadlock = impasse
stdlib.path.io.crosses_devices = overschrijdt apparaten
stdlib.path.io.too_many_links = te veel koppelingen
stdlib.path.io.invalid_filename = ongeldige bestandsnaam
stdlib.path.io.arg_list_too_long = argumentenlijst te lang
stdlib.path.io.stale_handle = verouderde netwerkbestandsverwijzing
stdlib.path.io.storage_full = opslag vol
stdlib.path.io.not_seekable = niet doorzoekbaar
stdlib.path.io.network_down = netwerk ligt plat
stdlib.path.io.network_unreachable = netwerk onbereikbaar
stdlib.path.io.host_unreachable = host onbereikbaar
stdlib.path.io.other = I/O-fout
stdlib.path.action.canonicalize = canoniseren
stdlib.path.action.open_directory = openen van de map
stdlib.path.action.stat = opvragen
stdlib.path.action.read = lezen
stdlib.path.action.open_file = openen van het bestand
stdlib.path.with_suffix.empty_separator = with_suffix vereist een scheidingsteken dat niet leeg is.
stdlib.path.relative_to.mismatch = { $path } is niet relatief ten opzichte van { $root }.
stdlib.path.expanduser.unsupported = Gebruikerspecifieke uitbreiding van ~ wordt niet ondersteund.
stdlib.path.expanduser.no_home = ~ kan niet worden uitgebreid: er zijn geen omgevingsvariabelen voor de thuismap ingesteld.
stdlib.path.contents.unsupported_encoding = De tekencodering ‘{ $encoding }’ wordt niet ondersteund.
stdlib.path.hash.unsupported_algorithm = Het hash-algoritme ‘{ $algorithm }’ wordt niet ondersteund.
stdlib.path.hash.unsupported_algorithm_legacy = Het hash-algoritme ‘{ $algorithm }’ wordt niet ondersteund (schakel functie ‘{ $feature }’ in).

# Diagnostiek van de verzamelinghelpers.
stdlib.collections.flatten.expected_sequence = flatten verwachtte items uit een reeks, maar vond { $kind }.
stdlib.collections.group_by.empty_attribute = group_by vereist een attribuut dat niet leeg is.
stdlib.collections.group_by.unresolved = group_by kon ‘{ $attr }’ niet vinden op een item van het type { $kind }.

# Diagnostiek van de tijdhelpers.
stdlib.time.offset.invalid = De verschuiving voor now ‘{ $offset }’ is ongeldig: verwacht werd ‘+HH:MM[:SS]’ of ‘Z’.
stdlib.time.timedelta.overflow = Overloop in timedelta bij het optellen van { $component }.
stdlib.time.label.weeks = weken
stdlib.time.label.days = dagen
stdlib.time.label.hours = uren
stdlib.time.label.minutes = minuten
stdlib.time.label.seconds = seconden
stdlib.time.label.milliseconds = milliseconden
stdlib.time.label.microseconds = microseconden
stdlib.time.label.nanoseconds = nanoseconden

# Diagnostiek van de which-helper.
stdlib.which.not_found = [netsuke::jinja::which::not_found] opdracht ‘{ $command }’ niet gevonden na het doorlopen van { $count } PATH-items. Voorbeeld: { $preview }
stdlib.which.not_found.hint.cwd_auto = Automatic mode searches only directories explicitly named by PATH.
stdlib.which.not_found.hint.cwd_always = Always mode searches only the current directory before directories named by PATH.
stdlib.which.not_found.hint.workspace = To recursively search the workspace tree, use cwd_mode="workspace-recursive".
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] de opdracht ‘{ $command }’ in ‘{ $path }’ ontbreekt of is niet uitvoerbaar.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <leeg>
stdlib.which.path_entry.non_utf8 = PATH-item nr. { $index } bevat tekens die geen UTF-8 zijn; Netsuke vereist UTF-8-paden.
stdlib.which.command.empty = which vereist een tekenreeks die niet leeg is.
stdlib.which.cwd_mode.invalid = cwd_mode must be 'auto', 'always', 'never', or 'workspace-recursive', got '{ $mode }'.
stdlib.which.cwd.resolve_failed = De huidige map kon niet worden bepaald: { $details }.
stdlib.which.cwd.non_utf8 = De huidige map bevat delen die geen UTF-8 zijn.
stdlib.which.canonicalize_failed = ‘{ $path }’ kon niet worden gecanoniseerd: { $details }.
stdlib.which.is_executable = Er kon niet worden vastgesteld of ‘{ $path }’ uitvoerbaar is: { $details }.
stdlib.which.canonicalize_non_utf8 = Het canonieke pad bevat delen die geen UTF-8 zijn.
stdlib.which.workspace_non_utf8 = Het pad van de werkruimte bevat delen die geen UTF-8 zijn bij het opzoeken van opdracht ‘{ $command }’: { $path }.
stdlib.which.walkdir_error = Fout bij het doorlopen van de werkruimte tijdens het opzoeken van de opdracht: { $details }.

# Registratie van de standaardbibliotheek.
stdlib.register.open_dir = De huidige map kon niet worden geopend voor de registratie van stdlib.
stdlib.register.resolve_dir = De huidige map kon niet worden bepaald voor de registratie van stdlib.
stdlib.register.dir_non_utf8 = De huidige map bevat delen die geen UTF-8 zijn: { $path }.

# Statusrapportage voor de toegankelijke uitvoermodus.
status.state.pending = in de wachtrij
status.state.running = bezig
status.state.done = klaar
status.state.failed = mislukt
status.stage.label = Stap { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Taak { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Manifestbestand lezen
status.stage.initial_yaml_parsing = YAML-document inlezen
status.stage.template_expansion = Sjabloondirectieven uitbreiden
status.stage.final_rendering = Manifestwaarden deserialiseren en weergeven
status.stage.ir_generation_validation = Afhankelijkheidsgraaf opbouwen en controleren
status.stage.ninja_synthesis = Ninja-bouwplan samenstellen
status.stage.ninja_synthesis_execute = Ninja-plan samenstellen en { $tool } uitvoeren
status.stage.graph_rendering = Graafartefact weergeven
status.stage.graph_rendering_with_tool = { $tool } weergeven
status.complete = { $tool } voltooid.
status.timing.summary_header = Tijdsoverzicht per stap:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Totale tijd van de keten: { $duration }
status.tool.build = Bouw
status.tool.clean = Opruimen
status.tool.graph = Graaf
status.tool.graph_html = Graaf (HTML)
status.tool.generate = Genereren
status.tool.help_targets = Doelhulp

# Teksten van de HTML-weergave van de graaf.
graph.html.title = Netsuke-bouwgraaf
graph.html.heading = Netsuke-bouwgraaf
graph.html.description = Bouwgraaf weergegeven door Netsuke
graph.html.outline.summary = Doelen en afhankelijkheden (tekstoverzicht)
graph.html.outline.no_inputs = Geen invoer
graph.html.noscript.notice = JavaScript staat uit. Het tekstoverzicht hierboven is de volledige graaf; de DOT-broncode volgt hieronder.

# Semantische voorvoegsels voor toegankelijke uitvoer.
semantic.prefix.error = Fout:
semantic.prefix.warning = Waarschuwing:
semantic.prefix.success = Gelukt:
semantic.prefix.info = Info:
semantic.prefix.timing = Tijd:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Voorbeelden van meervoudsvormen voor vertalers.
# Het Nederlands gebruikt de CLDR-categorieën `one` en `other`, net als de
# brontaal.
example.files_processed = { $count ->
    [one] { $count } bestand verwerkt.
   *[other] { $count } bestanden verwerkt.
}

example.errors_found = { $count ->
    [0] Geen fouten gevonden.
    [one] { $count } fout gevonden.
   *[other] { $count } fouten gevonden.
}
