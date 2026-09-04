# Lokalizační zdroje příkazové řádky Netsuke.

runner.io.dyndep.retention = Nepodařilo se použít uchování vygenerovaného dyndep (cesta: { $path }).
cli.about = Netsuke překládá manifesty YAML + Jinja na plány sestavení pro Ninju.
cli.long_about = Netsuke převádí manifesty YAML + Jinja na reprodukovatelné grafy Ninja a spouští Ninju s bezpečným výchozím nastavením.
cli.usage = { $usage }

# Text nápovědy globálních přepínačů.
cli.flag.file.help = Cesta k souboru manifestu Netsuke, který se má použít.
cli.flag.directory.help = Spustit, jako by byl program spuštěn v tomto adresáři.
cli.flag.config.help = Cesta ke konfiguračnímu souboru; obchází automatické hledání.
cli.flag.jobs.help = Nastavit počet souběžných úloh sestavení.
cli.flag.verbose.help = Zapnout podrobné diagnostické protokolování a souhrny časů po dokončení.
cli.flag.locale.help = Jazyková značka textů příkazové řádky (například: en-US, cs).
cli.flag.fetch_allow_scheme.help = Další schémata URL povolená pro pomocníka fetch.
cli.flag.fetch_allow_host.help = Názvy hostitelů povolené, když je zapnuto výchozí odmítání.
cli.flag.fetch_block_host.help = Názvy hostitelů, které se vždy blokují, i když jsou povoleny jinde.
cli.flag.fetch_default_deny.help = Ve výchozím stavu odmítat všechny hostitele; povolit jen uvedený seznam.
cli.flag.trust_project_fetch_policy.help = Umožnit konfiguraci projektu rozšířit oprávnění zásady fetch.
cli.flag.json.help = Vypisovat strojově čitelný výstup JSON.
cli.flag.no_input.help = Nikdy nečíst interaktivní vstup.
cli.flag.color.help = Zásada barevného výstupu (auto, always, never).
cli.flag.emoji.help = Zásada používání emodži (auto, always, never).
cli.flag.progress.help = Zásada zobrazování průběhu (auto, always, never).
cli.flag.accessibility.help = Zásada přístupného výstupu (auto, on, off).
cli.flag.default_targets.help = Výchozí cíle sestavení, pokud není žádný uveden.

# Popisy podpříkazů.
cli.subcommand.build.about = Sestavit cíle definované v manifestu (výchozí).
cli.subcommand.build.long_about = Sestavit požadované cíle; není-li žádný uveden, použít výchozí cíle z manifestu.
cli.subcommand.clean.about = Odstranit artefakty sestavení prostřednictvím Ninji.
cli.subcommand.clean.long_about = Vytvořit dočasný soubor Ninja a poté spustit `ninja -t clean`.
cli.subcommand.graph.about = Vypsat graf závislostí sestavení. Výchozí formát je DOT.
cli.subcommand.graph.long_about = Převést načtený manifest Netsuke na kanonický graf sestavení a zapsat jej jako Graphviz DOT, případně s přepínačem `--html` jako samostatnou stránku HTML. Zápis do souboru zajistí `--output <SOUBOR>`; `-` zapisuje na standardní výstup.
cli.subcommand.generate.about = Vytvořit manifest Ninja bez spuštění Ninji.
cli.subcommand.generate.long_about = Zapsat vytvořený manifest Ninja na standardní výstup nebo do souboru zvoleného přepínačem `--output`.
cli.subcommand.help.about = Vytisknout nápovědu na nejvyšší úrovni, nebo nápovědu pro pojmenované téma.
cli.subcommand.help.long_about = Bez tématu odpovídá příkazu `--help`. Pomocí `help targets` vytisknete katalog cílů a akcí pro vybraný soubor.

# Help catalogue headings and markers.
cli.help.actions_heading = Akce:
cli.help.targets_heading = Cíle:
cli.help.targets.about = Vypsat cíle a akce ve vybraném manifestu.
cli.help.default_marker = výchozí
cli.help.conditional_marker = podmíněný

# Text nápovědy přepínačů podpříkazu build.
cli.subcommand.build.flag.targets.help = Cíle k sestavení (při vynechání se použijí výchozí cíle z manifestu).

# Text nápovědy přepínačů podpříkazu graph.
cli.subcommand.graph.flag.html.help = Vykreslit graf jako samostatnou stránku HTML místo formátu DOT.
cli.subcommand.graph.flag.output.help = Zapsat artefakt grafu do SOUBORU; pro standardní výstup použijte `-`.

# Text nápovědy přepínačů podpříkazu generate.
cli.subcommand.generate.flag.output.help = Zapsat vytvořený manifest Ninja do SOUBORU místo na standardní výstup.

# Chyby ověření na příkazové řádce.
cli.validation.jobs.invalid_number = { $value } není platné číslo.
cli.validation.jobs.out_of_range = Počet úloh musí být mezi { $min } a { $max }.
cli.validation.scheme.empty = Schéma nesmí být prázdné.
cli.validation.scheme.invalid_start = Schéma „{ $scheme }“ musí začínat písmenem ASCII.
cli.validation.scheme.invalid = Neplatné schéma „{ $scheme }“.
cli.validation.locale.empty = Jazyková značka nesmí být prázdná.
cli.validation.locale.invalid = Neplatná jazyková značka „{ $locale }“.
cli.validation.color.invalid = Neplatná zásada barev „{ $value }“. Platné možnosti: auto, always, never.
cli.validation.emoji.invalid = Neplatná zásada emodži „{ $value }“. Platné možnosti: auto, always, never.
cli.validation.progress.invalid = Neplatná zásada průběhu „{ $value }“. Platné možnosti: auto, always, never.
cli.validation.accessibility.invalid = Neplatná zásada přístupnosti „{ $value }“. Platné možnosti: auto, on, off.
cli.validation.config.expected_object = Hodnoty z příkazové řádky se měly serializovat do objektu, obdrženo { $value }.

# Chybové zprávy z Clapu.
clap-error-missing-argument = Chybí povinný argument: { $argument }
clap-error-missing-subcommand = Chybí podpříkaz. Dostupné možnosti: { $valid_subcommands }
clap-error-unknown-argument = Neznámý argument: { $argument }
clap-error-invalid-value = Neplatná hodnota argumentu { $argument }: { $value }
clap-error-invalid-subcommand = Neznámý podpříkaz: { $subcommand }
# Poznámka: value-validation je formulováno jinak než invalid-value, aby se
# odlišily chyby vlastních ověřovačů (ErrorKind::ValueValidation) od neshody
# typů (ErrorKind::InvalidValue).
clap-error-value-validation = Ověření selhalo pro { $argument }: { $value }

# Chyby a kontext běhu.
runner.manifest.not_found = Manifest „{ $manifest_name }“ nebyl v adresáři { $directory } nalezen.
runner.manifest.not_found.help = Ověřte, že manifest existuje, nebo zadejte `--file` se správnou cestou.
runner.manifest.path_missing_name = Cesta k manifestu „{ $path }“ neobsahuje název souboru.
cli.file.non_utf8 = Cesta k manifestu „{ $path }“ není platné UTF-8.
runner.manifest.directory_label = adresář `{ $directory }`
runner.manifest.current_directory_label = aktuální adresář
runner.manifest.default_not_declared = Výchozí položka manifestu „{ $default }“ neoznačuje deklarovanou akci ani cíl.
runner.context.network_policy = Síťovou zásadu se nepodařilo sestavit.
runner.context.load_manifest = Manifest v { $path } se nepodařilo načíst.
runner.context.serialise_manifest = Manifest se nepodařilo serializovat.
runner.context.build_graph = Z manifestu se nepodařilo sestavit graf.
runner.context.generate_ninja = Manifest Ninja se nepodařilo vytvořit.
runner.context.render_graph = Artefakt grafu se nepodařilo vykreslit.

runner.io.create_temp_file = Dočasný soubor Ninja se nepodařilo vytvořit.
runner.io.write_temp_ninja = Dočasný soubor Ninja se nepodařilo zapsat.
runner.io.flush_temp_ninja = Vyrovnávací paměť dočasného souboru Ninja se nepodařilo vyprázdnit.
runner.io.sync_temp_ninja = Dočasný soubor Ninja se nepodařilo synchronizovat.
runner.io.create_parent_dir = Nadřazený adresář { $path } se nepodařilo vytvořit.
runner.io.create_ninja_file = Soubor Ninja v { $path } se nepodařilo vytvořit.
runner.io.write_ninja_file = Soubor Ninja v { $path } se nepodařilo zapsat.
runner.io.flush_ninja_file = Vyrovnávací paměť souboru Ninja v { $path } se nepodařilo vyprázdnit.
runner.io.sync_ninja_file = Soubor Ninja v { $path } se nepodařilo synchronizovat.
runner.io.open_ambient_dir = Okolní adresář se nepodařilo otevřít.
cli.directory.non_utf8 = Cesta pracovního adresáře není platná v UTF-8. ({ $path })
runner.io.no_existing_ancestor = Pro { $path } neexistuje žádný nadřazený adresář.
runner.io.derive_relative_path = Relativní cestu pro Ninju se nepodařilo odvodit.
runner.io.non_utf8_path = Cesty, které nejsou v UTF-8, nejsou podporovány (cesta: { $path }).
runner.io.write_stdout = Manifest Ninja se nepodařilo zapsat na standardní výstup.
runner.io.flush_stdout = Vyrovnávací paměť standardního výstupu se nepodařilo vyprázdnit.
runner.io.dyndep.create_dir = Nepodařilo se vytvořit adresář dyndep { $path }.
runner.io.dyndep.read = Nepodařilo se přečíst vygenerovaný soubor dyndep (cesta: { $path }).
runner.io.dyndep.write = Nepodařilo se zapsat vygenerovaný soubor dyndep (cesta: { $path }).
runner.io.dyndep.rename = Nepodařilo se dokončit vygenerovaný soubor dyndep (cesta: { $path }).
runner.io.dyndep.corrupt = Vygenerovaný soubor dyndep (cesta: { $path }) neodpovídá očekávanému obsahu; odstraňte pouze tento soubor a zkuste to znovu.
runner.io.dyndep.temp_collisions = Po opakovaných kolizích názvů se nepodařilo vytvořit jedinečný dočasný soubor dyndep (cesta: { $path }).
runner.io.dyndep.too_large = Vygenerovaný soubor dyndep (cesta: { $path }) překračuje limit ověření { $limit } bajtů.

# Diagnostika manifestu.
manifest.parse = Zpracování manifestu selhalo.
manifest.structure_error = Chyba struktury manifestu u { $name }: { $details }
manifest.yaml.parse = Chyba zpracování YAML na řádku { $line }, sloupci { $column }: { $details }
manifest.yaml.label = neplatný YAML
manifest.yaml.hint.tabs = YAML nepovoluje tabulátory; k odsazení používejte mezery.
manifest.yaml.hint.list_item = Položky seznamu YAML musí začínat znakem „-“ a být správně odsazené.
manifest.yaml.hint.expected_colon = Vypadá to na položku mapování; za klíčem chybí „:“.
manifest.yaml.hint.mapping_values = Mapování YAML vyžadují za „:“ hodnotu (nebo vnořený blok).
manifest.yaml.hint.invalid_token = Token YAML je neplatný nebo neočekávaný.
manifest.yaml.hint.escape = Escapujte zpětná lomítka nebo odstraňte neplatné únikové sekvence.
manifest.env.missing = Povinná proměnná prostředí není nastavena.
manifest.env.invalid_utf8 = Proměnná prostředí obsahuje neplatné UTF-8.
manifest.vars.not_object = Položka `vars` manifestu musí být mapování nebo objekt.
manifest.vars.reserved_name = Klíč `vars` '{ $name }' v manifestu je vyhrazen pro vestavěného pomocníka šablon; přejmenujte proměnnou.
manifest.read_failed = Manifest v { $path } se nepodařilo přečíst.
manifest.resolve_workspace_root = Kořen pracovního prostoru se nepodařilo určit.
manifest.workspace_non_utf8 = Kořenová cesta pracovního prostoru „{ $path }“ není platné UTF-8.
manifest.path_non_utf8 = Cesta manifestu „{ $manifest }“ není platné UTF-8: { $path }.
manifest.path_missing_name = Cesta k manifestu „{ $path }“ neobsahuje název souboru.
manifest.open_workspace_failed = Pracovní prostor { $workspace } se nepodařilo otevřít pro manifest { $manifest }.
manifest.foreach.not_iterable = Výraz `foreach` nelze procházet.
manifest.foreach.serialise_item = Položku výrazu `foreach` se nepodařilo serializovat.
manifest.when.empty = Výraz `when` nesmí být prázdný.
manifest.when.eval_error = Výraz `when` „{ $expr }“ se nepodařilo vyhodnotit.
manifest.when.template_error = Šablonu `when` „{ $expr }“ se nepodařilo vykreslit.
manifest.target.vars_not_object = Položka `vars` cíle musí být objekt, obdrženo { $value }.
manifest.vars.entry_not_object = Položka `vars` manifestu musí být objekt.
manifest.field_not_string = Pole „{ $field }“ musí být řetězec.
manifest.expression.parse_error = Výraz { $name } se nepodařilo zpracovat.
manifest.expression.eval_error = Výraz { $name } se nepodařilo vyhodnotit.

# Diagnostika maker manifestu.
manifest.macro.signature_missing_identifier = V hlavičce makra chybí identifikátor.
manifest.macro.signature_missing_params = V hlavičce makra chybí parametry.
manifest.macro.compile_failed = Makro { $name } se nepodařilo přeložit.
manifest.macro.sequence_invalid = Makra musí být definována jako mapování názvů na šablony.
manifest.macro.register_failed = Makra manifestu se nepodařilo zaregistrovat.
manifest.macro.not_initialised = Prostředí maker není inicializováno.
manifest.macro.caller_invalid = Volající makra musí být řetězec.
manifest.macro.template_load_failed = Šablonu makra se nepodařilo načíst.
manifest.macro.init_failed = Prostředí maker se nepodařilo inicializovat.
manifest.macro.missing = Makro { $name } chybí.

# Chyby vzorů glob v manifestu.
manifest.glob.unmatched_brace = Neplatný vzor glob „{ $pattern }“: „{ $character }“ bez protějšku na pozici { $position }.
manifest.glob.invalid_pattern = Neplatný vzor glob „{ $pattern }“: { $detail }.
manifest.glob.unknown_pattern_error = neznámá chyba vzoru.
manifest.glob.io_failed = Glob selhal pro „{ $pattern }“: { $detail }.
manifest.glob.unknown_io_error = neznámá vstupně-výstupní chyba.
manifest.command_list_empty = Pole „command“ nesmí být prázdné: zadejte řetězec s příkazem nebo neprázdný seznam.

# Chyby mezikódu.
ir.rule_not_found = Pravidlo „{ $rule }“, na které odkazuje cíl „{ $target }“, nebylo nalezeno.
ir.multiple_rules = Cíl „{ $target }“ musí odkazovat právě na jedno pravidlo, obdrženo { $rules }.
ir.empty_rule = Cíl „{ $target }“ musí odkazovat na pravidlo.
ir.duplicate_outputs = Byly zjištěny duplicitní výstupy: { $outputs }.
ir.circular_dependency = Byla zjištěna cyklická závislost: { $cycle }.
ir.action_serialisation = Akci se nepodařilo serializovat: { $details }.
ir.invalid_command = Neplatné vložení v příkazu: { $snippet }.

# Chyby při generování souborů Ninja.
ninja_gen.missing_action = Chybí akce „{ $id }“, na kterou odkazuje hrana sestavení.
ninja_gen.format = Výstup manifestu Ninja se nepodařilo naformátovat.
ninja_gen.dyndep_files_required = Toto sestavení vyžaduje vygenerovaný balíček Ninja; použijte `netsuke build`, `netsuke clean` nebo `netsuke generate`, aby se soubory dyndep materializovaly.
ninja_gen.reserved_output_path = Cesta '{ $path }' je vyhrazena pro stav sériových závislostí Netsuke.
ninja_gen.unsupported_path_character = Cesta '{ $path }' obsahuje nepodporovaný znak cesty Ninja '{ $character }'.

# Ověření vzorů hostitelů.
host_pattern.empty = Vzor hostitele nesmí být prázdný.
host_pattern.contains_scheme = Vzor hostitele „{ $pattern }“ nesmí obsahovat schéma URL.
host_pattern.contains_slash = Vzor hostitele „{ $pattern }“ nesmí obsahovat znak „/“.
host_pattern.missing_suffix = Vzor hostitele „{ $pattern }“ musí obsahovat příponu za „*.“.
host_pattern.empty_label = Vzor hostitele „{ $pattern }“ obsahuje prázdný štítek.
host_pattern.invalid_chars = Vzor hostitele „{ $pattern }“ obsahuje neplatné znaky.
host_pattern.invalid_label_edge = Štítky vzoru hostitele „{ $pattern }“ nesmějí začínat ani končit znakem „-“.
host_pattern.label_too_long = Vzor hostitele „{ $pattern }“ obsahuje štítek delší než 63 znaků.
host_pattern.too_long = Vzor hostitele „{ $pattern }“ překračuje limit 255 znaků.

# Síťová zásada.
network_policy.scheme.empty = Schéma nesmí být prázdné.
network_policy.scheme.invalid = Schéma „{ $scheme }“ obsahuje neplatné znaky.
network_policy.allowlist.empty = Seznam povolených hostitelů nesmí být prázdný.
network_policy.scheme.not_allowed = Schéma „{ $scheme }“ není povoleno.
network_policy.missing_host = V adrese URL chybí hostitel.
network_policy.host.blocked = Hostitel „{ $host }“ je zásadou blokován.
network_policy.host.not_allowlisted = Hostitel „{ $host }“ není na seznamu povolených.

# Konfigurace standardní knihovny.
stdlib.config.default_fetch_cache_invalid = Výchozí cesta mezipaměti fetch musí být relativní.
stdlib.config.default_which_cache_invalid = Výchozí kapacita mezipaměti which musí být kladná.
stdlib.config.workspace_root_absolute = Kořenová cesta pracovního prostoru musí být absolutní.
stdlib.config.fetch_response_limit_positive = Limit odpovědi fetch musí být kladný.
stdlib.config.command_output_limit_positive = Limit zachyceného výstupu příkazů musí být kladný.
stdlib.config.command_stream_limit_positive = Limit proudu příkazů musí být kladný.
stdlib.config.which_cache_capacity_positive = Kapacita mezipaměti which musí být kladná.
stdlib.config.skip_dir_empty = Položky přeskakovaných adresářů nesmějí být prázdné.
stdlib.config.skip_dir_navigation = Položky přeskakovaných adresářů nesmějí obsahovat „..“.
stdlib.config.skip_dir_separator = Položky přeskakovaných adresářů nesmějí obsahovat oddělovače cest.
stdlib.config.fetch_cache_empty = Cesta mezipaměti fetch nesmí být prázdná.
stdlib.config.fetch_cache_not_relative = Cesta mezipaměti fetch musí být relativní, obdrženo { $path }.
stdlib.config.fetch_cache_escapes = Cesta mezipaměti fetch nesmí opustit pracovní prostor: { $path }.
stdlib.config.open_workspace_root = Aktuální adresář se nepodařilo otevřít jako kořen pracovního prostoru stdlib.
stdlib.config.resolve_cwd = Aktuální adresář se nepodařilo určit jako kořen pracovního prostoru stdlib.
stdlib.config.cwd_non_utf8 = Aktuální adresář obsahuje části, které nejsou v UTF-8: { $path }.

# Diagnostika pomocníka fetch.
stdlib.fetch.url_invalid = Neplatná adresa URL „{ $url }“: { $details }.
stdlib.fetch.disallowed = Adresa URL „{ $url }“ není povolena: { $details }.
stdlib.fetch.failed = Z adresy „{ $url }“ se nepodařilo stáhnout data: { $details }.
stdlib.fetch.cache_read_failed = Položku mezipaměti „{ $name }“ se nepodařilo přečíst: { $details }.
stdlib.fetch.cache_open_failed = Položku mezipaměti „{ $name }“ se nepodařilo otevřít: { $details }.
stdlib.fetch.response_read_failed = Odpověď z „{ $url }“ se nepodařilo přečíst: { $details }.
stdlib.fetch.response_buffer_overflow = Přetečení vyrovnávací paměti při čtení „{ $url }“.
stdlib.fetch.cache_write_failed = Mezipaměť pro „{ $url }“ se nepodařilo zapsat: { $details }.
stdlib.fetch.response_limit_exceeded = Odpověď z „{ $url }“ překročila limit { $limit } bajtů.
stdlib.fetch.cache_limit_exceeded = Odpověď „{ $name }“ v mezipaměti překročila limit { $limit } bajtů.
stdlib.fetch.io_failed = Akce „{ $action }“ selhala pro { $path }: { $details }.
stdlib.fetch.action.sync_cache = synchronizace mezipaměti fetch
stdlib.fetch.action.create_cache_dir = vytvoření adresáře mezipaměti fetch
stdlib.fetch.action.open_cache_dir = otevření adresáře mezipaměti fetch
stdlib.fetch.action.stat_cache = zjištění údajů o položce mezipaměti fetch
stdlib.fetch.action.open_cache_entry = otevření položky mezipaměti fetch

# Diagnostika pomocníka pro příkazy.
stdlib.command.location = příkaz „{ $command }“ v šabloně „{ $template }“
stdlib.command.spawn_failed = { $location } se nepodařilo spustit: { $details }.
stdlib.command.io_failed = { $location } selhal: { $details }.
stdlib.command.closed_input_early = Vstup se uzavřel dříve, než byl zápis do příkazu dokončen.
stdlib.command.broken_pipe = Přerušená roura při běhu { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } byl ukončen signálem.
stdlib.command.exited_with_status = { $location } skončil se stavem { $status }.
stdlib.command.output_limit_exceeded = { $location } překročil limit { $mode } ve výši { $limit } bajtů pro { $stream }.
stdlib.command.timeout = { $location } překročil časový limit { $seconds } s.
stdlib.command.exit_status_suffix = (návratový kód { $status })
stdlib.command.signal_suffix = (ukončeno signálem)
stdlib.command.shell.empty = Příkaz shellu nesmí být prázdný.
stdlib.command.grep.empty_pattern = Vzor pro grep nesmí být prázdný.
stdlib.command.grep.flags_not_string = Přepínače grepu musí být řetězce.
stdlib.command.quote.invalid = Argument { $arg } se nepodařilo uzavřít do uvozovek: { $details }.
stdlib.command.quote.line_break = Argumenty obsahující návrat vozíku nebo konec řádku nelze bezpečně uzavřít do uvozovek.
stdlib.command.input_undefined = Vstupní hodnota není definována.
stdlib.command.tempfile.root_required = Pro vytváření dočasných souborů příkazů je nutný kořen pracovního prostoru.
stdlib.command.tempfile.create_failed = Dočasný soubor příkazu se nepodařilo vytvořit: { $details }.
stdlib.command.options.invalid_utf8 = Klíč volby příkazu musí být platné UTF-8.
stdlib.command.option.mode_not_string = Režim výstupu musí být řetězec.
stdlib.command.options.invalid_type = Volby příkazu musí být objekt.
stdlib.command.output.mode_unsupported = Nepodporovaný režim výstupu „{ $mode }“.
stdlib.command.output.mode.capture = zachytávání
stdlib.command.output.mode.streaming = proudové zpracování
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostika pomocníka pro cesty.
stdlib.path.io.failed = Akce „{ $action }“ selhala pro { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Akce „{ $action }“ selhala pro { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Akce „{ $action }“ selhala pro { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = nenalezeno
stdlib.path.io.permission_denied = přístup odepřen
stdlib.path.io.already_exists = již existuje
stdlib.path.io.invalid_input = neplatný vstup
stdlib.path.io.invalid_data = neplatná data
stdlib.path.io.timed_out = vypršel časový limit
stdlib.path.io.interrupted = přerušeno
stdlib.path.io.would_block = došlo by k zablokování
stdlib.path.io.write_zero = zapsáno nula bajtů
stdlib.path.io.unexpected_eof = neočekávaný konec souboru
stdlib.path.io.broken_pipe = přerušená roura
stdlib.path.io.connection_refused = spojení odmítnuto
stdlib.path.io.connection_reset = spojení resetováno
stdlib.path.io.connection_aborted = spojení přerušeno
stdlib.path.io.not_connected = bez spojení
stdlib.path.io.addr_in_use = adresa se již používá
stdlib.path.io.addr_not_available = adresa není dostupná
stdlib.path.io.out_of_memory = došla paměť
stdlib.path.io.unsupported = nepodporováno
stdlib.path.io.file_too_large = soubor je příliš velký
stdlib.path.io.resource_busy = prostředek je zaneprázdněn
stdlib.path.io.executable_busy = spustitelný soubor je zaneprázdněn
stdlib.path.io.deadlock = uváznutí
stdlib.path.io.crosses_devices = překračuje hranici zařízení
stdlib.path.io.too_many_links = příliš mnoho odkazů
stdlib.path.io.invalid_filename = neplatný název souboru
stdlib.path.io.arg_list_too_long = seznam argumentů je příliš dlouhý
stdlib.path.io.stale_handle = zastaralý popisovač síťového souboru
stdlib.path.io.storage_full = úložiště je plné
stdlib.path.io.not_seekable = nelze v něm nastavovat pozici
stdlib.path.io.network_down = síť je mimo provoz
stdlib.path.io.network_unreachable = síť je nedosažitelná
stdlib.path.io.host_unreachable = hostitel je nedosažitelný
stdlib.path.io.other = vstupně-výstupní chyba
stdlib.path.action.canonicalize = kanonizace
stdlib.path.action.open_directory = otevření adresáře
stdlib.path.action.stat = zjištění údajů
stdlib.path.action.read = čtení
stdlib.path.action.open_file = otevření souboru
stdlib.path.with_suffix.empty_separator = with_suffix vyžaduje neprázdný oddělovač.
stdlib.path.relative_to.mismatch = { $path } není relativní vůči { $root }.
stdlib.path.expanduser.unsupported = Rozvoj znaku ~ pro konkrétního uživatele není podporován.
stdlib.path.expanduser.no_home = Znak ~ nelze rozvinout: není nastavena žádná proměnná prostředí domovského adresáře.
stdlib.path.contents.unsupported_encoding = Nepodporované kódování „{ $encoding }“.
stdlib.path.hash.unsupported_algorithm = Nepodporovaný hashovací algoritmus „{ $algorithm }“.
stdlib.path.hash.unsupported_algorithm_legacy = Nepodporovaný hashovací algoritmus „{ $algorithm }“ (zapněte funkci „{ $feature }“).

# Diagnostika pomocníků pro kolekce.
stdlib.collections.flatten.expected_sequence = flatten očekával prvky posloupnosti, ale nalezl { $kind }.
stdlib.collections.group_by.empty_attribute = group_by vyžaduje neprázdný atribut.
stdlib.collections.group_by.unresolved = group_by nedokázal najít „{ $attr }“ u prvku typu { $kind }.

# Diagnostika pomocníků pro čas.
stdlib.time.offset.invalid = Posun now „{ $offset }“ je neplatný: očekáváno „+HH:MM[:SS]“ nebo „Z“.
stdlib.time.timedelta.overflow = Přetečení timedelta při přičítání složky { $component }.
stdlib.time.label.weeks = týdny
stdlib.time.label.days = dny
stdlib.time.label.hours = hodiny
stdlib.time.label.minutes = minuty
stdlib.time.label.seconds = sekundy
stdlib.time.label.milliseconds = milisekundy
stdlib.time.label.microseconds = mikrosekundy
stdlib.time.label.nanoseconds = nanosekundy

# Diagnostika pomocníka which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] příkaz „{ $command }“ nebyl nalezen po prohledání { $count } položek proměnné PATH. Náhled: { $preview }
stdlib.which.not_found.hint.cwd_auto = Prázdné části proměnné PATH se ignorují; pomocí cwd_mode="auto" zahrnete pracovní adresář.
stdlib.which.not_found.hint.cwd_always = Nastavte cwd_mode="always", chcete-li zahrnout aktuální adresář.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] příkaz „{ $command }“ v „{ $path }“ chybí nebo není spustitelný.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <prázdné>
stdlib.which.path_entry.non_utf8 = Položka č. { $index } proměnné PATH obsahuje znaky, které nejsou v UTF-8; Netsuke vyžaduje cesty v UTF-8.
stdlib.which.command.empty = which vyžaduje neprázdný řetězec.
stdlib.which.cwd_mode.invalid = cwd_mode musí být „auto“, „always“ nebo „never“, obdrženo „{ $mode }“.
stdlib.which.cwd.resolve_failed = Aktuální adresář se nepodařilo určit: { $details }.
stdlib.which.cwd.non_utf8 = Aktuální adresář obsahuje části, které nejsou v UTF-8.
stdlib.which.canonicalize_failed = Cestu „{ $path }“ se nepodařilo kanonizovat: { $details }.
stdlib.which.is_executable = Nepodařilo se zjistit, zda je „{ $path }“ spustitelný: { $details }.
stdlib.which.canonicalize_non_utf8 = Kanonická cesta obsahuje části, které nejsou v UTF-8.
stdlib.which.workspace_non_utf8 = Cesta pracovního prostoru obsahuje při hledání příkazu „{ $command }“ části, které nejsou v UTF-8: { $path }.
stdlib.which.walkdir_error = Chyba při procházení pracovního prostoru během hledání příkazu: { $details }.

# Registrace standardní knihovny.
stdlib.register.open_dir = Aktuální adresář se nepodařilo otevřít pro registraci stdlib.
stdlib.register.resolve_dir = Aktuální adresář se nepodařilo určit pro registraci stdlib.
stdlib.register.dir_non_utf8 = Aktuální adresář obsahuje části, které nejsou v UTF-8: { $path }.

# Hlášení stavu v přístupném režimu výstupu.
status.state.pending = čeká
status.state.running = probíhá
status.state.done = hotovo
status.state.failed = selhalo
status.stage.label = Fáze { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Úloha { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Čtení souboru manifestu
status.stage.initial_yaml_parsing = Zpracování dokumentu YAML
status.stage.template_expansion = Rozvíjení direktiv šablon
status.stage.final_rendering = Deserializace a vykreslení hodnot manifestu
status.stage.ir_generation_validation = Sestavení a ověření grafu závislostí
status.stage.ninja_synthesis = Sestavení plánu Ninja
status.stage.ninja_synthesis_execute = Sestavení plánu Ninja a spuštění { $tool }
status.stage.graph_rendering = Vykreslování artefaktu grafu
status.stage.graph_rendering_with_tool = Vykreslování { $tool }
status.complete = { $tool }: dokončeno.
status.timing.summary_header = Souhrn časů podle fází:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Celkový čas zpracování: { $duration }
status.tool.build = Sestavení
status.tool.clean = Vyčištění
status.tool.graph = Graf
status.tool.graph_html = Graf (HTML)
status.tool.generate = Generování
status.tool.help_targets = Katalog cílů

# Texty vykreslování grafu do HTML.
graph.html.title = Graf sestavení Netsuke
graph.html.heading = Graf sestavení Netsuke
graph.html.description = Graf sestavení vykreslený nástrojem Netsuke
graph.html.outline.summary = Cíle a závislosti (textový přehled)
graph.html.outline.no_inputs = Žádné vstupy
graph.html.noscript.notice = JavaScript je vypnutý. Textový přehled výše obsahuje celý graf; níže následuje zdroj DOT.

# Sémantické předpony přístupného výstupu.
semantic.prefix.error = Chyba:
semantic.prefix.warning = Varování:
semantic.prefix.success = Úspěch:
semantic.prefix.info = Informace:
semantic.prefix.timing = Čas:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Příklady množných tvarů pro překladatele.
# Čeština používá kategorie CLDR `one`, `few`, `many` a `other`; `few` pokrývá
# 2–4 a `many` desetinná čísla.
example.files_processed = { $count ->
    [one] Zpracován { $count } soubor.
    [few] Zpracovány { $count } soubory.
    [many] Zpracováno { $count } souboru.
   *[other] Zpracováno { $count } souborů.
}

example.errors_found = { $count ->
    [0] Nebyly nalezeny žádné chyby.
    [one] Nalezena { $count } chyba.
    [few] Nalezeny { $count } chyby.
    [many] Nalezeno { $count } chyby.
   *[other] Nalezeno { $count } chyb.
}
