# Lokalisierungsressourcen für die Netsuke-CLI.

cli.about = Netsuke übersetzt YAML- + Jinja-Manifeste in Ninja-Build-Pläne.
cli.long_about = Netsuke wandelt YAML- + Jinja-Manifeste in reproduzierbare Ninja-Graphen um und führt Ninja mit sicheren Voreinstellungen aus.
cli.usage = { $usage }

# Hilfetext für globale Optionen.
cli.flag.file.help = Pfad zur zu verwendenden Netsuke-Manifestdatei.
cli.flag.directory.help = So ausführen, als wäre in diesem Verzeichnis gestartet worden.
cli.flag.config.help = Pfad zu einer Konfigurationsdatei; überspringt die automatische Suche.
cli.flag.jobs.help = Anzahl der parallelen Build-Jobs festlegen.
cli.flag.verbose.help = Ausführliche Diagnoseprotokolle und Zeitübersichten nach Abschluss aktivieren.
cli.flag.locale.help = Sprachkennung für CLI-Texte (zum Beispiel: en-US, de).
cli.flag.fetch_allow_scheme.help = Zusätzliche URL-Schemata, die der fetch-Helfer verwenden darf.
cli.flag.fetch_allow_host.help = Hostnamen, die bei aktivierter Standardsperre zugelassen sind.
cli.flag.fetch_block_host.help = Hostnamen, die immer blockiert werden, auch wenn sie anderweitig erlaubt sind.
cli.flag.fetch_default_deny.help = Alle Hosts standardmäßig sperren; nur die deklarierte Positivliste zulassen.
cli.flag.json.help = Maschinenlesbare JSON-Ausgabe erzeugen.
cli.flag.no_input.help = Niemals interaktive Eingaben lesen.
cli.flag.color.help = Richtlinie für Farbausgabe (auto, always, never).
cli.flag.emoji.help = Emoji-Richtlinie (auto, always, never).
cli.flag.progress.help = Richtlinie für Fortschrittsanzeige (auto, always, never).
cli.flag.accessibility.help = Richtlinie für barrierefreie Ausgabe (auto, on, off).
cli.flag.default_targets.help = Standard-Build-Ziele, wenn keine angegeben werden.

# Beschreibungen der Unterbefehle.
cli.subcommand.build.about = Im Manifest definierte Ziele bauen (Standard).
cli.subcommand.build.long_about = Die angeforderten Ziele bauen; ohne Angabe werden die Standardziele des Manifests verwendet.
cli.subcommand.clean.about = Build-Artefakte über Ninja entfernen.
cli.subcommand.clean.long_about = Eine temporäre Ninja-Datei erzeugen und anschließend `ninja -t clean` ausführen.
cli.subcommand.graph.about = Den Build-Abhängigkeitsgraphen ausgeben. Standardformat ist DOT.
cli.subcommand.graph.long_about = Das eingelesene Netsuke-Manifest in einen kanonischen Build-Graphen überführen und als Graphviz-DOT ausgeben oder mit `--html` als eigenständige HTML-Seite. Mit `--output <DATEI>` in eine Datei schreiben; `-` schreibt nach stdout.
cli.subcommand.generate.about = Das Ninja-Manifest erzeugen, ohne Ninja auszuführen.
cli.subcommand.generate.long_about = Das erzeugte Ninja-Manifest nach stdout schreiben oder in eine mit `--output` gewählte Datei.

# Hilfetext für Optionen des Unterbefehls build.
cli.subcommand.build.flag.targets.help = Zu bauende Ziele (ohne Angabe gelten die Standardziele des Manifests).

# Hilfetext für Optionen des Unterbefehls graph.
cli.subcommand.graph.flag.html.help = Den Graphen statt als DOT als eigenständige HTML-Seite rendern.
cli.subcommand.graph.flag.output.help = Das Graph-Artefakt in DATEI schreiben; `-` für stdout verwenden.

# Hilfetext für Optionen des Unterbefehls generate.
cli.subcommand.generate.flag.output.help = Das erzeugte Ninja-Manifest statt nach stdout in DATEI schreiben.

# Validierungsfehler der CLI.
cli.validation.jobs.invalid_number = { $value } ist keine gültige Zahl.
cli.validation.jobs.out_of_range = Die Job-Anzahl muss zwischen { $min } und { $max } liegen.
cli.validation.scheme.empty = Das Schema darf nicht leer sein.
cli.validation.scheme.invalid_start = Das Schema „{ $scheme }“ muss mit einem ASCII-Buchstaben beginnen.
cli.validation.scheme.invalid = Ungültiges Schema „{ $scheme }“.
cli.validation.locale.empty = Die Sprachkennung darf nicht leer sein.
cli.validation.locale.invalid = Ungültige Sprachkennung „{ $locale }“.
cli.validation.color.invalid = Ungültige Farbrichtlinie „{ $value }“. Gültige Optionen: auto, always, never.
cli.validation.emoji.invalid = Ungültige Emoji-Richtlinie „{ $value }“. Gültige Optionen: auto, always, never.
cli.validation.progress.invalid = Ungültige Fortschrittsrichtlinie „{ $value }“. Gültige Optionen: auto, always, never.
cli.validation.accessibility.invalid = Ungültige Barrierefreiheitsrichtlinie „{ $value }“. Gültige Optionen: auto, on, off.
cli.validation.config.expected_object = Die eingelesenen CLI-Werte sollten als Objekt serialisiert werden, erhalten wurde { $value }.

# Fehlermeldungen von Clap.
clap-error-missing-argument = Erforderliches Argument fehlt: { $argument }
clap-error-missing-subcommand = Unterbefehl fehlt. Verfügbare Optionen: { $valid_subcommands }
clap-error-unknown-argument = Unbekanntes Argument: { $argument }
clap-error-invalid-value = Ungültiger Wert für { $argument }: { $value }
clap-error-invalid-subcommand = Unbekannter Unterbefehl: { $subcommand }
# Hinweis: value-validation ist bewusst anders formuliert als invalid-value, um
# Fehler eigener Validierer (ErrorKind::ValueValidation) von Typkonflikten
# (ErrorKind::InvalidValue) zu unterscheiden.
clap-error-value-validation = Validierung fehlgeschlagen für { $argument }: { $value }

# Fehler und Kontexte des Runners.
runner.manifest.not_found = Manifest „{ $manifest_name }“ wurde in { $directory } nicht gefunden.
runner.manifest.not_found.help = Stellen Sie sicher, dass das Manifest existiert, oder geben Sie `--file` mit dem richtigen Pfad an.
runner.manifest.path_missing_name = Der Manifestpfad „{ $path }“ enthält keinen Dateinamen.
runner.manifest.path_utf8 = Der Manifestpfad „{ $path }“ ist kein gültiges UTF-8.
runner.manifest.directory_utf8 = Der Pfad des Manifestverzeichnisses „{ $path }“ ist kein gültiges UTF-8.
runner.manifest.directory_label = Verzeichnis `{ $directory }`
runner.manifest.current_directory_label = das aktuelle Verzeichnis
runner.context.network_policy = Die Netzwerkrichtlinie konnte nicht erstellt werden.
runner.context.load_manifest = Das Manifest unter { $path } konnte nicht geladen werden.
runner.context.serialise_manifest = Das Manifest konnte nicht serialisiert werden.
runner.context.build_graph = Aus dem Manifest konnte kein Graph erstellt werden.
runner.context.generate_ninja = Das Ninja-Manifest konnte nicht erzeugt werden.
runner.context.render_graph = Das Graph-Artefakt konnte nicht gerendert werden.

runner.io.create_temp_file = Die temporäre Ninja-Datei konnte nicht erstellt werden.
runner.io.write_temp_ninja = Die temporäre Ninja-Datei konnte nicht geschrieben werden.
runner.io.flush_temp_ninja = Die temporäre Ninja-Datei konnte nicht geleert werden.
runner.io.sync_temp_ninja = Die temporäre Ninja-Datei konnte nicht synchronisiert werden.
runner.io.create_parent_dir = Das übergeordnete Verzeichnis { $path } konnte nicht erstellt werden.
runner.io.create_ninja_file = Die Ninja-Datei unter { $path } konnte nicht erstellt werden.
runner.io.write_ninja_file = Die Ninja-Datei unter { $path } konnte nicht geschrieben werden.
runner.io.flush_ninja_file = Die Ninja-Datei unter { $path } konnte nicht geleert werden.
runner.io.sync_ninja_file = Die Ninja-Datei unter { $path } konnte nicht synchronisiert werden.
runner.io.open_ambient_dir = Das umgebende Verzeichnis konnte nicht geöffnet werden.
runner.io.no_existing_ancestor = Für { $path } existiert kein übergeordnetes Verzeichnis.
runner.io.derive_relative_path = Der relative Ninja-Pfad konnte nicht abgeleitet werden.
runner.io.non_utf8_path = Pfade ohne gültiges UTF-8 werden nicht unterstützt (Pfad: { $path }).
runner.io.write_stdout = Das Ninja-Manifest konnte nicht nach stdout geschrieben werden.
runner.io.flush_stdout = stdout konnte nicht geleert werden.

# Manifest-Diagnosen.
manifest.parse = Das Einlesen des Manifests ist fehlgeschlagen.
manifest.structure_error = Strukturfehler im Manifest bei { $name }: { $details }
manifest.yaml.parse = YAML-Fehler in Zeile { $line }, Spalte { $column }: { $details }
manifest.yaml.label = ungültiges YAML
manifest.yaml.hint.tabs = YAML erlaubt keine Tabulatoren; verwenden Sie Leerzeichen zur Einrückung.
manifest.yaml.hint.list_item = YAML-Listeneinträge müssen mit „-“ beginnen und korrekt eingerückt sein.
manifest.yaml.hint.expected_colon = Das sieht nach einem Mapping-Eintrag aus; nach dem Schlüssel fehlt ein „:“.
manifest.yaml.hint.mapping_values = YAML-Mappings benötigen nach „:“ einen Wert (oder einen eingerückten Block).
manifest.yaml.hint.invalid_token = Das YAML-Token ist ungültig oder unerwartet.
manifest.yaml.hint.escape = Maskieren Sie Backslashes oder entfernen Sie ungültige Escape-Sequenzen.
manifest.env.missing = Die erforderliche Umgebungsvariable „{ $name }“ ist nicht gesetzt.
manifest.env.invalid_utf8 = Die Umgebungsvariable „{ $name }“ enthält ungültiges UTF-8.
manifest.vars.not_object = `vars` im Manifest muss eine Zuordnung bzw. ein Objekt sein.
manifest.read_failed = Das Manifest unter { $path } konnte nicht gelesen werden.
manifest.resolve_workspace_root = Das Wurzelverzeichnis des Arbeitsbereichs konnte nicht ermittelt werden.
manifest.workspace_non_utf8 = Der Wurzelpfad des Arbeitsbereichs „{ $path }“ ist kein gültiges UTF-8.
manifest.path_non_utf8 = Der Pfad des Manifests „{ $manifest }“ ist kein gültiges UTF-8: { $path }.
manifest.path_missing_name = Der Manifestpfad „{ $path }“ enthält keinen Dateinamen.
manifest.open_workspace_failed = Der Arbeitsbereich { $workspace } konnte für das Manifest { $manifest } nicht geöffnet werden.
manifest.foreach.not_iterable = Der Ausdruck `foreach` ist nicht iterierbar.
manifest.foreach.serialise_item = Das foreach-Element konnte nicht serialisiert werden.
manifest.when.empty = Der Ausdruck `when` darf nicht leer sein.
manifest.when.eval_error = Der Ausdruck `when` „{ $expr }“ konnte nicht ausgewertet werden.
manifest.when.template_error = Die Vorlage `when` „{ $expr }“ konnte nicht gerendert werden.
manifest.target.vars_not_object = `vars` des Ziels muss ein Objekt sein, erhalten wurde { $value }.
manifest.vars.entry_not_object = Ein `vars`-Eintrag des Manifests muss ein Objekt sein.
manifest.field_not_string = Das Feld „{ $field }“ muss eine Zeichenkette sein.
manifest.expression.parse_error = Der Ausdruck { $name } konnte nicht eingelesen werden.
manifest.expression.eval_error = Der Ausdruck { $name } konnte nicht ausgewertet werden.

# Diagnosen zu Manifest-Makros.
manifest.macro.signature_missing_identifier = Der Makro-Signatur fehlt ein Bezeichner.
manifest.macro.signature_missing_params = Der Makro-Signatur fehlen Parameter.
manifest.macro.compile_failed = Das Makro { $name } konnte nicht kompiliert werden.
manifest.macro.sequence_invalid = Makros müssen als Zuordnung von Namen zu Vorlagen definiert werden.
manifest.macro.register_failed = Die Manifest-Makros konnten nicht registriert werden.
manifest.macro.not_initialised = Die Makro-Umgebung ist nicht initialisiert.
manifest.macro.caller_invalid = Der Makro-Aufrufer muss eine Zeichenkette sein.
manifest.macro.template_load_failed = Die Makro-Vorlage konnte nicht geladen werden.
manifest.macro.init_failed = Die Makro-Umgebung konnte nicht initialisiert werden.
manifest.macro.missing = Das Makro { $name } fehlt.

# Glob-Fehler im Manifest.
manifest.glob.unmatched_brace = Ungültiges Glob-Muster „{ $pattern }“: „{ $character }“ ohne Gegenstück an Position { $position }.
manifest.glob.invalid_pattern = Ungültiges Glob-Muster „{ $pattern }“: { $detail }.
manifest.glob.unknown_pattern_error = unbekannter Musterfehler.
manifest.glob.io_failed = Glob für „{ $pattern }“ fehlgeschlagen: { $detail }.
manifest.glob.unknown_io_error = unbekannter E/A-Fehler.

# Fehler der Zwischendarstellung.
ir.rule_not_found = Die vom Ziel „{ $target }“ referenzierte Regel „{ $rule }“ wurde nicht gefunden.
ir.multiple_rules = Das Ziel „{ $target }“ muss genau eine Regel referenzieren, erhalten wurde { $rules }.
ir.empty_rule = Das Ziel „{ $target }“ muss eine Regel referenzieren.
ir.duplicate_outputs = Doppelte Ausgaben erkannt: { $outputs }.
ir.circular_dependency = Zyklische Abhängigkeit erkannt: { $cycle }.
ir.action_serialisation = Die Aktion konnte nicht serialisiert werden: { $details }.
ir.invalid_command = Ungültige Befehlsinterpolation: { $snippet }.

# Fehler bei der Ninja-Erzeugung.
ninja_gen.missing_action = Die von einer Build-Kante referenzierte Aktion „{ $id }“ fehlt.
ninja_gen.format = Die Ausgabe des Ninja-Manifests konnte nicht formatiert werden.

# Validierung von Host-Mustern.
host_pattern.empty = Das Host-Muster darf nicht leer sein.
host_pattern.contains_scheme = Das Host-Muster „{ $pattern }“ darf kein URL-Schema enthalten.
host_pattern.contains_slash = Das Host-Muster „{ $pattern }“ darf kein „/“ enthalten.
host_pattern.missing_suffix = Das Host-Muster „{ $pattern }“ muss nach „*.“ ein Suffix enthalten.
host_pattern.empty_label = Das Host-Muster „{ $pattern }“ enthält ein leeres Label.
host_pattern.invalid_chars = Das Host-Muster „{ $pattern }“ enthält ungültige Zeichen.
host_pattern.invalid_label_edge = Labels des Host-Musters „{ $pattern }“ dürfen nicht mit „-“ beginnen oder enden.
host_pattern.label_too_long = Das Host-Muster „{ $pattern }“ enthält ein Label mit mehr als 63 Zeichen.
host_pattern.too_long = Das Host-Muster „{ $pattern }“ überschreitet die Grenze von 255 Zeichen.

# Netzwerkrichtlinie.
network_policy.scheme.empty = Das Schema darf nicht leer sein.
network_policy.scheme.invalid = Das Schema „{ $scheme }“ enthält ungültige Zeichen.
network_policy.allowlist.empty = Die Host-Positivliste darf nicht leer sein.
network_policy.scheme.not_allowed = Das Schema „{ $scheme }“ ist nicht zugelassen.
network_policy.missing_host = Der URL fehlt ein Host.
network_policy.host.blocked = Der Host „{ $host }“ ist durch die Richtlinie blockiert.
network_policy.host.not_allowlisted = Der Host „{ $host }“ steht nicht auf der Positivliste.

# Konfiguration der Standardbibliothek.
stdlib.config.default_fetch_cache_invalid = Der voreingestellte Pfad des fetch-Caches muss relativ sein.
stdlib.config.default_which_cache_invalid = Die voreingestellte Kapazität des which-Caches muss positiv sein.
stdlib.config.workspace_root_absolute = Der Wurzelpfad des Arbeitsbereichs muss absolut sein.
stdlib.config.fetch_response_limit_positive = Das Antwortlimit von fetch muss positiv sein.
stdlib.config.command_output_limit_positive = Das Limit für erfasste Befehlsausgaben muss positiv sein.
stdlib.config.command_stream_limit_positive = Das Stream-Limit für Befehle muss positiv sein.
stdlib.config.which_cache_capacity_positive = Die Kapazität des which-Caches muss positiv sein.
stdlib.config.skip_dir_empty = Einträge zu übersprungenen Verzeichnissen dürfen nicht leer sein.
stdlib.config.skip_dir_navigation = Einträge zu übersprungenen Verzeichnissen dürfen kein „..“ enthalten.
stdlib.config.skip_dir_separator = Einträge zu übersprungenen Verzeichnissen dürfen keine Pfadtrenner enthalten.
stdlib.config.fetch_cache_empty = Der Pfad des fetch-Caches darf nicht leer sein.
stdlib.config.fetch_cache_not_relative = Der Pfad des fetch-Caches muss relativ sein, erhalten wurde { $path }.
stdlib.config.fetch_cache_escapes = Der Pfad des fetch-Caches darf den Arbeitsbereich nicht verlassen: { $path }.
stdlib.config.open_workspace_root = Das aktuelle Verzeichnis konnte nicht als Wurzel des stdlib-Arbeitsbereichs geöffnet werden.
stdlib.config.resolve_cwd = Das aktuelle Verzeichnis konnte nicht als Wurzel des stdlib-Arbeitsbereichs ermittelt werden.
stdlib.config.cwd_non_utf8 = Das aktuelle Verzeichnis enthält Komponenten ohne gültiges UTF-8: { $path }.

# Diagnosen des fetch-Helfers.
stdlib.fetch.url_invalid = Ungültige URL „{ $url }“: { $details }.
stdlib.fetch.disallowed = Die URL „{ $url }“ ist nicht zugelassen: { $details }.
stdlib.fetch.failed = „{ $url }“ konnte nicht abgerufen werden: { $details }.
stdlib.fetch.cache_read_failed = Der Cache-Eintrag „{ $name }“ konnte nicht gelesen werden: { $details }.
stdlib.fetch.cache_open_failed = Der Cache-Eintrag „{ $name }“ konnte nicht geöffnet werden: { $details }.
stdlib.fetch.response_read_failed = Die Antwort von „{ $url }“ konnte nicht gelesen werden: { $details }.
stdlib.fetch.response_buffer_overflow = Pufferüberlauf beim Lesen von „{ $url }“.
stdlib.fetch.cache_write_failed = Der Cache für „{ $url }“ konnte nicht geschrieben werden: { $details }.
stdlib.fetch.response_limit_exceeded = Die Antwort von „{ $url }“ überschritt das Limit von { $limit } Byte.
stdlib.fetch.cache_limit_exceeded = Die zwischengespeicherte Antwort „{ $name }“ überschritt das Limit von { $limit } Byte.
stdlib.fetch.io_failed = { $action } für { $path } fehlgeschlagen: { $details }.
stdlib.fetch.action.sync_cache = fetch-Cache synchronisieren
stdlib.fetch.action.create_cache_dir = fetch-Cache-Verzeichnis erstellen
stdlib.fetch.action.open_cache_dir = fetch-Cache-Verzeichnis öffnen
stdlib.fetch.action.stat_cache = fetch-Cache-Eintrag abfragen
stdlib.fetch.action.open_cache_entry = fetch-Cache-Eintrag öffnen

# Diagnosen des Befehlshelfers.
stdlib.command.location = Befehl „{ $command }“ in der Vorlage „{ $template }“
stdlib.command.spawn_failed = { $location } konnte nicht gestartet werden: { $details }.
stdlib.command.io_failed = { $location } fehlgeschlagen: { $details }.
stdlib.command.closed_input_early = Die Eingabe wurde geschlossen, bevor das Schreiben an den Befehl abgeschlossen war.
stdlib.command.broken_pipe = Unterbrochene Pipe beim Ausführen von { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } wurde durch ein Signal beendet.
stdlib.command.exited_with_status = { $location } wurde mit Status { $status } beendet.
stdlib.command.output_limit_exceeded = { $location } überschritt das { $mode }-Limit von { $limit } Byte für { $stream }.
stdlib.command.timeout = { $location } überschritt die Zeitgrenze von { $seconds } Sekunden.
stdlib.command.exit_status_suffix = (Exit-Status { $status })
stdlib.command.signal_suffix = (durch Signal beendet)
stdlib.command.shell.empty = Der Shell-Befehl darf nicht leer sein.
stdlib.command.grep.empty_pattern = Das grep-Muster darf nicht leer sein.
stdlib.command.grep.flags_not_string = grep-Flags müssen Zeichenketten sein.
stdlib.command.quote.invalid = { $arg } konnte nicht in Anführungszeichen gesetzt werden: { $details }.
stdlib.command.quote.line_break = Argumente mit Wagenrücklauf oder Zeilenumbruch lassen sich nicht sicher in Anführungszeichen setzen.
stdlib.command.input_undefined = Der Eingabewert ist nicht definiert.
stdlib.command.tempfile.root_required = Zum Anlegen temporärer Befehlsdateien wird die Wurzel des Arbeitsbereichs benötigt.
stdlib.command.tempfile.create_failed = Die temporäre Befehlsdatei konnte nicht erstellt werden: { $details }.
stdlib.command.options.invalid_utf8 = Der Schlüssel einer Befehlsoption muss gültiges UTF-8 sein.
stdlib.command.option.mode_not_string = Der Ausgabemodus muss eine Zeichenkette sein.
stdlib.command.options.invalid_type = Befehlsoptionen müssen ein Objekt sein.
stdlib.command.output.mode_unsupported = Nicht unterstützter Ausgabemodus „{ $mode }“.
stdlib.command.output.mode.capture = Erfassung
stdlib.command.output.mode.streaming = Streaming
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnosen des Pfadhelfers.
stdlib.path.io.failed = { $action } für { $path } fehlgeschlagen ({ $label }).
stdlib.path.io.failed_with_detail = { $action } für { $path } fehlgeschlagen: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } für { $path } fehlgeschlagen ({ $label }): { $detail }.
stdlib.path.io.not_found = nicht gefunden
stdlib.path.io.permission_denied = Zugriff verweigert
stdlib.path.io.already_exists = existiert bereits
stdlib.path.io.invalid_input = ungültige Eingabe
stdlib.path.io.invalid_data = ungültige Daten
stdlib.path.io.timed_out = Zeitüberschreitung
stdlib.path.io.interrupted = unterbrochen
stdlib.path.io.would_block = würde blockieren
stdlib.path.io.write_zero = null Bytes geschrieben
stdlib.path.io.unexpected_eof = unerwartetes Dateiende
stdlib.path.io.broken_pipe = unterbrochene Pipe
stdlib.path.io.connection_refused = Verbindung abgelehnt
stdlib.path.io.connection_reset = Verbindung zurückgesetzt
stdlib.path.io.connection_aborted = Verbindung abgebrochen
stdlib.path.io.not_connected = nicht verbunden
stdlib.path.io.addr_in_use = Adresse bereits belegt
stdlib.path.io.addr_not_available = Adresse nicht verfügbar
stdlib.path.io.out_of_memory = kein Speicher mehr
stdlib.path.io.unsupported = nicht unterstützt
stdlib.path.io.file_too_large = Datei zu groß
stdlib.path.io.resource_busy = Ressource belegt
stdlib.path.io.executable_busy = ausführbare Datei belegt
stdlib.path.io.deadlock = Verklemmung
stdlib.path.io.crosses_devices = überschreitet Gerätegrenzen
stdlib.path.io.too_many_links = zu viele Verknüpfungen
stdlib.path.io.invalid_filename = ungültiger Dateiname
stdlib.path.io.arg_list_too_long = Argumentliste zu lang
stdlib.path.io.stale_handle = veralteter Netzwerk-Dateizeiger
stdlib.path.io.storage_full = Speicher voll
stdlib.path.io.not_seekable = nicht positionierbar
stdlib.path.io.network_down = Netzwerk ausgefallen
stdlib.path.io.network_unreachable = Netzwerk nicht erreichbar
stdlib.path.io.host_unreachable = Host nicht erreichbar
stdlib.path.io.other = E/A-Fehler
stdlib.path.action.canonicalize = kanonisieren
stdlib.path.action.open_directory = Verzeichnis öffnen
stdlib.path.action.stat = abfragen
stdlib.path.action.read = lesen
stdlib.path.action.open_file = Datei öffnen
stdlib.path.with_suffix.empty_separator = with_suffix benötigt ein nicht leeres Trennzeichen.
stdlib.path.relative_to.mismatch = { $path } ist nicht relativ zu { $root }.
stdlib.path.expanduser.unsupported = Die benutzerspezifische Erweiterung von ~ wird nicht unterstützt.
stdlib.path.expanduser.no_home = ~ kann nicht erweitert werden: Es sind keine Umgebungsvariablen für das Heimatverzeichnis gesetzt.
stdlib.path.contents.unsupported_encoding = Nicht unterstützte Kodierung „{ $encoding }“.
stdlib.path.hash.unsupported_algorithm = Nicht unterstützter Hash-Algorithmus „{ $algorithm }“.
stdlib.path.hash.unsupported_algorithm_legacy = Nicht unterstützter Hash-Algorithmus „{ $algorithm }“ (aktivieren Sie das Feature „{ $feature }“).

# Diagnosen der Sammlungshelfer.
stdlib.collections.flatten.expected_sequence = flatten erwartete Sequenzelemente, fand aber { $kind }.
stdlib.collections.group_by.empty_attribute = group_by benötigt ein nicht leeres Attribut.
stdlib.collections.group_by.unresolved = group_by konnte „{ $attr }“ an einem Element vom Typ { $kind } nicht auflösen.

# Diagnosen der Zeithelfer.
stdlib.time.offset.invalid = Der now-Offset „{ $offset }“ ist ungültig: erwartet wurde „+HH:MM[:SS]“ oder „Z“.
stdlib.time.timedelta.overflow = Überlauf in timedelta beim Addieren von { $component }.
stdlib.time.label.weeks = Wochen
stdlib.time.label.days = Tage
stdlib.time.label.hours = Stunden
stdlib.time.label.minutes = Minuten
stdlib.time.label.seconds = Sekunden
stdlib.time.label.milliseconds = Millisekunden
stdlib.time.label.microseconds = Mikrosekunden
stdlib.time.label.nanoseconds = Nanosekunden

# Diagnosen des which-Helfers.
stdlib.which.not_found = [netsuke::jinja::which::not_found] Befehl „{ $command }“ nach Prüfung von { $count } PATH-Einträgen nicht gefunden. Vorschau: { $preview }
stdlib.which.not_found.hint.cwd_auto = Leere PATH-Segmente werden ignoriert; verwenden Sie cwd_mode="auto", um das Arbeitsverzeichnis einzubeziehen.
stdlib.which.not_found.hint.cwd_always = Setzen Sie cwd_mode="always", um das aktuelle Verzeichnis einzubeziehen.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] Der Befehl „{ $command }“ unter „{ $path }“ fehlt oder ist nicht ausführbar.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <leer>
stdlib.which.path_entry.non_utf8 = Der PATH-Eintrag Nr. { $index } enthält Zeichen ohne gültiges UTF-8; Netsuke benötigt UTF-8-Pfade.
stdlib.which.command.empty = which benötigt eine nicht leere Zeichenkette.
stdlib.which.cwd_mode.invalid = cwd_mode muss „auto“, „always“ oder „never“ sein, erhalten wurde „{ $mode }“.
stdlib.which.cwd.resolve_failed = Das aktuelle Verzeichnis konnte nicht ermittelt werden: { $details }.
stdlib.which.cwd.non_utf8 = Das aktuelle Verzeichnis enthält Komponenten ohne gültiges UTF-8.
stdlib.which.canonicalize_failed = „{ $path }“ konnte nicht kanonisiert werden: { $details }.
stdlib.which.is_executable = Es konnte nicht geprüft werden, ob „{ $path }“ ausführbar ist: { $details }.
stdlib.which.canonicalize_non_utf8 = Der kanonische Pfad enthält Komponenten ohne gültiges UTF-8.
stdlib.which.workspace_non_utf8 = Der Arbeitsbereichspfad enthält beim Auflösen des Befehls „{ $command }“ Komponenten ohne gültiges UTF-8: { $path }.
stdlib.which.walkdir_error = Fehler beim Durchlaufen des Arbeitsbereichs während der Befehlsauflösung: { $details }.

# Registrierung der Standardbibliothek.
stdlib.register.open_dir = Das aktuelle Verzeichnis konnte für die stdlib-Registrierung nicht geöffnet werden.
stdlib.register.resolve_dir = Das aktuelle Verzeichnis konnte für die stdlib-Registrierung nicht ermittelt werden.
stdlib.register.dir_non_utf8 = Das aktuelle Verzeichnis enthält Komponenten ohne gültiges UTF-8: { $path }.

# Statusmeldungen für die barrierefreie Ausgabe.
status.state.pending = ausstehend
status.state.running = läuft
status.state.done = fertig
status.state.failed = fehlgeschlagen
status.stage.label = Phase { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Aufgabe { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Manifestdatei wird gelesen
status.stage.initial_yaml_parsing = YAML-Dokument wird eingelesen
status.stage.template_expansion = Vorlagendirektiven werden expandiert
status.stage.final_rendering = Manifestwerte werden deserialisiert und gerendert
status.stage.ir_generation_validation = Abhängigkeitsgraph wird erstellt und geprüft
status.stage.ninja_synthesis = Ninja-Build-Plan wird erzeugt
status.stage.ninja_synthesis_execute = Ninja-Plan wird erzeugt und { $tool } ausgeführt
status.stage.graph_rendering = Graph-Artefakt wird gerendert
status.stage.graph_rendering_with_tool = { $tool } wird gerendert
status.complete = { $tool } abgeschlossen.
status.timing.summary_header = Zeitübersicht der Phasen:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Gesamtdauer der Pipeline: { $duration }
status.tool.build = Build
status.tool.clean = Bereinigung
status.tool.graph = Graph
status.tool.graph_html = Graph (HTML)
status.tool.generate = Erzeugung

# Zeichenketten des HTML-Graph-Renderers.
graph.html.title = Netsuke-Build-Graph
graph.html.heading = Netsuke-Build-Graph
graph.html.description = Von Netsuke gerenderter Build-Graph
graph.html.outline.summary = Ziele und Abhängigkeiten (Textgliederung)
graph.html.outline.no_inputs = Keine Eingaben
graph.html.noscript.notice = JavaScript ist deaktiviert. Die Textgliederung oben enthält den vollständigen Graphen; darunter folgt der DOT-Quelltext.

# Semantische Präfixe für die barrierefreie Ausgabe.
semantic.prefix.error = Fehler:
semantic.prefix.warning = Warnung:
semantic.prefix.success = Erfolg:
semantic.prefix.info = Info:
semantic.prefix.timing = Zeit:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Beispiele für Pluralformen für Übersetzerinnen und Übersetzer.
# Deutsch verwendet wie die Quellsprache die CLDR-Kategorien `one` und `other`.
example.files_processed = { $count ->
    [one] { $count } Datei verarbeitet.
   *[other] { $count } Dateien verarbeitet.
}

example.errors_found = { $count ->
    [0] Keine Fehler gefunden.
    [one] { $count } Fehler gefunden.
   *[other] { $count } Fehler gefunden.
}
