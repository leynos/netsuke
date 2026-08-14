# A Netsuke parancssorának honosítási erőforrásai.

cli.about = A Netsuke YAML- és Jinja-jegyzékeket fordít Ninja-építési tervekké.
cli.long_about = A Netsuke a YAML- és Jinja-jegyzékeket reprodukálható Ninja-gráfokká alakítja, majd biztonságos alapértelmezésekkel futtatja a Ninját.
cli.usage = { $usage }

# Az általános kapcsolók súgószövege.
cli.flag.file.help = A használandó Netsuke-jegyzékfájl elérési útja.
cli.flag.directory.help = Úgy fusson, mintha ebben a könyvtárban indult volna.
cli.flag.config.help = Egy beállításfájl elérési útja, az automatikus keresés megkerülésével.
cli.flag.jobs.help = A párhuzamos építési feladatok számának megadása.
cli.flag.verbose.help = Részletes diagnosztikai naplózás és befejezéskori időösszegzés bekapcsolása.
cli.flag.locale.help = A parancssori szövegek nyelvi címkéje (például: en-US, hu).
cli.flag.fetch_allow_scheme.help = A fetch segédfüggvény által használható további URL-sémák.
cli.flag.fetch_allow_host.help = Engedélyezett gépnevek, ha az alapértelmezett tiltás be van kapcsolva.
cli.flag.fetch_block_host.help = Mindig letiltott gépnevek, akkor is, ha máshol engedélyezettek.
cli.flag.fetch_default_deny.help = Alapértelmezés szerint minden gép tiltása; csak a megadott lista engedélyezése.
cli.flag.json.help = Géppel olvasható JSON kimenet előállítása.
cli.flag.no_input.help = Soha ne olvasson interaktív bemenetet.
cli.flag.color.help = A színes kimenet szabálya (auto, always, never).
cli.flag.emoji.help = Az emodzsik szabálya (auto, always, never).
cli.flag.progress.help = A folyamatjelzés szabálya (auto, always, never).
cli.flag.accessibility.help = Az akadálymentes kimenet szabálya (auto, on, off).
cli.flag.default_targets.help = Alapértelmezett építési célok, ha egyet sem adnak meg.

# Az alparancsok leírása.
cli.subcommand.build.about = A jegyzékben megadott célok építése (alapértelmezett).
cli.subcommand.build.long_about = A kért célok építése; ha egyet sem adnak meg, a jegyzék alapértelmezett céljai.
cli.subcommand.clean.about = Az építési termékek eltávolítása a Ninja segítségével.
cli.subcommand.clean.long_about = Ideiglenes Ninja-fájl előállítása, majd a `ninja -t clean` futtatása.
cli.subcommand.graph.about = Az építési függőségi gráf kiírása. Az alapértelmezett formátum a DOT.
cli.subcommand.graph.long_about = A beolvasott Netsuke-jegyzék kanonikus építési gráffá alakítása és kiírása Graphviz DOT formátumban, illetve a `--html` kapcsolóval önálló HTML-oldalként. Fájlba íráshoz használja az `--output <FÁJL>` kapcsolót; a `-` a szabványos kimenetre ír.
cli.subcommand.generate.about = A Ninja-jegyzék előállítása a Ninja futtatása nélkül.
cli.subcommand.generate.long_about = Az előállított Ninja-jegyzék kiírása a szabványos kimenetre vagy az `--output` kapcsolóval megadott fájlba.
cli.subcommand.help.about = A felső szintű súgó vagy a megnevezett téma súgójának kiírása.
cli.subcommand.help.long_about = Téma nélkül ez a `--help`-nek felel meg. A `help targets` paranccsal nyomtathatja ki a kiválasztott fájl cél- és műveletkatalógusát.

# Help catalogue headings and markers.
cli.help.actions_heading = Műveletek:
cli.help.targets_heading = Célok:
cli.help.targets.about = A kiválasztott fájl céljainak és műveleteinek listázása.
cli.help.default_marker = alapértelmezett

# A build alparancs kapcsolóinak súgószövege.
cli.subcommand.build.flag.targets.help = Az építendő célok (elhagyásuk esetén a jegyzék alapértelmezett céljai).

# A graph alparancs kapcsolóinak súgószövege.
cli.subcommand.graph.flag.html.help = A gráf megjelenítése önálló HTML-oldalként a DOT formátum helyett.
cli.subcommand.graph.flag.output.help = A gráftermék kiírása a FÁJLBA; a szabványos kimenethez használja a `-` jelet.

# A generate alparancs kapcsolóinak súgószövege.
cli.subcommand.generate.flag.output.help = Az előállított Ninja-jegyzék kiírása a FÁJLBA a szabványos kimenet helyett.

# Parancssori ellenőrzési hibák.
cli.validation.jobs.invalid_number = A(z) { $value } nem érvényes szám.
cli.validation.jobs.out_of_range = A feladatok számának { $min } és { $max } között kell lennie.
cli.validation.scheme.empty = A séma nem lehet üres.
cli.validation.scheme.invalid_start = A(z) „{ $scheme }” sémának ASCII betűvel kell kezdődnie.
cli.validation.scheme.invalid = Érvénytelen séma: „{ $scheme }”.
cli.validation.locale.empty = A nyelvi címke nem lehet üres.
cli.validation.locale.invalid = Érvénytelen nyelvi címke: „{ $locale }”.
cli.validation.color.invalid = Érvénytelen színszabály: „{ $value }”. Érvényes lehetőségek: auto, always, never.
cli.validation.emoji.invalid = Érvénytelen emodzsiszabály: „{ $value }”. Érvényes lehetőségek: auto, always, never.
cli.validation.progress.invalid = Érvénytelen folyamatszabály: „{ $value }”. Érvényes lehetőségek: auto, always, never.
cli.validation.accessibility.invalid = Érvénytelen akadálymentesítési szabály: „{ $value }”. Érvényes lehetőségek: auto, on, off.
cli.validation.config.expected_object = A parancssori értékeknek objektummá kellett volna alakulniuk, de ez érkezett: { $value }.

# A Clap hibaüzenetei.
clap-error-missing-argument = Hiányzó kötelező argumentum: { $argument }
clap-error-missing-subcommand = Hiányzó alparancs. Elérhető lehetőségek: { $valid_subcommands }
clap-error-unknown-argument = Ismeretlen argumentum: { $argument }
clap-error-invalid-value = Érvénytelen érték ehhez: { $argument }: { $value }
clap-error-invalid-subcommand = Ismeretlen alparancs: { $subcommand }
# Megjegyzés: a value-validation megfogalmazása szándékosan eltér az
# invalid-value szövegétől, hogy elkülönüljenek a saját ellenőrzők hibái
# (ErrorKind::ValueValidation) a típuseltérésektől (ErrorKind::InvalidValue).
clap-error-value-validation = Az ellenőrzés sikertelen ehhez: { $argument }: { $value }

# A futtatás hibái és környezete.
runner.manifest.not_found = A(z) „{ $manifest_name }” jegyzék nem található itt: { $directory }.
runner.manifest.not_found.help = Győződjön meg róla, hogy a jegyzék létezik, vagy adja meg a `--file` kapcsolót a helyes útvonallal.
runner.manifest.path_missing_name = A(z) „{ $path }” jegyzékútvonalban nincs fájlnév.
runner.manifest.path_utf8 = A(z) „{ $path }” jegyzékútvonal nem érvényes UTF-8.
runner.manifest.directory_utf8 = A jegyzék könyvtárának útvonala („{ $path }”) nem érvényes UTF-8.
runner.manifest.directory_label = a(z) `{ $directory }` könyvtár
runner.manifest.current_directory_label = az aktuális könyvtár
runner.manifest.default_not_declared = A(z) '{ $default }' jegyzék-alapértelmezés nem nevez meg deklarált műveletet vagy célt.
runner.context.network_policy = A hálózati szabályt nem sikerült felépíteni.
runner.context.load_manifest = A jegyzéket nem sikerült betölteni innen: { $path }.
runner.context.serialise_manifest = A jegyzéket nem sikerült sorosítani.
runner.context.build_graph = A gráfot nem sikerült felépíteni a jegyzékből.
runner.context.generate_ninja = A Ninja-jegyzéket nem sikerült előállítani.
runner.context.render_graph = A gráfterméket nem sikerült megjeleníteni.

runner.io.create_temp_file = Az ideiglenes Ninja-fájlt nem sikerült létrehozni.
runner.io.write_temp_ninja = Az ideiglenes Ninja-fájlt nem sikerült megírni.
runner.io.flush_temp_ninja = Az ideiglenes Ninja-fájl pufferét nem sikerült üríteni.
runner.io.sync_temp_ninja = Az ideiglenes Ninja-fájlt nem sikerült szinkronizálni.
runner.io.create_parent_dir = A(z) { $path } szülőkönyvtárat nem sikerült létrehozni.
runner.io.create_ninja_file = A Ninja-fájlt nem sikerült létrehozni itt: { $path }.
runner.io.write_ninja_file = A Ninja-fájlt nem sikerült megírni itt: { $path }.
runner.io.flush_ninja_file = A Ninja-fájl pufferét nem sikerült üríteni itt: { $path }.
runner.io.sync_ninja_file = A Ninja-fájlt nem sikerült szinkronizálni itt: { $path }.
runner.io.open_ambient_dir = A környező könyvtárat nem sikerült megnyitni.
runner.io.non_utf8_working_directory = A munkakönyvtár útvonala nem érvényes UTF-8.
runner.io.no_existing_ancestor = A(z) { $path } útvonalhoz nincs létező szülőkönyvtár.
runner.io.derive_relative_path = A viszonylagos Ninja-útvonalat nem sikerült levezetni.
runner.io.non_utf8_path = A nem UTF-8 útvonalak nem támogatottak (útvonal: { $path }).
runner.io.write_stdout = A Ninja-jegyzéket nem sikerült a szabványos kimenetre írni.
runner.io.flush_stdout = A szabványos kimenet pufferét nem sikerült üríteni.
runner.io.dyndep.create_dir = A dyndep könyvtár létrehozása sikertelen: { $path }.
runner.io.dyndep.read = A generált dyndep fájl olvasása sikertelen innen: { $path }.
runner.io.dyndep.write = A generált dyndep fájl írása sikertelen ide: { $path }.
runner.io.dyndep.rename = A generált dyndep fájl véglegesítése sikertelen itt: { $path }.
runner.io.dyndep.corrupt = A(z) { $path } helyen található generált dyndep fájl tartalma nem egyezik a várt tartalommal; csak ezt a fájlt törölje, majd próbálja újra.
runner.io.dyndep.temp_collisions = Ismételt névütközések után nem sikerült egyedi ideiglenes dyndep fájlt létrehozni a(z) { $path } útvonalhoz.
runner.io.dyndep.too_large = A létrehozott dyndep fájl ({ $path }) meghaladja a(z) { $limit } bájtos ellenőrzési korlátot.

# Jegyzékdiagnosztika.
manifest.parse = A jegyzék feldolgozása sikertelen.
manifest.structure_error = Szerkezeti hiba a jegyzékben itt: { $name }: { $details }
manifest.yaml.parse = YAML-feldolgozási hiba a(z) { $line }. sorban, { $column }. oszlopban: { $details }
manifest.yaml.label = érvénytelen YAML
manifest.yaml.hint.tabs = A YAML nem engedélyezi a tabulátorokat; a behúzáshoz használjon szóközöket.
manifest.yaml.hint.list_item = A YAML-listaelemeknek „-” jellel kell kezdődniük, és helyesen behúzottnak kell lenniük.
manifest.yaml.hint.expected_colon = Ez leképezési bejegyzésnek tűnik; a kulcs után hiányzik a „:”.
manifest.yaml.hint.mapping_values = A YAML-leképezések a „:” után értéket igényelnek (vagy beágyazott blokkot).
manifest.yaml.hint.invalid_token = A YAML-token érvénytelen vagy váratlan.
manifest.yaml.hint.escape = Escape-elje a fordított perjeleket, vagy távolítsa el az érvénytelen escape-szekvenciákat.
manifest.env.missing = Egy kötelező környezeti változó nincs beállítva.
manifest.env.invalid_utf8 = Egy környezeti változó érvénytelen UTF-8 kódolást tartalmaz.
manifest.vars.not_object = A jegyzék `vars` mezőjének leképezésnek vagy objektumnak kell lennie.
manifest.vars.reserved_name = A manifest `vars` kulcsa, '{ $name }', egy beépített sablonsegéd számára fenntartott; nevezze át a változót.
manifest.read_failed = A jegyzéket nem sikerült beolvasni innen: { $path }.
manifest.resolve_workspace_root = A munkaterület gyökerét nem sikerült meghatározni.
manifest.workspace_non_utf8 = A munkaterület gyökérútvonala („{ $path }”) nem érvényes UTF-8.
manifest.path_non_utf8 = A(z) „{ $manifest }” jegyzék útvonala nem érvényes UTF-8: { $path }.
manifest.path_missing_name = A(z) „{ $path }” jegyzékútvonalban nincs fájlnév.
manifest.open_workspace_failed = A(z) { $workspace } munkaterületet nem sikerült megnyitni a(z) { $manifest } jegyzékhez.
manifest.foreach.not_iterable = A `foreach` kifejezés nem bejárható.
manifest.foreach.serialise_item = A `foreach` elemét nem sikerült sorosítani.
manifest.when.empty = A `when` kifejezés nem lehet üres.
manifest.when.eval_error = A(z) „{ $expr }” `when` kifejezést nem sikerült kiértékelni.
manifest.when.template_error = A(z) „{ $expr }” `when` sablont nem sikerült megjeleníteni.
manifest.target.vars_not_object = A cél `vars` mezőjének objektumnak kell lennie, de ez érkezett: { $value }.
manifest.vars.entry_not_object = A jegyzék `vars` bejegyzésének objektumnak kell lennie.
manifest.field_not_string = A(z) „{ $field }” mezőnek karakterláncnak kell lennie.
manifest.expression.parse_error = A(z) { $name } kifejezést nem sikerült feldolgozni.
manifest.expression.eval_error = A(z) { $name } kifejezést nem sikerült kiértékelni.

# A jegyzék makróinak diagnosztikája.
manifest.macro.signature_missing_identifier = A makró fejlécéből hiányzik az azonosító.
manifest.macro.signature_missing_params = A makró fejlécéből hiányoznak a paraméterek.
manifest.macro.compile_failed = A(z) { $name } makrót nem sikerült lefordítani.
manifest.macro.sequence_invalid = A makrókat nevek és sablonok leképezéseként kell megadni.
manifest.macro.register_failed = A jegyzék makróit nem sikerült regisztrálni.
manifest.macro.not_initialised = A makrókörnyezet nincs előkészítve.
manifest.macro.caller_invalid = A makró hívójának karakterláncnak kell lennie.
manifest.macro.template_load_failed = A makró sablonját nem sikerült betölteni.
manifest.macro.init_failed = A makrókörnyezetet nem sikerült előkészíteni.
manifest.macro.missing = A(z) { $name } makró hiányzik.

# A jegyzék glob-mintáinak hibái.
manifest.glob.unmatched_brace = Érvénytelen glob-minta („{ $pattern }”): a(z) „{ $character }” párja hiányzik a(z) { $position }. pozíción.
manifest.glob.invalid_pattern = Érvénytelen glob-minta („{ $pattern }”): { $detail }.
manifest.glob.unknown_pattern_error = ismeretlen mintahiba.
manifest.glob.io_failed = A glob sikertelen ehhez: „{ $pattern }”: { $detail }.
manifest.glob.unknown_io_error = ismeretlen be- és kiviteli hiba.
manifest.command_list_empty = A „command” mező nem lehet üres: adjon meg egy parancs-karakterláncot vagy egy nem üres listát.

# A köztes ábrázolás hibái.
ir.rule_not_found = A(z) „{ $target }” cél által hivatkozott „{ $rule }” szabály nem található.
ir.multiple_rules = A(z) „{ $target }” célnak pontosan egy szabályra kell hivatkoznia, de ez érkezett: { $rules }.
ir.empty_rule = A(z) „{ $target }” célnak szabályra kell hivatkoznia.
ir.duplicate_outputs = Ismétlődő kimenetek találhatók: { $outputs }.
ir.circular_dependency = Körkörös függőség található: { $cycle }.
ir.action_serialisation = A műveletet nem sikerült sorosítani: { $details }.
ir.invalid_command = Érvénytelen behelyettesítés a parancsban: { $snippet }.

# A Ninja-fájlok előállításának hibái.
ninja_gen.missing_action = Hiányzik a(z) „{ $id }” művelet, amelyre egy építési él hivatkozik.
ninja_gen.format = A Ninja-jegyzék kimenetét nem sikerült formázni.
ninja_gen.dyndep_files_required = Ehhez a buildhez generált Ninja-csomag szükséges; a dyndep fájlok létrehozásához használja a `netsuke build`, `netsuke clean` vagy `netsuke generate` parancsot.
ninja_gen.reserved_output_path = A(z) '{ $path }' útvonal a Netsuke soros függőségi állapota számára van fenntartva.
ninja_gen.unsupported_path_character = A(z) '{ $path }' útvonal nem támogatott Ninja-útvonal-karaktert tartalmaz: '{ $character }'.

# A gépminták ellenőrzése.
host_pattern.empty = A gépminta nem lehet üres.
host_pattern.contains_scheme = A(z) „{ $pattern }” gépminta nem tartalmazhat URL-sémát.
host_pattern.contains_slash = A(z) „{ $pattern }” gépminta nem tartalmazhat „/” jelet.
host_pattern.missing_suffix = A(z) „{ $pattern }” gépmintának utótagot kell tartalmaznia a „*.” után.
host_pattern.empty_label = A(z) „{ $pattern }” gépminta üres címkét tartalmaz.
host_pattern.invalid_chars = A(z) „{ $pattern }” gépminta érvénytelen karaktereket tartalmaz.
host_pattern.invalid_label_edge = A(z) „{ $pattern }” gépminta címkéi nem kezdődhetnek és nem végződhetnek „-” jellel.
host_pattern.label_too_long = A(z) „{ $pattern }” gépminta 63 karakternél hosszabb címkét tartalmaz.
host_pattern.too_long = A(z) „{ $pattern }” gépminta meghaladja a 255 karakteres korlátot.

# Hálózati szabály.
network_policy.scheme.empty = A séma nem lehet üres.
network_policy.scheme.invalid = A(z) „{ $scheme }” séma érvénytelen karaktereket tartalmaz.
network_policy.allowlist.empty = Az engedélyezett gépek listája nem lehet üres.
network_policy.scheme.not_allowed = A(z) „{ $scheme }” séma nem engedélyezett.
network_policy.missing_host = Az URL-ből hiányzik a gép.
network_policy.host.blocked = A(z) „{ $host }” gépet a szabály letiltja.
network_policy.host.not_allowlisted = A(z) „{ $host }” gép nem szerepel az engedélyezettek listáján.

# A szabványos programkönyvtár beállításai.
stdlib.config.default_fetch_cache_invalid = A fetch gyorsítótárának alapértelmezett útvonalának viszonylagosnak kell lennie.
stdlib.config.default_which_cache_invalid = A which gyorsítótárának alapértelmezett kapacitásának pozitívnak kell lennie.
stdlib.config.workspace_root_absolute = A munkaterület gyökérútvonalának abszolútnak kell lennie.
stdlib.config.fetch_response_limit_positive = A fetch válaszkorlátjának pozitívnak kell lennie.
stdlib.config.command_output_limit_positive = A parancskimenet rögzítési korlátjának pozitívnak kell lennie.
stdlib.config.command_stream_limit_positive = A parancsok folyamkorlátjának pozitívnak kell lennie.
stdlib.config.which_cache_capacity_positive = A which gyorsítótárának kapacitásának pozitívnak kell lennie.
stdlib.config.skip_dir_empty = A kihagyandó könyvtárak bejegyzései nem lehetnek üresek.
stdlib.config.skip_dir_navigation = A kihagyandó könyvtárak bejegyzései nem tartalmazhatnak „..” elemet.
stdlib.config.skip_dir_separator = A kihagyandó könyvtárak bejegyzései nem tartalmazhatnak útvonal-elválasztókat.
stdlib.config.fetch_cache_empty = A fetch gyorsítótárának útvonala nem lehet üres.
stdlib.config.fetch_cache_not_relative = A fetch gyorsítótárának útvonalának viszonylagosnak kell lennie, de ez érkezett: { $path }.
stdlib.config.fetch_cache_escapes = A fetch gyorsítótárának útvonala nem léphet ki a munkaterületből: { $path }.
stdlib.config.open_workspace_root = Az aktuális könyvtárat nem sikerült megnyitni a stdlib munkaterületének gyökereként.
stdlib.config.resolve_cwd = Az aktuális könyvtárat nem sikerült meghatározni a stdlib munkaterületének gyökereként.
stdlib.config.cwd_non_utf8 = Az aktuális könyvtár nem UTF-8 részeket tartalmaz: { $path }.

# A fetch segédfüggvény diagnosztikája.
stdlib.fetch.url_invalid = Érvénytelen URL („{ $url }”): { $details }.
stdlib.fetch.disallowed = A(z) „{ $url }” URL nem engedélyezett: { $details }.
stdlib.fetch.failed = A(z) „{ $url }” letöltése sikertelen: { $details }.
stdlib.fetch.cache_read_failed = A(z) „{ $name }” gyorsítótár-bejegyzést nem sikerült beolvasni: { $details }.
stdlib.fetch.cache_open_failed = A(z) „{ $name }” gyorsítótár-bejegyzést nem sikerült megnyitni: { $details }.
stdlib.fetch.response_read_failed = A(z) „{ $url }” válaszát nem sikerült beolvasni: { $details }.
stdlib.fetch.response_buffer_overflow = Puffertúlcsordulás a(z) „{ $url }” olvasása közben.
stdlib.fetch.cache_write_failed = A(z) „{ $url }” gyorsítótárát nem sikerült megírni: { $details }.
stdlib.fetch.response_limit_exceeded = A(z) „{ $url }” válasza meghaladta a(z) { $limit } bájtos korlátot.
stdlib.fetch.cache_limit_exceeded = A gyorsítótárazott „{ $name }” válasz meghaladta a(z) { $limit } bájtos korlátot.
stdlib.fetch.io_failed = A(z) „{ $action }” művelet sikertelen ehhez: { $path }: { $details }.
stdlib.fetch.action.sync_cache = a fetch gyorsítótárának szinkronizálása
stdlib.fetch.action.create_cache_dir = a fetch gyorsítótár-könyvtárának létrehozása
stdlib.fetch.action.open_cache_dir = a fetch gyorsítótár-könyvtárának megnyitása
stdlib.fetch.action.stat_cache = a fetch gyorsítótár-bejegyzésének lekérdezése
stdlib.fetch.action.open_cache_entry = a fetch gyorsítótár-bejegyzésének megnyitása

# A parancsokat kezelő segédfüggvény diagnosztikája.
stdlib.command.location = a(z) „{ $command }” parancs a(z) „{ $template }” sablonban
stdlib.command.spawn_failed = A(z) { $location } indítása sikertelen: { $details }.
stdlib.command.io_failed = A(z) { $location } sikertelen: { $details }.
stdlib.command.closed_input_early = A bemenet lezárult, mielőtt a parancs írása befejeződött volna.
stdlib.command.broken_pipe = Megszakadt csővezeték a(z) { $location } futtatása közben: { $details }.
stdlib.command.terminated_by_signal = A(z) { $location } futását jelzés szakította meg.
stdlib.command.exited_with_status = A(z) { $location } { $status } állapottal fejeződött be.
stdlib.command.output_limit_exceeded = A(z) { $location } túllépte a(z) { $mode } { $limit } bájtos korlátját ehhez: { $stream }.
stdlib.command.timeout = A(z) { $location } túllépte a(z) { $seconds } másodperces időkorlátot.
stdlib.command.exit_status_suffix = (kilépési állapot: { $status })
stdlib.command.signal_suffix = (jelzés szakította meg)
stdlib.command.shell.empty = A parancsértelmezőnek szóló parancs nem lehet üres.
stdlib.command.grep.empty_pattern = A grep mintája nem lehet üres.
stdlib.command.grep.flags_not_string = A grep kapcsolóinak karakterláncnak kell lenniük.
stdlib.command.quote.invalid = A(z) { $arg } idézőjelezése sikertelen: { $details }.
stdlib.command.quote.line_break = A kocsivissza vagy soremelés karaktert tartalmazó argumentumok nem idézőjelezhetők biztonságosan.
stdlib.command.input_undefined = A bemeneti érték nincs meghatározva.
stdlib.command.tempfile.root_required = Az ideiglenes parancsfájlok létrehozásához szükség van a munkaterület gyökerére.
stdlib.command.tempfile.create_failed = Az ideiglenes parancsfájlt nem sikerült létrehozni: { $details }.
stdlib.command.options.invalid_utf8 = A parancs beállításkulcsának érvényes UTF-8 kódolásúnak kell lennie.
stdlib.command.option.mode_not_string = A kimeneti módnak karakterláncnak kell lennie.
stdlib.command.options.invalid_type = A parancs beállításainak objektumnak kell lenniük.
stdlib.command.output.mode_unsupported = Nem támogatott kimeneti mód: „{ $mode }”.
stdlib.command.output.mode.capture = rögzítés
stdlib.command.output.mode.streaming = folyamatos átvitel
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Az útvonalakat kezelő segédfüggvény diagnosztikája.
stdlib.path.io.failed = A(z) „{ $action }” művelet sikertelen ehhez: { $path } ({ $label }).
stdlib.path.io.failed_with_detail = A(z) „{ $action }” művelet sikertelen ehhez: { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = A(z) „{ $action }” művelet sikertelen ehhez: { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = nem található
stdlib.path.io.permission_denied = hozzáférés megtagadva
stdlib.path.io.already_exists = már létezik
stdlib.path.io.invalid_input = érvénytelen bemenet
stdlib.path.io.invalid_data = érvénytelen adat
stdlib.path.io.timed_out = időtúllépés
stdlib.path.io.interrupted = megszakítva
stdlib.path.io.would_block = blokkolást okozna
stdlib.path.io.write_zero = nulla bájt íródott
stdlib.path.io.unexpected_eof = váratlan fájlvég
stdlib.path.io.broken_pipe = megszakadt csővezeték
stdlib.path.io.connection_refused = a kapcsolat elutasítva
stdlib.path.io.connection_reset = a kapcsolat visszaállítva
stdlib.path.io.connection_aborted = a kapcsolat megszakítva
stdlib.path.io.not_connected = nincs kapcsolat
stdlib.path.io.addr_in_use = a cím már használatban van
stdlib.path.io.addr_not_available = a cím nem érhető el
stdlib.path.io.out_of_memory = elfogyott a memória
stdlib.path.io.unsupported = nem támogatott
stdlib.path.io.file_too_large = a fájl túl nagy
stdlib.path.io.resource_busy = az erőforrás foglalt
stdlib.path.io.executable_busy = a futtatható fájl foglalt
stdlib.path.io.deadlock = holtpont
stdlib.path.io.crosses_devices = eszközhatárt lép át
stdlib.path.io.too_many_links = túl sok hivatkozás
stdlib.path.io.invalid_filename = érvénytelen fájlnév
stdlib.path.io.arg_list_too_long = túl hosszú argumentumlista
stdlib.path.io.stale_handle = elavult hálózati fájlleíró
stdlib.path.io.storage_full = a tároló megtelt
stdlib.path.io.not_seekable = nem pozicionálható
stdlib.path.io.network_down = a hálózat nem működik
stdlib.path.io.network_unreachable = a hálózat nem érhető el
stdlib.path.io.host_unreachable = a gép nem érhető el
stdlib.path.io.other = be- és kiviteli hiba
stdlib.path.action.canonicalize = kanonizálás
stdlib.path.action.open_directory = könyvtár megnyitása
stdlib.path.action.stat = lekérdezés
stdlib.path.action.read = olvasás
stdlib.path.action.open_file = fájl megnyitása
stdlib.path.with_suffix.empty_separator = A with_suffix nem üres elválasztót igényel.
stdlib.path.relative_to.mismatch = A(z) { $path } nem viszonyítható ehhez: { $root }.
stdlib.path.expanduser.unsupported = A ~ jel adott felhasználóra vonatkozó kibontása nem támogatott.
stdlib.path.expanduser.no_home = A ~ jel nem bontható ki: nincs beállítva a saját könyvtárra vonatkozó környezeti változó.
stdlib.path.contents.unsupported_encoding = Nem támogatott kódolás: „{ $encoding }”.
stdlib.path.hash.unsupported_algorithm = Nem támogatott kivonatoló algoritmus: „{ $algorithm }”.
stdlib.path.hash.unsupported_algorithm_legacy = Nem támogatott kivonatoló algoritmus: „{ $algorithm }” (kapcsolja be a(z) „{ $feature }” szolgáltatást).

# A gyűjteményeket kezelő segédfüggvények diagnosztikája.
stdlib.collections.flatten.expected_sequence = A flatten sorozatelemeket várt, de ezt találta: { $kind }.
stdlib.collections.group_by.empty_attribute = A group_by nem üres attribútumot igényel.
stdlib.collections.group_by.unresolved = A group_by nem találta a(z) „{ $attr }” attribútumot a(z) { $kind } típusú elemen.

# Az időkezelő segédfüggvények diagnosztikája.
stdlib.time.offset.invalid = A now eltolása („{ $offset }”) érvénytelen: „+HH:MM[:SS]” vagy „Z” formátum szükséges.
stdlib.time.timedelta.overflow = Túlcsordulás a timedelta műveletben a(z) { $component } hozzáadásakor.
stdlib.time.label.weeks = hét
stdlib.time.label.days = nap
stdlib.time.label.hours = óra
stdlib.time.label.minutes = perc
stdlib.time.label.seconds = másodperc
stdlib.time.label.milliseconds = ezredmásodperc
stdlib.time.label.microseconds = mikromásodperc
stdlib.time.label.nanoseconds = nanomásodperc

# A which segédfüggvény diagnosztikája.
stdlib.which.not_found = [netsuke::jinja::which::not_found] a(z) „{ $command }” parancs nem található { $count } PATH-bejegyzés ellenőrzése után. Előnézet: { $preview }
stdlib.which.not_found.hint.cwd_auto = A PATH üres szakaszait a rendszer figyelmen kívül hagyja; a munkakönyvtár bevonásához használja a cwd_mode="auto" beállítást.
stdlib.which.not_found.hint.cwd_always = Az aktuális könyvtár bevonásához állítsa be a cwd_mode="always" értéket.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] a(z) „{ $command }” parancs itt: „{ $path }” hiányzik, vagy nem futtatható.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <üres>
stdlib.which.path_entry.non_utf8 = A(z) { $index }. PATH-bejegyzés nem UTF-8 karaktereket tartalmaz; a Netsuke UTF-8 útvonalakat igényel.
stdlib.which.command.empty = A which nem üres karakterláncot igényel.
stdlib.which.cwd_mode.invalid = A cwd_mode értéke „auto”, „always” vagy „never” lehet, de ez érkezett: „{ $mode }”.
stdlib.which.cwd.resolve_failed = Az aktuális könyvtárat nem sikerült meghatározni: { $details }.
stdlib.which.cwd.non_utf8 = Az aktuális könyvtár nem UTF-8 részeket tartalmaz.
stdlib.which.canonicalize_failed = A(z) „{ $path }” kanonizálása sikertelen: { $details }.
stdlib.which.is_executable = Nem sikerült megállapítani, hogy a(z) „{ $path }” futtatható-e: { $details }.
stdlib.which.canonicalize_non_utf8 = A kanonikus útvonal nem UTF-8 részeket tartalmaz.
stdlib.which.workspace_non_utf8 = A munkaterület útvonala nem UTF-8 részeket tartalmaz a(z) „{ $command }” parancs feloldásakor: { $path }.
stdlib.which.walkdir_error = Hiba a munkaterület bejárása közben a parancs feloldásakor: { $details }.

# A szabványos programkönyvtár regisztrálása.
stdlib.register.open_dir = Az aktuális könyvtárat nem sikerült megnyitni a stdlib regisztrálásához.
stdlib.register.resolve_dir = Az aktuális könyvtárat nem sikerült meghatározni a stdlib regisztrálásához.
stdlib.register.dir_non_utf8 = Az aktuális könyvtár nem UTF-8 részeket tartalmaz: { $path }.

# Állapotjelentés akadálymentes kimeneti módban.
status.state.pending = várakozik
status.state.running = folyamatban
status.state.done = kész
status.state.failed = sikertelen
status.stage.label = { $current }/{ $total }. szakasz: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = { $current }/{ $total }. feladat
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = A jegyzékfájl beolvasása
status.stage.initial_yaml_parsing = A YAML-dokumentum feldolgozása
status.stage.template_expansion = A sablondirektívák kibontása
status.stage.final_rendering = A jegyzék értékeinek visszafejtése és megjelenítése
status.stage.ir_generation_validation = A függőségi gráf felépítése és ellenőrzése
status.stage.ninja_synthesis = A Ninja-építési terv összeállítása
status.stage.ninja_synthesis_execute = A Ninja-terv összeállítása és a(z) { $tool } futtatása
status.stage.graph_rendering = A gráftermék megjelenítése
status.stage.graph_rendering_with_tool = A(z) { $tool } megjelenítése
status.complete = { $tool }: kész.
status.timing.summary_header = Szakaszonkénti időösszegzés:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = A folyamat teljes ideje: { $duration }
status.tool.build = Építés
status.tool.clean = Tisztítás
status.tool.graph = Gráf
status.tool.graph_html = Gráf (HTML)
status.tool.generate = Előállítás
status.tool.help_targets = Célsúgó

# A gráf HTML-megjelenítésének szövegei.
graph.html.title = Netsuke építési gráf
graph.html.heading = Netsuke építési gráf
graph.html.description = A Netsuke által megjelenített építési gráf
graph.html.outline.summary = Célok és függőségek (szöveges vázlat)
graph.html.outline.no_inputs = Nincs bemenet
graph.html.noscript.notice = A JavaScript ki van kapcsolva. A fenti szöveges vázlat a teljes gráfot tartalmazza; alább a DOT-forrás következik.

# Jelentéstani előtagok az akadálymentes kimenethez.
semantic.prefix.error = Hiba:
semantic.prefix.warning = Figyelmeztetés:
semantic.prefix.success = Sikeres:
semantic.prefix.info = Tájékoztatás:
semantic.prefix.timing = Idő:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Többes számú alakok példái fordítóknak.
# A CLDR szerint a magyarnak `one` és `other` kategóriája van, de a számnév
# után a főnév egyes számban marad, ezért a két változat szövege azonos:
# „1 fájl”, „5 fájl”.
example.files_processed = { $count ->
    [one] { $count } fájl feldolgozva.
   *[other] { $count } fájl feldolgozva.
}

example.errors_found = { $count ->
    [0] Nem található hiba.
    [one] { $count } hiba található.
   *[other] { $count } hiba található.
}
