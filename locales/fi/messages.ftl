# Netsuken komentorivin lokalisointiresurssit.

cli.about = Netsuke kääntää YAML- ja Jinja-manifestit Ninja-koontisuunnitelmiksi.
cli.long_about = Netsuke muuntaa YAML- ja Jinja-manifestit toistettaviksi Ninja-graafeiksi ja suorittaa Ninjan turvallisin oletusasetuksin.
cli.usage = { $usage }

# Yleisten valitsimien ohjeteksti.
cli.flag.file.help = Käytettävän Netsuke-manifestitiedoston polku.
cli.flag.directory.help = Suorita ikään kuin ohjelma olisi käynnistetty tässä hakemistossa.
cli.flag.config.help = Asetustiedoston polku, joka ohittaa automaattisen haun.
cli.flag.jobs.help = Aseta rinnakkaisten koontitöiden määrä.
cli.flag.verbose.help = Ota käyttöön yksityiskohtainen diagnostiikkaloki ja ajoituskoosteet lopuksi.
cli.flag.locale.help = Komentorivin tekstien kielitunnus (esimerkiksi: en-US, fi).
cli.flag.fetch_allow_scheme.help = Lisää URL-skeemoja, jotka fetch-apuri saa käyttää.
cli.flag.fetch_allow_host.help = Sallitut isäntänimet, kun oletusesto on käytössä.
cli.flag.fetch_block_host.help = Isäntänimet, jotka estetään aina, vaikka ne sallittaisiin muualla.
cli.flag.fetch_default_deny.help = Estä kaikki isännät oletuksena; salli vain määritelty luettelo.
cli.flag.json.help = Tuota koneluettavaa JSON-tulostetta.
cli.flag.no_input.help = Älä koskaan lue vuorovaikutteista syötettä.
cli.flag.color.help = Väritulosteen käytäntö (auto, always, never).
cli.flag.emoji.help = Emojien käytäntö (auto, always, never).
cli.flag.progress.help = Edistymisen näyttämisen käytäntö (auto, always, never).
cli.flag.accessibility.help = Saavutettavan tulosteen käytäntö (auto, on, off).
cli.flag.default_targets.help = Koonnin oletuskohteet, kun mitään ei ole annettu.

# Alikomentojen kuvaukset.
cli.subcommand.build.about = Koosta manifestissa määritellyt kohteet (oletus).
cli.subcommand.build.long_about = Koosta pyydetyt kohteet; jos niitä ei anneta, käytä manifestin oletuskohteita.
cli.subcommand.clean.about = Poista koonnin tuotokset Ninjan avulla.
cli.subcommand.clean.long_about = Luo väliaikainen Ninja-tiedosto ja suorita sitten `ninja -t clean`.
cli.subcommand.graph.about = Tulosta koonnin riippuvuusgraafi. Oletusmuoto on DOT.
cli.subcommand.graph.long_about = Muunna luettu Netsuke-manifesti kanoniseksi koontigraafiksi ja kirjoita se Graphviz DOT -muodossa tai `--html`-valitsimella itsenäisenä HTML-sivuna. Kirjoita tiedostoon valitsimella `--output <TIEDOSTO>`; `-` kirjoittaa vakiotulosteeseen.
cli.subcommand.generate.about = Luo Ninja-manifesti suorittamatta Ninjaa.
cli.subcommand.generate.long_about = Kirjoita luotu Ninja-manifesti vakiotulosteeseen tai valitsimella `--output` valittuun tiedostoon.
cli.subcommand.help.about = Tulosta ylimmän tason ohje tai nimetyn aiheen ohje.
cli.subcommand.help.long_about = Ilman aihetta tämä vastaa `--help`-komentoa. Käytä `help targets` tulostaaksesi valitun tiedoston kohde- ja toimintaluettelon.

# Help catalogue headings and markers.
cli.help.actions_heading = Toiminnot:
cli.help.targets_heading = Kohteet:
cli.help.targets.about = Luettele valitun tiedoston kohteet ja toiminnot.
cli.help.default_marker = oletus

# build-alikomennon valitsimien ohjeteksti.
cli.subcommand.build.flag.targets.help = Koostettavat kohteet (jos puuttuu, käytetään manifestin oletuskohteita).

# graph-alikomennon valitsimien ohjeteksti.
cli.subcommand.graph.flag.html.help = Hahmonna graafi itsenäisenä HTML-sivuna DOT-muodon sijaan.
cli.subcommand.graph.flag.output.help = Kirjoita graafituotos TIEDOSTOon; käytä `-` vakiotulosteeseen.

# generate-alikomennon valitsimien ohjeteksti.
cli.subcommand.generate.flag.output.help = Kirjoita luotu Ninja-manifesti TIEDOSTOon vakiotulosteen sijaan.

# Komentorivin kelpoisuustarkistusten virheet.
cli.validation.jobs.invalid_number = { $value } ei ole kelvollinen luku.
cli.validation.jobs.out_of_range = Töiden määrän on oltava välillä { $min }–{ $max }.
cli.validation.scheme.empty = Skeema ei saa olla tyhjä.
cli.validation.scheme.invalid_start = Skeeman ”{ $scheme }” on alettava ASCII-kirjaimella.
cli.validation.scheme.invalid = Virheellinen skeema ”{ $scheme }”.
cli.validation.locale.empty = Kielitunnus ei saa olla tyhjä.
cli.validation.locale.invalid = Virheellinen kielitunnus ”{ $locale }”.
cli.validation.color.invalid = Virheellinen värikäytäntö ”{ $value }”. Kelvolliset vaihtoehdot: auto, always, never.
cli.validation.emoji.invalid = Virheellinen emojikäytäntö ”{ $value }”. Kelvolliset vaihtoehdot: auto, always, never.
cli.validation.progress.invalid = Virheellinen edistymiskäytäntö ”{ $value }”. Kelvolliset vaihtoehdot: auto, always, never.
cli.validation.accessibility.invalid = Virheellinen saavutettavuuskäytäntö ”{ $value }”. Kelvolliset vaihtoehdot: auto, on, off.
cli.validation.config.expected_object = Komentorivin arvojen piti sarjallistua objektiksi, mutta saatiin { $value }.

# Clapin virheilmoitukset.
clap-error-missing-argument = Pakollinen argumentti puuttuu: { $argument }
clap-error-missing-subcommand = Alikomento puuttuu. Käytettävissä olevat vaihtoehdot: { $valid_subcommands }
clap-error-unknown-argument = Tuntematon argumentti: { $argument }
clap-error-invalid-value = Virheellinen arvo argumentille { $argument }: { $value }
clap-error-invalid-subcommand = Tuntematon alikomento: { $subcommand }
# Huomio: value-validation on muotoiltu eri tavalla kuin invalid-value, jotta
# omien tarkistimien virheet (ErrorKind::ValueValidation) erottuvat
# tyyppiristiriidoista (ErrorKind::InvalidValue).
clap-error-value-validation = Kelpoisuustarkistus epäonnistui argumentille { $argument }: { $value }

# Suorittajan virheet ja konteksti.
runner.manifest.not_found = Manifestia ”{ $manifest_name }” ei löytynyt hakemistosta { $directory }.
runner.manifest.not_found.help = Varmista, että manifesti on olemassa, tai anna `--file` oikealla polulla.
runner.manifest.path_missing_name = Manifestipolussa ”{ $path }” ei ole tiedostonimeä.
runner.manifest.path_utf8 = Manifestipolku ”{ $path }” ei ole kelvollista UTF-8:aa.
runner.manifest.directory_utf8 = Manifestihakemiston polku ”{ $path }” ei ole kelvollista UTF-8:aa.
runner.manifest.directory_label = hakemisto `{ $directory }`
runner.manifest.current_directory_label = nykyinen hakemisto
runner.context.network_policy = Verkkokäytäntöä ei voitu muodostaa.
runner.context.load_manifest = Manifestia ei voitu ladata polusta { $path }.
runner.context.serialise_manifest = Manifestia ei voitu sarjallistaa.
runner.context.build_graph = Graafia ei voitu muodostaa manifestista.
runner.context.generate_ninja = Ninja-manifestia ei voitu luoda.
runner.context.render_graph = Graafituotosta ei voitu hahmontaa.

runner.io.create_temp_file = Väliaikaista Ninja-tiedostoa ei voitu luoda.
runner.io.write_temp_ninja = Väliaikaista Ninja-tiedostoa ei voitu kirjoittaa.
runner.io.flush_temp_ninja = Väliaikaisen Ninja-tiedoston puskuria ei voitu tyhjentää.
runner.io.sync_temp_ninja = Väliaikaista Ninja-tiedostoa ei voitu synkronoida.
runner.io.create_parent_dir = Ylähakemistoa { $path } ei voitu luoda.
runner.io.create_ninja_file = Ninja-tiedostoa polkuun { $path } ei voitu luoda.
runner.io.write_ninja_file = Ninja-tiedostoa polussa { $path } ei voitu kirjoittaa.
runner.io.flush_ninja_file = Ninja-tiedoston puskuria polussa { $path } ei voitu tyhjentää.
runner.io.sync_ninja_file = Ninja-tiedostoa polussa { $path } ei voitu synkronoida.
runner.io.open_ambient_dir = Ympäröivää hakemistoa ei voitu avata.
runner.io.no_existing_ancestor = Polulle { $path } ei löydy olemassa olevaa ylähakemistoa.
runner.io.derive_relative_path = Suhteellista Ninja-polkua ei voitu johtaa.
runner.io.non_utf8_path = Polkuja, jotka eivät ole UTF-8:aa, ei tueta (polku: { $path }).
runner.io.write_stdout = Ninja-manifestia ei voitu kirjoittaa vakiotulosteeseen.
runner.io.flush_stdout = Vakiotulosteen puskuria ei voitu tyhjentää.

# Manifestin diagnostiikka.
manifest.parse = Manifestin jäsentäminen epäonnistui.
manifest.structure_error = Manifestin rakennevirhe kohdassa { $name }: { $details }
manifest.yaml.parse = YAML-jäsennysvirhe rivillä { $line }, sarakkeessa { $column }: { $details }
manifest.yaml.label = virheellinen YAML
manifest.yaml.hint.tabs = YAML ei salli sarkaimia; käytä sisennykseen välilyöntejä.
manifest.yaml.hint.list_item = YAML-luettelon alkioiden on alettava merkillä ”-” ja oltava oikein sisennettyjä.
manifest.yaml.hint.expected_colon = Tämä näyttää avain-arvo-parilta; avaimen jälkeen puuttuu ”:”.
manifest.yaml.hint.mapping_values = YAML-kuvaukset vaativat arvon merkin ”:” jälkeen (tai sisennetyn lohkon).
manifest.yaml.hint.invalid_token = YAML-tunnus on virheellinen tai odottamaton.
manifest.yaml.hint.escape = Suojaa kenoviivat tai poista virheelliset ohjausmerkkijonot.
manifest.env.missing = Vaadittua ympäristömuuttujaa ei ole asetettu.
manifest.env.invalid_utf8 = Ympäristömuuttuja sisältää virheellistä UTF-8:aa.
manifest.vars.not_object = Manifestin `vars` on oltava kuvaus tai objekti.
manifest.vars.reserved_name = Manifestin `vars`-avain '{ $name }' on varattu sisäänrakennetulle mallineapufunktiolle; nimeä muuttuja uudelleen.
manifest.read_failed = Manifestia ei voitu lukea polusta { $path }.
manifest.resolve_workspace_root = Työtilan juurta ei voitu selvittää.
manifest.workspace_non_utf8 = Työtilan juuripolku ”{ $path }” ei ole kelvollista UTF-8:aa.
manifest.path_non_utf8 = Manifestin ”{ $manifest }” polku ei ole kelvollista UTF-8:aa: { $path }.
manifest.path_missing_name = Manifestipolussa ”{ $path }” ei ole tiedostonimeä.
manifest.open_workspace_failed = Työtilaa { $workspace } ei voitu avata manifestille { $manifest }.
manifest.foreach.not_iterable = Lauseke `foreach` ei ole läpikäytävissä.
manifest.foreach.serialise_item = Lausekkeen `foreach` alkiota ei voitu sarjallistaa.
manifest.when.empty = Lauseke `when` ei saa olla tyhjä.
manifest.when.eval_error = Lauseketta `when` ”{ $expr }” ei voitu evaluoida.
manifest.when.template_error = Mallipohjaa `when` ”{ $expr }” ei voitu hahmontaa.
manifest.target.vars_not_object = Kohteen `vars` on oltava objekti, mutta saatiin { $value }.
manifest.vars.entry_not_object = Manifestin `vars`-merkinnän on oltava objekti.
manifest.field_not_string = Kentän ”{ $field }” on oltava merkkijono.
manifest.expression.parse_error = Lauseketta { $name } ei voitu jäsentää.
manifest.expression.eval_error = Lauseketta { $name } ei voitu evaluoida.

# Manifestin makrojen diagnostiikka.
manifest.macro.signature_missing_identifier = Makron esittelystä puuttuu tunniste.
manifest.macro.signature_missing_params = Makron esittelystä puuttuvat parametrit.
manifest.macro.compile_failed = Makroa { $name } ei voitu kääntää.
manifest.macro.sequence_invalid = Makrot on määriteltävä nimien ja mallipohjien kuvauksena.
manifest.macro.register_failed = Manifestin makroja ei voitu rekisteröidä.
manifest.macro.not_initialised = Makroympäristöä ei ole alustettu.
manifest.macro.caller_invalid = Makron kutsujan on oltava merkkijono.
manifest.macro.template_load_failed = Makron mallipohjaa ei voitu ladata.
manifest.macro.init_failed = Makroympäristöä ei voitu alustaa.
manifest.macro.missing = Makro { $name } puuttuu.

# Manifestin glob-virheet.
manifest.glob.unmatched_brace = Virheellinen glob-hahmo ”{ $pattern }”: merkillä ”{ $character }” ei ole paria kohdassa { $position }.
manifest.glob.invalid_pattern = Virheellinen glob-hahmo ”{ $pattern }”: { $detail }.
manifest.glob.unknown_pattern_error = tuntematon hahmovirhe.
manifest.glob.io_failed = Glob epäonnistui hahmolle ”{ $pattern }”: { $detail }.
manifest.glob.unknown_io_error = tuntematon siirräntävirhe.
manifest.command_list_empty = Kenttä ”command” ei saa olla tyhjä: anna komentomerkkijono tai ei-tyhjä luettelo.

# Välimuotoesityksen virheet.
ir.rule_not_found = Sääntöä ”{ $rule }”, johon kohde ”{ $target }” viittaa, ei löytynyt.
ir.multiple_rules = Kohteen ”{ $target }” on viitattava täsmälleen yhteen sääntöön, mutta saatiin { $rules }.
ir.empty_rule = Kohteen ”{ $target }” on viitattava sääntöön.
ir.duplicate_outputs = Havaittiin päällekkäisiä tulosteita: { $outputs }.
ir.circular_dependency = Havaittiin kehäriippuvuus: { $cycle }.
ir.action_serialisation = Toimintoa ei voitu sarjallistaa: { $details }.
ir.invalid_command = Virheellinen komennon sijoitus: { $snippet }.

# Ninja-generoinnin virheet.
ninja_gen.missing_action = Toiminto ”{ $id }”, johon koontikaari viittaa, puuttuu.
ninja_gen.format = Ninja-manifestin tulostetta ei voitu muotoilla.

# Isäntähahmojen tarkistus.
host_pattern.empty = Isäntähahmo ei saa olla tyhjä.
host_pattern.contains_scheme = Isäntähahmo ”{ $pattern }” ei saa sisältää URL-skeemaa.
host_pattern.contains_slash = Isäntähahmo ”{ $pattern }” ei saa sisältää merkkiä ”/”.
host_pattern.missing_suffix = Isäntähahmossa ”{ $pattern }” on oltava pääte merkkijonon ”*.” jälkeen.
host_pattern.empty_label = Isäntähahmo ”{ $pattern }” sisältää tyhjän nimiön.
host_pattern.invalid_chars = Isäntähahmo ”{ $pattern }” sisältää virheellisiä merkkejä.
host_pattern.invalid_label_edge = Isäntähahmon ”{ $pattern }” nimiöt eivät saa alkaa tai päättyä merkkiin ”-”.
host_pattern.label_too_long = Isäntähahmo ”{ $pattern }” sisältää yli 63 merkin nimiön.
host_pattern.too_long = Isäntähahmo ”{ $pattern }” ylittää 255 merkin rajan.

# Verkkokäytäntö.
network_policy.scheme.empty = Skeema ei saa olla tyhjä.
network_policy.scheme.invalid = Skeema ”{ $scheme }” sisältää virheellisiä merkkejä.
network_policy.allowlist.empty = Sallittujen isäntien luettelo ei saa olla tyhjä.
network_policy.scheme.not_allowed = Skeema ”{ $scheme }” ei ole sallittu.
network_policy.missing_host = URL-osoitteesta puuttuu isäntä.
network_policy.host.blocked = Käytäntö estää isännän ”{ $host }”.
network_policy.host.not_allowlisted = Isäntä ”{ $host }” ei ole sallittujen luettelossa.

# Vakiokirjaston asetukset.
stdlib.config.default_fetch_cache_invalid = fetch-välimuistin oletuspolun on oltava suhteellinen.
stdlib.config.default_which_cache_invalid = which-välimuistin oletuskapasiteetin on oltava positiivinen.
stdlib.config.workspace_root_absolute = Työtilan juuripolun on oltava absoluuttinen.
stdlib.config.fetch_response_limit_positive = fetch-vastauksen rajan on oltava positiivinen.
stdlib.config.command_output_limit_positive = Komennon tulosteen talteenoton rajan on oltava positiivinen.
stdlib.config.command_stream_limit_positive = Komennon virtausrajan on oltava positiivinen.
stdlib.config.which_cache_capacity_positive = which-välimuistin kapasiteetin on oltava positiivinen.
stdlib.config.skip_dir_empty = Ohitettavien hakemistojen merkinnät eivät saa olla tyhjiä.
stdlib.config.skip_dir_navigation = Ohitettavien hakemistojen merkinnät eivät saa sisältää merkkijonoa ”..”.
stdlib.config.skip_dir_separator = Ohitettavien hakemistojen merkinnät eivät saa sisältää polkuerottimia.
stdlib.config.fetch_cache_empty = fetch-välimuistin polku ei saa olla tyhjä.
stdlib.config.fetch_cache_not_relative = fetch-välimuistin polun on oltava suhteellinen, mutta saatiin { $path }.
stdlib.config.fetch_cache_escapes = fetch-välimuistin polku ei saa johtaa työtilan ulkopuolelle: { $path }.
stdlib.config.open_workspace_root = Nykyistä hakemistoa ei voitu avata stdlib-työtilan juureksi.
stdlib.config.resolve_cwd = Nykyistä hakemistoa ei voitu selvittää stdlib-työtilan juureksi.
stdlib.config.cwd_non_utf8 = Nykyinen hakemisto sisältää osia, jotka eivät ole UTF-8:aa: { $path }.

# fetch-apurin diagnostiikka.
stdlib.fetch.url_invalid = Virheellinen URL-osoite ”{ $url }”: { $details }.
stdlib.fetch.disallowed = URL-osoite ”{ $url }” ei ole sallittu: { $details }.
stdlib.fetch.failed = Osoitteesta ”{ $url }” ei voitu hakea: { $details }.
stdlib.fetch.cache_read_failed = Välimuistimerkintää ”{ $name }” ei voitu lukea: { $details }.
stdlib.fetch.cache_open_failed = Välimuistimerkintää ”{ $name }” ei voitu avata: { $details }.
stdlib.fetch.response_read_failed = Vastausta osoitteesta ”{ $url }” ei voitu lukea: { $details }.
stdlib.fetch.response_buffer_overflow = Puskurin ylivuoto luettaessa osoitetta ”{ $url }”.
stdlib.fetch.cache_write_failed = Välimuistia osoitteelle ”{ $url }” ei voitu kirjoittaa: { $details }.
stdlib.fetch.response_limit_exceeded = Vastaus osoitteesta ”{ $url }” ylitti { $limit } tavun rajan.
stdlib.fetch.cache_limit_exceeded = Välimuistiin tallennettu vastaus ”{ $name }” ylitti { $limit } tavun rajan.
stdlib.fetch.io_failed = { $action } epäonnistui polun { $path } käsittelyssä: { $details }.
stdlib.fetch.action.sync_cache = fetch-välimuistin synkronointi
stdlib.fetch.action.create_cache_dir = fetch-välimuistihakemiston luonti
stdlib.fetch.action.open_cache_dir = fetch-välimuistihakemiston avaus
stdlib.fetch.action.stat_cache = fetch-välimuistimerkinnän haku
stdlib.fetch.action.open_cache_entry = fetch-välimuistimerkinnän avaus

# Komentoapurin diagnostiikka.
stdlib.command.location = komento ”{ $command }” mallipohjassa ”{ $template }”
stdlib.command.spawn_failed = Kohdetta { $location } ei voitu käynnistää: { $details }.
stdlib.command.io_failed = { $location } epäonnistui: { $details }.
stdlib.command.closed_input_early = Syöte sulkeutui ennen kuin kirjoitus komennolle valmistui.
stdlib.command.broken_pipe = Katkennut putki suoritettaessa kohdetta { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } päättyi signaaliin.
stdlib.command.exited_with_status = { $location } päättyi tilaan { $status }.
stdlib.command.output_limit_exceeded = { $location } ylitti { $mode }-rajan { $limit } tavua virralle { $stream }.
stdlib.command.timeout = { $location } ylitti { $seconds } sekunnin aikarajan.
stdlib.command.exit_status_suffix = (päättymistila { $status })
stdlib.command.signal_suffix = (päättyi signaaliin)
stdlib.command.shell.empty = Komentotulkin komento ei saa olla tyhjä.
stdlib.command.grep.empty_pattern = grep-hahmo ei saa olla tyhjä.
stdlib.command.grep.flags_not_string = grep-valitsimien on oltava merkkijonoja.
stdlib.command.quote.invalid = Argumenttia { $arg } ei voitu lainausmerkitä: { $details }.
stdlib.command.quote.line_break = Argumentteja, joissa on vaunupalautus tai rivinvaihto, ei voi lainausmerkitä turvallisesti.
stdlib.command.input_undefined = Syötteen arvoa ei ole määritelty.
stdlib.command.tempfile.root_required = Komentojen väliaikaistiedostojen luonti vaatii työtilan juuren.
stdlib.command.tempfile.create_failed = Komennon väliaikaistiedostoa ei voitu luoda: { $details }.
stdlib.command.options.invalid_utf8 = Komennon asetusavaimen on oltava kelvollista UTF-8:aa.
stdlib.command.option.mode_not_string = Tulostetilan on oltava merkkijono.
stdlib.command.options.invalid_type = Komennon asetusten on oltava objekti.
stdlib.command.output.mode_unsupported = Tulostetilaa ”{ $mode }” ei tueta.
stdlib.command.output.mode.capture = talteenotto
stdlib.command.output.mode.streaming = virtaus
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Polkuapurin diagnostiikka.
stdlib.path.io.failed = { $action } epäonnistui polun { $path } käsittelyssä ({ $label }).
stdlib.path.io.failed_with_detail = { $action } epäonnistui polun { $path } käsittelyssä: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } epäonnistui polun { $path } käsittelyssä ({ $label }): { $detail }.
stdlib.path.io.not_found = ei löytynyt
stdlib.path.io.permission_denied = käyttö evätty
stdlib.path.io.already_exists = on jo olemassa
stdlib.path.io.invalid_input = virheellinen syöte
stdlib.path.io.invalid_data = virheelliset tiedot
stdlib.path.io.timed_out = aikakatkaisu
stdlib.path.io.interrupted = keskeytetty
stdlib.path.io.would_block = estäisi suorituksen
stdlib.path.io.write_zero = nolla tavua kirjoitettu
stdlib.path.io.unexpected_eof = odottamaton tiedoston loppu
stdlib.path.io.broken_pipe = katkennut putki
stdlib.path.io.connection_refused = yhteys torjuttiin
stdlib.path.io.connection_reset = yhteys nollattiin
stdlib.path.io.connection_aborted = yhteys keskeytettiin
stdlib.path.io.not_connected = ei yhteyttä
stdlib.path.io.addr_in_use = osoite on käytössä
stdlib.path.io.addr_not_available = osoite ei ole käytettävissä
stdlib.path.io.out_of_memory = muisti loppui
stdlib.path.io.unsupported = ei tuettu
stdlib.path.io.file_too_large = tiedosto on liian suuri
stdlib.path.io.resource_busy = resurssi on varattu
stdlib.path.io.executable_busy = ohjelmatiedosto on varattu
stdlib.path.io.deadlock = lukkiutuma
stdlib.path.io.crosses_devices = ylittää laiterajan
stdlib.path.io.too_many_links = liian monta linkkiä
stdlib.path.io.invalid_filename = virheellinen tiedostonimi
stdlib.path.io.arg_list_too_long = argumenttiluettelo on liian pitkä
stdlib.path.io.stale_handle = vanhentunut verkkotiedostokahva
stdlib.path.io.storage_full = tallennustila on täynnä
stdlib.path.io.not_seekable = ei tue kohdistusta
stdlib.path.io.network_down = verkko on alhaalla
stdlib.path.io.network_unreachable = verkkoa ei tavoiteta
stdlib.path.io.host_unreachable = isäntää ei tavoiteta
stdlib.path.io.other = siirräntävirhe
stdlib.path.action.canonicalize = kanonisointi
stdlib.path.action.open_directory = hakemiston avaus
stdlib.path.action.stat = tietojen haku
stdlib.path.action.read = luku
stdlib.path.action.open_file = tiedoston avaus
stdlib.path.with_suffix.empty_separator = with_suffix vaatii erottimen, joka ei ole tyhjä.
stdlib.path.relative_to.mismatch = { $path } ei ole suhteellinen polkuun { $root } nähden.
stdlib.path.expanduser.unsupported = Käyttäjäkohtaista ~-laajennusta ei tueta.
stdlib.path.expanduser.no_home = Merkkiä ~ ei voi laajentaa: kotihakemiston ympäristömuuttujia ei ole asetettu.
stdlib.path.contents.unsupported_encoding = Merkistökoodausta ”{ $encoding }” ei tueta.
stdlib.path.hash.unsupported_algorithm = Tiivistealgoritmia ”{ $algorithm }” ei tueta.
stdlib.path.hash.unsupported_algorithm_legacy = Tiivistealgoritmia ”{ $algorithm }” ei tueta (ota käyttöön ominaisuus ”{ $feature }”).

# Kokoelma-apurien diagnostiikka.
stdlib.collections.flatten.expected_sequence = flatten odotti jonon alkioita, mutta löysi { $kind }.
stdlib.collections.group_by.empty_attribute = group_by vaatii määritteen, joka ei ole tyhjä.
stdlib.collections.group_by.unresolved = group_by ei löytänyt määritettä ”{ $attr }” tyypin { $kind } alkiosta.

# Aika-apurien diagnostiikka.
stdlib.time.offset.invalid = now-siirtymä ”{ $offset }” on virheellinen: odotettiin muotoa ”+HH:MM[:SS]” tai ”Z”.
stdlib.time.timedelta.overflow = timedelta-ylivuoto lisättäessä komponenttia { $component }.
stdlib.time.label.weeks = viikkoa
stdlib.time.label.days = päivää
stdlib.time.label.hours = tuntia
stdlib.time.label.minutes = minuuttia
stdlib.time.label.seconds = sekuntia
stdlib.time.label.milliseconds = millisekuntia
stdlib.time.label.microseconds = mikrosekuntia
stdlib.time.label.nanoseconds = nanosekuntia

# which-apurin diagnostiikka.
stdlib.which.not_found = [netsuke::jinja::which::not_found] komentoa ”{ $command }” ei löytynyt, kun { $count } PATH-merkintää oli tarkistettu. Esikatselu: { $preview }
stdlib.which.not_found.hint.cwd_auto = PATH-muuttujan tyhjät osat ohitetaan; käytä cwd_mode="auto" sisällyttääksesi työhakemiston.
stdlib.which.not_found.hint.cwd_always = Aseta cwd_mode="always" sisällyttääksesi nykyisen hakemiston.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] komento ”{ $command }” polussa ”{ $path }” puuttuu tai ei ole suoritettava.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <tyhjä>
stdlib.which.path_entry.non_utf8 = PATH-merkintä nro { $index } sisältää merkkejä, jotka eivät ole UTF-8:aa; Netsuke vaatii UTF-8-polkuja.
stdlib.which.command.empty = which vaatii merkkijonon, joka ei ole tyhjä.
stdlib.which.cwd_mode.invalid = cwd_mode-arvon on oltava ”auto”, ”always” tai ”never”, mutta saatiin ”{ $mode }”.
stdlib.which.cwd.resolve_failed = Nykyistä hakemistoa ei voitu selvittää: { $details }.
stdlib.which.cwd.non_utf8 = Nykyinen hakemisto sisältää osia, jotka eivät ole UTF-8:aa.
stdlib.which.canonicalize_failed = Polkua ”{ $path }” ei voitu kanonisoida: { $details }.
stdlib.which.is_executable = Ei voitu selvittää, onko ”{ $path }” suoritettava: { $details }.
stdlib.which.canonicalize_non_utf8 = Kanoninen polku sisältää osia, jotka eivät ole UTF-8:aa.
stdlib.which.workspace_non_utf8 = Työtilan polku sisältää osia, jotka eivät ole UTF-8:aa, selvitettäessä komentoa ”{ $command }”: { $path }.
stdlib.which.walkdir_error = Virhe työtilan läpikäynnissä komentoa selvitettäessä: { $details }.

# Vakiokirjaston rekisteröinti.
stdlib.register.open_dir = Nykyistä hakemistoa ei voitu avata stdlib-rekisteröintiä varten.
stdlib.register.resolve_dir = Nykyistä hakemistoa ei voitu selvittää stdlib-rekisteröintiä varten.
stdlib.register.dir_non_utf8 = Nykyinen hakemisto sisältää osia, jotka eivät ole UTF-8:aa: { $path }.

# Tilaraportointi saavutettavassa tulostetilassa.
status.state.pending = odottaa
status.state.running = käynnissä
status.state.done = valmis
status.state.failed = epäonnistui
status.stage.label = Vaihe { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Tehtävä { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Luetaan manifestitiedostoa
status.stage.initial_yaml_parsing = Jäsennetään YAML-asiakirjaa
status.stage.template_expansion = Laajennetaan mallipohjadirektiivejä
status.stage.final_rendering = Puretaan sarjallistus ja hahmonnetaan manifestin arvot
status.stage.ir_generation_validation = Muodostetaan ja tarkistetaan riippuvuusgraafi
status.stage.ninja_synthesis = Muodostetaan Ninja-koontisuunnitelma
status.stage.ninja_synthesis_execute = Muodostetaan Ninja-suunnitelma ja suoritetaan { $tool }
status.stage.graph_rendering = Hahmonnetaan graafituotosta
status.stage.graph_rendering_with_tool = Hahmonnetaan { $tool }
status.complete = { $tool } valmis.
status.timing.summary_header = Vaiheiden ajoituskooste:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Putken kokonaisaika: { $duration }
status.tool.build = Koonti
status.tool.clean = Siivous
status.tool.graph = Graafi
status.tool.graph_html = Graafi (HTML)
status.tool.generate = Luonti
status.tool.help_targets = Kohdeohje

# Graafin HTML-hahmonnuksen tekstit.
graph.html.title = Netsuken koontigraafi
graph.html.heading = Netsuken koontigraafi
graph.html.description = Netsuken hahmontama koontigraafi
graph.html.outline.summary = Kohteet ja riippuvuudet (tekstijäsennys)
graph.html.outline.no_inputs = Ei syötteitä
graph.html.noscript.notice = JavaScript on poissa käytöstä. Yllä oleva tekstijäsennys sisältää koko graafin; DOT-lähde seuraa alla.

# Saavutettavan tulosteen semanttiset etuliitteet.
semantic.prefix.error = Virhe:
semantic.prefix.warning = Varoitus:
semantic.prefix.success = Onnistui:
semantic.prefix.info = Tiedoksi:
semantic.prefix.timing = Ajoitus:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Monikkomuotojen esimerkkejä kääntäjille.
# Suomi käyttää CLDR-luokkia `one` ja `other`. Luokassa `one` substantiivi on
# yksikön nominatiivissa (”1 tiedosto”), luokassa `other` yksikön
# partitiivissa (”5 tiedostoa”).
example.files_processed = { $count ->
    [one] Käsiteltiin { $count } tiedosto.
   *[other] Käsiteltiin { $count } tiedostoa.
}

example.errors_found = { $count ->
    [0] Virheitä ei löytynyt.
    [one] Löytyi { $count } virhe.
   *[other] Löytyi { $count } virhettä.
}
