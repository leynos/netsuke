# Zasoby lokalizacyjne wiersza poleceń Netsuke.

runner.io.dyndep.retention = Nie udało się zastosować zasad retencji wygenerowanego pliku dyndep (ścieżka: { $path }).
cli.about = Netsuke kompiluje manifesty YAML + Jinja do planów budowania Ninja.
cli.long_about = Netsuke przekształca manifesty YAML + Jinja w powtarzalne grafy Ninja i uruchamia Ninję z bezpiecznymi ustawieniami domyślnymi.
cli.usage = { $usage }

# Tekst pomocy opcji globalnych.
cli.flag.file.help = Ścieżka do pliku manifestu Netsuke, który ma zostać użyty.
cli.flag.directory.help = Uruchom tak, jakby start nastąpił w tym katalogu.
cli.flag.config.help = Ścieżka do pliku konfiguracyjnego, z pominięciem automatycznego wyszukiwania.
cli.flag.jobs.help = Ustaw liczbę równoległych zadań budowania.
cli.flag.verbose.help = Włącz szczegółowe rejestrowanie diagnostyczne i podsumowania czasów po zakończeniu.
cli.flag.locale.help = Znacznik języka tekstów wiersza poleceń (na przykład: en-US, pl).
cli.flag.fetch_allow_scheme.help = Dodatkowe schematy URL dozwolone dla pomocnika fetch.
cli.flag.fetch_allow_host.help = Nazwy hostów dozwolone, gdy włączona jest domyślna odmowa.
cli.flag.fetch_block_host.help = Nazwy hostów zawsze blokowane, nawet jeśli są dozwolone gdzie indziej.
cli.flag.fetch_default_deny.help = Odmawiaj domyślnie wszystkim hostom; zezwalaj tylko na zadeklarowaną listę.
cli.flag.json.help = Wypisuj dane wyjściowe JSON czytelne dla maszyn.
cli.flag.no_input.help = Nigdy nie czytaj danych wprowadzanych interaktywnie.
cli.flag.color.help = Zasada kolorowania wyjścia (auto, always, never).
cli.flag.emoji.help = Zasada użycia emoji (auto, always, never).
cli.flag.progress.help = Zasada wyświetlania postępu (auto, always, never).
cli.flag.accessibility.help = Zasada wyjścia dostępnego (auto, on, off).
cli.flag.default_targets.help = Domyślne cele budowania, gdy nie podano żadnego.

# Opisy podpoleceń.
cli.subcommand.build.about = Zbuduj cele zdefiniowane w manifeście (domyślne).
cli.subcommand.build.long_about = Zbuduj żądane cele; jeśli żadnego nie podano, użyj celów domyślnych z manifestu.
cli.subcommand.clean.about = Usuń artefakty budowania za pomocą Ninji.
cli.subcommand.clean.long_about = Wygeneruj tymczasowy plik Ninja, a następnie uruchom `ninja -t clean`.
cli.subcommand.graph.about = Wypisz graf zależności budowania. Domyślnym formatem jest DOT.
cli.subcommand.graph.long_about = Przekształć wczytany manifest Netsuke w kanoniczny graf budowania i zapisz go jako Graphviz DOT albo — z opcją `--html` — jako samodzielną stronę HTML. Użyj `--output <PLIK>`, aby zapisać do pliku; `-` zapisuje na standardowe wyjście.
cli.subcommand.generate.about = Wygeneruj manifest Ninja bez uruchamiania Ninji.
cli.subcommand.generate.long_about = Zapisz wygenerowany manifest Ninja na standardowe wyjście albo do pliku wybranego opcją `--output`.
cli.subcommand.help.about = Wyświetl pomoc najwyższego poziomu lub pomoc dla nazwanego tematu.
cli.subcommand.help.long_about = Bez tematu odpowiada to `--help`. Użyj `help targets`, aby wyświetlić katalog celów i akcji dla wybranego manifestu.

# Help catalogue headings and markers.
cli.help.actions_heading = Akcje:
cli.help.targets_heading = Cele:
cli.help.targets.about = Wyświetl cele i akcje w wybranym manifeście.
cli.help.default_marker = domyślny
cli.help.conditional_marker = warunkowy

# Tekst pomocy opcji podpolecenia build.
cli.subcommand.build.flag.targets.help = Cele do zbudowania (w razie pominięcia używa celów domyślnych z manifestu).

# Tekst pomocy opcji podpolecenia graph.
cli.subcommand.graph.flag.html.help = Wyrenderuj graf jako samodzielną stronę HTML zamiast formatu DOT.
cli.subcommand.graph.flag.output.help = Zapisz artefakt grafu do PLIKU; użyj `-` dla standardowego wyjścia.

# Tekst pomocy opcji podpolecenia generate.
cli.subcommand.generate.flag.output.help = Zapisz wygenerowany manifest Ninja do PLIKU zamiast na standardowe wyjście.

# Błędy walidacji wiersza poleceń.
cli.validation.jobs.invalid_number = { $value } nie jest prawidłową liczbą.
cli.validation.jobs.out_of_range = Liczba zadań musi mieścić się w przedziale od { $min } do { $max }.
cli.validation.scheme.empty = Schemat nie może być pusty.
cli.validation.scheme.invalid_start = Schemat „{ $scheme }” musi zaczynać się literą ASCII.
cli.validation.scheme.invalid = Nieprawidłowy schemat „{ $scheme }”.
cli.validation.locale.empty = Znacznik języka nie może być pusty.
cli.validation.locale.invalid = Nieprawidłowy znacznik języka „{ $locale }”.
cli.validation.color.invalid = Nieprawidłowa zasada kolorowania „{ $value }”. Prawidłowe opcje: auto, always, never.
cli.validation.emoji.invalid = Nieprawidłowa zasada emoji „{ $value }”. Prawidłowe opcje: auto, always, never.
cli.validation.progress.invalid = Nieprawidłowa zasada postępu „{ $value }”. Prawidłowe opcje: auto, always, never.
cli.validation.accessibility.invalid = Nieprawidłowa zasada dostępności „{ $value }”. Prawidłowe opcje: auto, on, off.
cli.validation.config.expected_object = Wartości wiersza poleceń miały zostać zserializowane do obiektu, otrzymano { $value }.

# Komunikaty błędów z Clap.
clap-error-missing-argument = Brak wymaganego argumentu: { $argument }
clap-error-missing-subcommand = Brak podpolecenia. Dostępne opcje: { $valid_subcommands }
clap-error-unknown-argument = Nieznany argument: { $argument }
clap-error-invalid-value = Nieprawidłowa wartość argumentu { $argument }: { $value }
clap-error-invalid-subcommand = Nieznane podpolecenie: { $subcommand }
# Uwaga: value-validation sformułowano inaczej niż invalid-value, aby odróżnić
# błędy własnych walidatorów (ErrorKind::ValueValidation) od niezgodności typów
# (ErrorKind::InvalidValue).
clap-error-value-validation = Walidacja nie powiodła się dla { $argument }: { $value }

# Błędy i kontekst wykonania.
runner.manifest.not_found = Nie znaleziono manifestu „{ $manifest_name }” w katalogu { $directory }.
runner.manifest.not_found.help = Upewnij się, że manifest istnieje, albo podaj `--file` z właściwą ścieżką.
runner.manifest.path_missing_name = Ścieżka manifestu „{ $path }” nie zawiera nazwy pliku.
runner.manifest.path_utf8 = Ścieżka manifestu „{ $path }” nie jest prawidłowym UTF-8.
runner.manifest.directory_utf8 = Ścieżka katalogu manifestu „{ $path }” nie jest prawidłowym UTF-8.
runner.manifest.directory_label = katalog `{ $directory }`
runner.manifest.current_directory_label = bieżący katalog
runner.manifest.default_not_declared = Domyślna wartość manifestu '{ $default }' nie wskazuje zadeklarowanej akcji ani celu.
runner.context.network_policy = Nie udało się zbudować zasad sieciowych.
runner.context.load_manifest = Nie udało się wczytać manifestu z { $path }.
runner.context.serialise_manifest = Nie udało się zserializować manifestu.
runner.context.build_graph = Nie udało się zbudować grafu na podstawie manifestu.
runner.context.generate_ninja = Nie udało się wygenerować manifestu Ninja.
runner.context.render_graph = Nie udało się wyrenderować artefaktu grafu.

runner.io.create_temp_file = Nie udało się utworzyć tymczasowego pliku Ninja.
runner.io.write_temp_ninja = Nie udało się zapisać tymczasowego pliku Ninja.
runner.io.flush_temp_ninja = Nie udało się opróżnić bufora tymczasowego pliku Ninja.
runner.io.sync_temp_ninja = Nie udało się zsynchronizować tymczasowego pliku Ninja.
runner.io.create_parent_dir = Nie udało się utworzyć katalogu nadrzędnego { $path }.
runner.io.create_ninja_file = Nie udało się utworzyć pliku Ninja w { $path }.
runner.io.write_ninja_file = Nie udało się zapisać pliku Ninja w { $path }.
runner.io.flush_ninja_file = Nie udało się opróżnić bufora pliku Ninja w { $path }.
runner.io.sync_ninja_file = Nie udało się zsynchronizować pliku Ninja w { $path }.
runner.io.open_ambient_dir = Nie udało się otworzyć katalogu otoczenia.
runner.io.non_utf8_working_directory = Ścieżka katalogu roboczego nie jest prawidłowym tekstem UTF-8.
runner.io.no_existing_ancestor = Dla { $path } nie istnieje żaden katalog nadrzędny.
runner.io.derive_relative_path = Nie udało się wyznaczyć względnej ścieżki Ninja.
runner.io.non_utf8_path = Ścieżki inne niż UTF-8 nie są obsługiwane (ścieżka: { $path }).
runner.io.write_stdout = Nie udało się zapisać manifestu Ninja na standardowe wyjście.
runner.io.flush_stdout = Nie udało się opróżnić bufora standardowego wyjścia.
runner.io.dyndep.create_dir = Nie udało się utworzyć katalogu dyndep { $path }.
runner.io.dyndep.read = Nie udało się odczytać wygenerowanego pliku dyndep (ścieżka: { $path }).
runner.io.dyndep.write = Nie udało się zapisać wygenerowanego pliku dyndep (ścieżka: { $path }).
runner.io.dyndep.rename = Nie udało się zmienić nazwy wygenerowanego pliku dyndep (ścieżka: { $path }).
runner.io.dyndep.corrupt = Wygenerowany plik dyndep (ścieżka: { $path }) nie pasuje do oczekiwanej zawartości; usuń tylko ten plik i spróbuj ponownie.
runner.io.dyndep.temp_collisions = Po wielokrotnych kolizjach nazw nie udało się utworzyć unikatowego tymczasowego pliku dyndep (ścieżka: { $path }).
runner.io.dyndep.too_large = Wygenerowany plik dyndep (ścieżka: { $path }) przekracza limit weryfikacji wynoszący { $limit } bajtów.

# Diagnostyka manifestu.
manifest.parse = Analiza manifestu nie powiodła się.
manifest.structure_error = Błąd struktury manifestu w { $name }: { $details }
manifest.yaml.parse = Błąd analizy YAML w wierszu { $line }, kolumnie { $column }: { $details }
manifest.yaml.label = nieprawidłowy YAML
manifest.yaml.hint.tabs = YAML nie dopuszcza tabulatorów; do wcięć używaj spacji.
manifest.yaml.hint.list_item = Elementy listy YAML muszą zaczynać się od „-” i mieć prawidłowe wcięcie.
manifest.yaml.hint.expected_colon = To wygląda na wpis odwzorowania; po kluczu brakuje „:”.
manifest.yaml.hint.mapping_values = Odwzorowania YAML wymagają wartości po „:” (albo zagnieżdżonego bloku).
manifest.yaml.hint.invalid_token = Token YAML jest nieprawidłowy lub nieoczekiwany.
manifest.yaml.hint.escape = Poprzedź ukośniki odwrotne znakiem ucieczki albo usuń nieprawidłowe sekwencje.
manifest.env.missing = Wymagana zmienna środowiskowa nie jest ustawiona.
manifest.env.invalid_utf8 = Zmienna środowiskowa zawiera nieprawidłowy UTF-8.
manifest.vars.not_object = Pole `vars` manifestu musi być odwzorowaniem lub obiektem.
manifest.vars.reserved_name = Klucz `vars` '{ $name }' w manifeście jest zarezerwowany dla wbudowanej funkcji pomocniczej szablonów; zmień nazwę zmiennej.
manifest.read_failed = Nie udało się odczytać manifestu z { $path }.
manifest.resolve_workspace_root = Nie udało się ustalić katalogu głównego obszaru roboczego.
manifest.workspace_non_utf8 = Ścieżka główna obszaru roboczego „{ $path }” nie jest prawidłowym UTF-8.
manifest.path_non_utf8 = Ścieżka manifestu „{ $manifest }” nie jest prawidłowym UTF-8: { $path }.
manifest.path_missing_name = Ścieżka manifestu „{ $path }” nie zawiera nazwy pliku.
manifest.open_workspace_failed = Nie udało się otworzyć obszaru roboczego { $workspace } dla manifestu { $manifest }.
manifest.foreach.not_iterable = Wyrażenie `foreach` nie jest iterowalne.
manifest.foreach.serialise_item = Nie udało się zserializować elementu `foreach`.
manifest.when.empty = Wyrażenie `when` nie może być puste.
manifest.when.eval_error = Nie udało się obliczyć wyrażenia `when` „{ $expr }”.
manifest.when.template_error = Nie udało się wyrenderować szablonu `when` „{ $expr }”.
manifest.target.vars_not_object = Pole `vars` celu musi być obiektem, otrzymano { $value }.
manifest.vars.entry_not_object = Wpis `vars` manifestu musi być obiektem.
manifest.field_not_string = Pole „{ $field }” musi być łańcuchem znaków.
manifest.expression.parse_error = Nie udało się przeanalizować wyrażenia { $name }.
manifest.expression.eval_error = Nie udało się obliczyć wyrażenia { $name }.

# Diagnostyka makr manifestu.
manifest.macro.signature_missing_identifier = W sygnaturze makra brakuje identyfikatora.
manifest.macro.signature_missing_params = W sygnaturze makra brakuje parametrów.
manifest.macro.compile_failed = Nie udało się skompilować makra { $name }.
manifest.macro.sequence_invalid = Makra muszą być zdefiniowane jako odwzorowanie nazw na szablony.
manifest.macro.register_failed = Nie udało się zarejestrować makr manifestu.
manifest.macro.not_initialised = Środowisko makr nie zostało zainicjowane.
manifest.macro.caller_invalid = Wywołujący makro musi być łańcuchem znaków.
manifest.macro.template_load_failed = Nie udało się wczytać szablonu makra.
manifest.macro.init_failed = Nie udało się zainicjować środowiska makr.
manifest.macro.missing = Brakuje makra { $name }.

# Błędy wzorców glob w manifeście.
manifest.glob.unmatched_brace = Nieprawidłowy wzorzec glob „{ $pattern }”: „{ $character }” bez pary na pozycji { $position }.
manifest.glob.invalid_pattern = Nieprawidłowy wzorzec glob „{ $pattern }”: { $detail }.
manifest.glob.unknown_pattern_error = nieznany błąd wzorca.
manifest.glob.io_failed = Wzorzec glob „{ $pattern }” zawiódł: { $detail }.
manifest.glob.unknown_io_error = nieznany błąd wejścia/wyjścia.
manifest.command_list_empty = Pole „command” nie może być puste: podaj łańcuch polecenia lub niepustą listę.

# Błędy reprezentacji pośredniej.
ir.rule_not_found = Nie znaleziono reguły „{ $rule }”, do której odwołuje się cel „{ $target }”.
ir.multiple_rules = Cel „{ $target }” musi odwoływać się do dokładnie jednej reguły, otrzymano { $rules }.
ir.empty_rule = Cel „{ $target }” musi odwoływać się do reguły.
ir.duplicate_outputs = Wykryto zduplikowane wyjścia: { $outputs }.
ir.circular_dependency = Wykryto zależność cykliczną: { $cycle }.
ir.action_serialisation = Nie udało się zserializować akcji: { $details }.
ir.invalid_command = Nieprawidłowa interpolacja w poleceniu: { $snippet }.

# Błędy generowania plików Ninja.
ninja_gen.missing_action = Brakuje akcji „{ $id }”, do której odwołuje się krawędź budowania.
ninja_gen.format = Nie udało się sformatować wyjścia manifestu Ninja.
ninja_gen.dyndep_files_required = Ta operacja wymaga wygenerowanego pakietu Ninja; użyj `netsuke build`, `netsuke clean` lub `netsuke generate`, aby utworzyć pliki dyndep.
ninja_gen.reserved_output_path = Ścieżka '{ $path }' jest zarezerwowana dla stanu szeregowych zależności Netsuke.
ninja_gen.unsupported_path_character = Ścieżka '{ $path }' zawiera nieobsługiwany znak ścieżki Ninja: '{ $character }'.

# Walidacja wzorców hostów.
host_pattern.empty = Wzorzec hosta nie może być pusty.
host_pattern.contains_scheme = Wzorzec hosta „{ $pattern }” nie może zawierać schematu URL.
host_pattern.contains_slash = Wzorzec hosta „{ $pattern }” nie może zawierać znaku „/”.
host_pattern.missing_suffix = Wzorzec hosta „{ $pattern }” musi zawierać przyrostek po „*.”.
host_pattern.empty_label = Wzorzec hosta „{ $pattern }” zawiera pustą etykietę.
host_pattern.invalid_chars = Wzorzec hosta „{ $pattern }” zawiera nieprawidłowe znaki.
host_pattern.invalid_label_edge = Etykiety wzorca hosta „{ $pattern }” nie mogą zaczynać się ani kończyć znakiem „-”.
host_pattern.label_too_long = Wzorzec hosta „{ $pattern }” zawiera etykietę dłuższą niż 63 znaki.
host_pattern.too_long = Wzorzec hosta „{ $pattern }” przekracza limit 255 znaków.

# Zasady sieciowe.
network_policy.scheme.empty = Schemat nie może być pusty.
network_policy.scheme.invalid = Schemat „{ $scheme }” zawiera nieprawidłowe znaki.
network_policy.allowlist.empty = Lista dozwolonych hostów nie może być pusta.
network_policy.scheme.not_allowed = Schemat „{ $scheme }” nie jest dozwolony.
network_policy.missing_host = W adresie URL brakuje hosta.
network_policy.host.blocked = Host „{ $host }” jest zablokowany przez zasady.
network_policy.host.not_allowlisted = Hosta „{ $host }” nie ma na liście dozwolonych.

# Konfiguracja biblioteki standardowej.
stdlib.config.default_fetch_cache_invalid = Domyślna ścieżka pamięci podręcznej fetch musi być względna.
stdlib.config.default_which_cache_invalid = Domyślna pojemność pamięci podręcznej which musi być dodatnia.
stdlib.config.workspace_root_absolute = Ścieżka główna obszaru roboczego musi być bezwzględna.
stdlib.config.fetch_response_limit_positive = Limit odpowiedzi fetch musi być dodatni.
stdlib.config.command_output_limit_positive = Limit przechwytywanego wyjścia poleceń musi być dodatni.
stdlib.config.command_stream_limit_positive = Limit strumienia poleceń musi być dodatni.
stdlib.config.which_cache_capacity_positive = Pojemność pamięci podręcznej which musi być dodatnia.
stdlib.config.skip_dir_empty = Wpisy pomijanych katalogów nie mogą być puste.
stdlib.config.skip_dir_navigation = Wpisy pomijanych katalogów nie mogą zawierać „..”.
stdlib.config.skip_dir_separator = Wpisy pomijanych katalogów nie mogą zawierać separatorów ścieżki.
stdlib.config.fetch_cache_empty = Ścieżka pamięci podręcznej fetch nie może być pusta.
stdlib.config.fetch_cache_not_relative = Ścieżka pamięci podręcznej fetch musi być względna, otrzymano { $path }.
stdlib.config.fetch_cache_escapes = Ścieżka pamięci podręcznej fetch nie może wychodzić poza obszar roboczy: { $path }.
stdlib.config.open_workspace_root = Nie udało się otworzyć bieżącego katalogu jako katalogu głównego obszaru roboczego stdlib.
stdlib.config.resolve_cwd = Nie udało się ustalić bieżącego katalogu jako katalogu głównego obszaru roboczego stdlib.
stdlib.config.cwd_non_utf8 = Bieżący katalog zawiera elementy inne niż UTF-8: { $path }.

# Diagnostyka pomocnika fetch.
stdlib.fetch.url_invalid = Nieprawidłowy adres URL „{ $url }”: { $details }.
stdlib.fetch.disallowed = Adres URL „{ $url }” jest niedozwolony: { $details }.
stdlib.fetch.failed = Nie udało się pobrać „{ $url }”: { $details }.
stdlib.fetch.cache_read_failed = Nie udało się odczytać wpisu pamięci podręcznej „{ $name }”: { $details }.
stdlib.fetch.cache_open_failed = Nie udało się otworzyć wpisu pamięci podręcznej „{ $name }”: { $details }.
stdlib.fetch.response_read_failed = Nie udało się odczytać odpowiedzi z „{ $url }”: { $details }.
stdlib.fetch.response_buffer_overflow = Przepełnienie bufora podczas odczytu „{ $url }”.
stdlib.fetch.cache_write_failed = Nie udało się zapisać pamięci podręcznej dla „{ $url }”: { $details }.
stdlib.fetch.response_limit_exceeded = Odpowiedź z „{ $url }” przekroczyła limit { $limit } bajtów.
stdlib.fetch.cache_limit_exceeded = Zapisana w pamięci podręcznej odpowiedź „{ $name }” przekroczyła limit { $limit } bajtów.
stdlib.fetch.io_failed = Operacja „{ $action }” nie powiodła się dla { $path }: { $details }.
stdlib.fetch.action.sync_cache = synchronizacja pamięci podręcznej fetch
stdlib.fetch.action.create_cache_dir = utworzenie katalogu pamięci podręcznej fetch
stdlib.fetch.action.open_cache_dir = otwarcie katalogu pamięci podręcznej fetch
stdlib.fetch.action.stat_cache = odczyt informacji o wpisie pamięci podręcznej fetch
stdlib.fetch.action.open_cache_entry = otwarcie wpisu pamięci podręcznej fetch

# Diagnostyka pomocnika poleceń.
stdlib.command.location = polecenie „{ $command }” w szablonie „{ $template }”
stdlib.command.spawn_failed = Nie udało się uruchomić { $location }: { $details }.
stdlib.command.io_failed = { $location } nie powiodło się: { $details }.
stdlib.command.closed_input_early = Wejście zamknięto przed zakończeniem zapisu do polecenia.
stdlib.command.broken_pipe = Przerwany potok podczas wykonywania { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } zostało przerwane sygnałem.
stdlib.command.exited_with_status = { $location } zakończyło się ze statusem { $status }.
stdlib.command.output_limit_exceeded = { $location } przekroczyło limit { $mode } wynoszący { $limit } bajtów dla { $stream }.
stdlib.command.timeout = { $location } przekroczyło limit czasu wynoszący { $seconds } s.
stdlib.command.exit_status_suffix = (status zakończenia { $status })
stdlib.command.signal_suffix = (przerwane sygnałem)
stdlib.command.shell.empty = Polecenie powłoki nie może być puste.
stdlib.command.grep.empty_pattern = Wzorzec grep nie może być pusty.
stdlib.command.grep.flags_not_string = Flagi grep muszą być łańcuchami znaków.
stdlib.command.quote.invalid = Nie udało się ująć { $arg } w cudzysłów: { $details }.
stdlib.command.quote.line_break = Argumentów zawierających powrót karetki lub znak nowego wiersza nie da się bezpiecznie ująć w cudzysłów.
stdlib.command.input_undefined = Wartość wejściowa jest niezdefiniowana.
stdlib.command.tempfile.root_required = Do tworzenia plików tymczasowych poleceń wymagany jest katalog główny obszaru roboczego.
stdlib.command.tempfile.create_failed = Nie udało się utworzyć pliku tymczasowego polecenia: { $details }.
stdlib.command.options.invalid_utf8 = Klucz opcji polecenia musi być prawidłowym UTF-8.
stdlib.command.option.mode_not_string = Tryb wyjścia musi być łańcuchem znaków.
stdlib.command.options.invalid_type = Opcje polecenia muszą być obiektem.
stdlib.command.output.mode_unsupported = Nieobsługiwany tryb wyjścia „{ $mode }”.
stdlib.command.output.mode.capture = przechwytywanie
stdlib.command.output.mode.streaming = strumieniowanie
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostyka pomocnika ścieżek.
stdlib.path.io.failed = Operacja „{ $action }” nie powiodła się dla { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Operacja „{ $action }” nie powiodła się dla { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Operacja „{ $action }” nie powiodła się dla { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = nie znaleziono
stdlib.path.io.permission_denied = odmowa dostępu
stdlib.path.io.already_exists = już istnieje
stdlib.path.io.invalid_input = nieprawidłowe dane wejściowe
stdlib.path.io.invalid_data = nieprawidłowe dane
stdlib.path.io.timed_out = przekroczono limit czasu
stdlib.path.io.interrupted = przerwano
stdlib.path.io.would_block = spowodowałoby zablokowanie
stdlib.path.io.write_zero = zapisano zero bajtów
stdlib.path.io.unexpected_eof = nieoczekiwany koniec pliku
stdlib.path.io.broken_pipe = przerwany potok
stdlib.path.io.connection_refused = odmowa połączenia
stdlib.path.io.connection_reset = połączenie zresetowane
stdlib.path.io.connection_aborted = połączenie przerwane
stdlib.path.io.not_connected = brak połączenia
stdlib.path.io.addr_in_use = adres jest już używany
stdlib.path.io.addr_not_available = adres niedostępny
stdlib.path.io.out_of_memory = brak pamięci
stdlib.path.io.unsupported = nieobsługiwane
stdlib.path.io.file_too_large = plik zbyt duży
stdlib.path.io.resource_busy = zasób zajęty
stdlib.path.io.executable_busy = plik wykonywalny zajęty
stdlib.path.io.deadlock = zakleszczenie
stdlib.path.io.crosses_devices = przekracza granicę urządzeń
stdlib.path.io.too_many_links = zbyt wiele dowiązań
stdlib.path.io.invalid_filename = nieprawidłowa nazwa pliku
stdlib.path.io.arg_list_too_long = lista argumentów zbyt długa
stdlib.path.io.stale_handle = nieaktualny uchwyt pliku sieciowego
stdlib.path.io.storage_full = brak miejsca w pamięci masowej
stdlib.path.io.not_seekable = brak możliwości zmiany pozycji
stdlib.path.io.network_down = sieć niedziałająca
stdlib.path.io.network_unreachable = sieć nieosiągalna
stdlib.path.io.host_unreachable = host nieosiągalny
stdlib.path.io.other = błąd wejścia/wyjścia
stdlib.path.action.canonicalize = kanonizacja
stdlib.path.action.open_directory = otwarcie katalogu
stdlib.path.action.stat = odczyt informacji
stdlib.path.action.read = odczyt
stdlib.path.action.open_file = otwarcie pliku
stdlib.path.with_suffix.empty_separator = with_suffix wymaga niepustego separatora.
stdlib.path.relative_to.mismatch = Ścieżka { $path } nie jest względna względem { $root }.
stdlib.path.expanduser.unsupported = Rozwijanie ~ dla konkretnego użytkownika nie jest obsługiwane.
stdlib.path.expanduser.no_home = Nie można rozwinąć ~: nie ustawiono żadnej zmiennej środowiskowej katalogu domowego.
stdlib.path.contents.unsupported_encoding = Nieobsługiwane kodowanie „{ $encoding }”.
stdlib.path.hash.unsupported_algorithm = Nieobsługiwany algorytm skrótu „{ $algorithm }”.
stdlib.path.hash.unsupported_algorithm_legacy = Nieobsługiwany algorytm skrótu „{ $algorithm }” (włącz funkcję „{ $feature }”).

# Diagnostyka pomocników kolekcji.
stdlib.collections.flatten.expected_sequence = flatten oczekiwał elementów sekwencji, ale napotkał { $kind }.
stdlib.collections.group_by.empty_attribute = group_by wymaga niepustego atrybutu.
stdlib.collections.group_by.unresolved = group_by nie zdołał odnaleźć „{ $attr }” w elemencie typu { $kind }.

# Diagnostyka pomocników czasu.
stdlib.time.offset.invalid = Przesunięcie now „{ $offset }” jest nieprawidłowe: oczekiwano „+HH:MM[:SS]” albo „Z”.
stdlib.time.timedelta.overflow = Przepełnienie timedelta przy dodawaniu składnika { $component }.
stdlib.time.label.weeks = tygodnie
stdlib.time.label.days = dni
stdlib.time.label.hours = godziny
stdlib.time.label.minutes = minuty
stdlib.time.label.seconds = sekundy
stdlib.time.label.milliseconds = milisekundy
stdlib.time.label.microseconds = mikrosekundy
stdlib.time.label.nanoseconds = nanosekundy

# Diagnostyka pomocnika which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] nie znaleziono polecenia „{ $command }” po sprawdzeniu { $count } wpisów zmiennej PATH. Podgląd: { $preview }
stdlib.which.not_found.hint.cwd_auto = Puste segmenty zmiennej PATH są pomijane; użyj cwd_mode="auto", aby uwzględnić katalog roboczy.
stdlib.which.not_found.hint.cwd_always = Ustaw cwd_mode="always", aby uwzględnić bieżący katalog.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] polecenia „{ $command }” w „{ $path }” brakuje albo nie jest wykonywalne.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <puste>
stdlib.which.path_entry.non_utf8 = Wpis nr { $index } zmiennej PATH zawiera znaki inne niż UTF-8; Netsuke wymaga ścieżek w UTF-8.
stdlib.which.command.empty = which wymaga niepustego łańcucha znaków.
stdlib.which.cwd_mode.invalid = cwd_mode musi mieć wartość „auto”, „always” albo „never”, otrzymano „{ $mode }”.
stdlib.which.cwd.resolve_failed = Nie udało się ustalić bieżącego katalogu: { $details }.
stdlib.which.cwd.non_utf8 = Bieżący katalog zawiera elementy inne niż UTF-8.
stdlib.which.canonicalize_failed = Nie udało się skanonizować „{ $path }”: { $details }.
stdlib.which.is_executable = Nie udało się sprawdzić, czy „{ $path }” jest wykonywalne: { $details }.
stdlib.which.canonicalize_non_utf8 = Ścieżka kanoniczna zawiera elementy inne niż UTF-8.
stdlib.which.workspace_non_utf8 = Ścieżka obszaru roboczego zawiera elementy inne niż UTF-8 podczas rozwiązywania polecenia „{ $command }”: { $path }.
stdlib.which.walkdir_error = Błąd przechodzenia obszaru roboczego podczas rozwiązywania polecenia: { $details }.

# Rejestracja biblioteki standardowej.
stdlib.register.open_dir = Nie udało się otworzyć bieżącego katalogu na potrzeby rejestracji stdlib.
stdlib.register.resolve_dir = Nie udało się ustalić bieżącego katalogu na potrzeby rejestracji stdlib.
stdlib.register.dir_non_utf8 = Bieżący katalog zawiera elementy inne niż UTF-8: { $path }.

# Raportowanie stanu w dostępnym trybie wyjścia.
status.state.pending = oczekuje
status.state.running = w toku
status.state.done = gotowe
status.state.failed = niepowodzenie
status.stage.label = Etap { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Zadanie { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Odczyt pliku manifestu
status.stage.initial_yaml_parsing = Analiza dokumentu YAML
status.stage.template_expansion = Rozwijanie dyrektyw szablonu
status.stage.final_rendering = Deserializacja i renderowanie wartości manifestu
status.stage.ir_generation_validation = Budowanie i sprawdzanie grafu zależności
status.stage.ninja_synthesis = Tworzenie planu budowania Ninja
status.stage.ninja_synthesis_execute = Tworzenie planu Ninja i uruchamianie { $tool }
status.stage.graph_rendering = Renderowanie artefaktu grafu
status.stage.graph_rendering_with_tool = Renderowanie { $tool }
status.complete = { $tool } zakończono.
status.timing.summary_header = Podsumowanie czasów etapów:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Łączny czas potoku: { $duration }
status.tool.build = Budowanie
status.tool.clean = Czyszczenie
status.tool.graph = Graf
status.tool.graph_html = Graf (HTML)
status.tool.generate = Generowanie
status.tool.help_targets = Pomoc dotycząca celów

# Teksty renderera HTML grafu.
graph.html.title = Graf budowania Netsuke
graph.html.heading = Graf budowania Netsuke
graph.html.description = Graf budowania wyrenderowany przez Netsuke
graph.html.outline.summary = Cele i zależności (zarys tekstowy)
graph.html.outline.no_inputs = Brak wejść
graph.html.noscript.notice = JavaScript jest wyłączony. Powyższy zarys tekstowy zawiera cały graf; poniżej znajduje się źródło DOT.

# Przedrostki semantyczne dostępnego wyjścia.
semantic.prefix.error = Błąd:
semantic.prefix.warning = Ostrzeżenie:
semantic.prefix.success = Powodzenie:
semantic.prefix.info = Informacja:
semantic.prefix.timing = Czas:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Przykłady form liczby mnogiej dla tłumaczy.
# Polski korzysta z czterech kategorii CLDR: `one`, `few`, `many` i `other`.
# `few` obejmuje liczby kończące się na 2–4 (22–24, 32–34 itd.), ale nie
# 12–14; `many` obejmuje 12–14 oraz pozostałe liczby całkowite.
example.files_processed = { $count ->
    [one] Przetworzono { $count } plik.
    [few] Przetworzono { $count } pliki.
    [many] Przetworzono { $count } plików.
   *[other] Przetworzono { $count } pliku.
}

example.errors_found = { $count ->
    [0] Nie znaleziono błędów.
    [one] Znaleziono { $count } błąd.
    [few] Znaleziono { $count } błędy.
    [many] Znaleziono { $count } błędów.
   *[other] Znaleziono { $count } błędu.
}
