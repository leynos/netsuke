# Netsuke komut satırı için yerelleştirme kaynakları.

runner.io.dyndep.retention = { $path } altındaki oluşturulan dyndep dosyasının tutulması uygulanamadı.
cli.about = Netsuke, YAML + Jinja bildirimlerini Ninja derleme planlarına derler.
cli.long_about = Netsuke, YAML + Jinja bildirimlerini yeniden üretilebilir Ninja çizgelerine dönüştürür ve Ninja'yı güvenli varsayılanlarla çalıştırır.
cli.usage = { $usage }

# Genel seçeneklerin yardım metni.
cli.flag.file.help = Kullanılacak Netsuke bildirim dosyasının yolu.
cli.flag.directory.help = Bu dizinde başlatılmış gibi çalıştır.
cli.flag.config.help = Otomatik aramayı atlayarak kullanılacak yapılandırma dosyasının yolu.
cli.flag.jobs.help = Koşut derleme işlerinin sayısını belirle.
cli.flag.verbose.help = Ayrıntılı tanılama günlüğünü ve bitişteki süre özetlerini etkinleştir.
cli.flag.locale.help = Komut satırı metinleri için dil etiketi (örneğin: en-US, tr).
cli.flag.fetch_allow_scheme.help = fetch yardımcısının kullanabileceği ek URL şemaları.
cli.flag.fetch_allow_host.help = Varsayılan reddetme açıkken izin verilen makine adları.
cli.flag.fetch_block_host.help = Başka yerde izin verilse bile her zaman engellenen makine adları.
cli.flag.fetch_default_deny.help = Varsayılan olarak tüm makineleri reddet; yalnızca bildirilen listeye izin ver.
cli.flag.json.help = Makinece okunabilir JSON çıktısı üret.
cli.flag.no_input.help = Etkileşimli girdi hiçbir zaman okunmasın.
cli.flag.color.help = Renkli çıktı ilkesi (auto, always, never).
cli.flag.emoji.help = Emoji ilkesi (auto, always, never).
cli.flag.progress.help = İlerleme gösterimi ilkesi (auto, always, never).
cli.flag.accessibility.help = Erişilebilir çıktı ilkesi (auto, on, off).
cli.flag.default_targets.help = Hiçbiri belirtilmediğinde kullanılacak varsayılan derleme hedefleri.

# Alt komut açıklamaları.
cli.subcommand.build.about = Bildirimde tanımlı hedefleri derle (varsayılan).
cli.subcommand.build.long_about = İstenen hedefleri derle; hiçbiri verilmezse bildirimdeki varsayılan hedefleri kullan.
cli.subcommand.clean.about = Derleme ürünlerini Ninja aracılığıyla kaldır.
cli.subcommand.clean.long_about = Geçici bir Ninja dosyası oluştur, ardından `ninja -t clean` komutunu çalıştır.
cli.subcommand.graph.about = Derleme bağımlılık çizgesini yaz. Varsayılan biçim DOT'tur.
cli.subcommand.graph.long_about = Ayrıştırılan Netsuke bildirimini kurallı bir derleme çizgesine dönüştür ve Graphviz DOT olarak ya da `--html` ile kendi kendine yeten bir HTML sayfası olarak yaz. Dosyaya yazmak için `--output <DOSYA>` kullanın; `-` standart çıktıya yazar.
cli.subcommand.generate.about = Ninja'yı çalıştırmadan Ninja bildirimini üret.
cli.subcommand.generate.long_about = Üretilen Ninja bildirimini standart çıktıya ya da `--output` ile seçilen dosyaya yaz.
cli.subcommand.help.about = Üst düzey yardımı veya adlandırılmış bir konunun yardımını yazdır.
cli.subcommand.help.long_about = Konu olmadan bu, `--help` ile aynıdır. Seçilen bildirim için hedef ve eylem kataloğunu yazdırmak üzere `help targets` komutunu kullanın.

# Help catalogue headings and markers.
cli.help.actions_heading = Eylemler:
cli.help.targets_heading = Hedefler:
cli.help.targets.about = Seçilen bildirimdeki hedef ve eylemleri listele.
cli.help.default_marker = varsayılan

# build alt komutunun seçenekleri için yardım metni.
cli.subcommand.build.flag.targets.help = Derlenecek hedefler (belirtilmezse bildirimdeki varsayılanlar kullanılır).

# graph alt komutunun seçenekleri için yardım metni.
cli.subcommand.graph.flag.html.help = Çizgeyi DOT yerine kendi kendine yeten bir HTML sayfası olarak işle.
cli.subcommand.graph.flag.output.help = Çizge ürününü DOSYA'ya yaz; standart çıktı için `-` kullanın.

# generate alt komutunun seçenekleri için yardım metni.
cli.subcommand.generate.flag.output.help = Üretilen Ninja bildirimini standart çıktı yerine DOSYA'ya yaz.

# Komut satırı doğrulama hataları.
cli.validation.jobs.invalid_number = { $value } geçerli bir sayı değil.
cli.validation.jobs.out_of_range = İş sayısı { $min } ile { $max } arasında olmalıdır.
cli.validation.scheme.empty = Şema boş olmamalıdır.
cli.validation.scheme.invalid_start = "{ $scheme }" şeması bir ASCII harfiyle başlamalıdır.
cli.validation.scheme.invalid = Geçersiz şema: "{ $scheme }".
cli.validation.locale.empty = Dil etiketi boş olmamalıdır.
cli.validation.locale.invalid = Geçersiz dil etiketi: "{ $locale }".
cli.validation.color.invalid = Geçersiz renk ilkesi: "{ $value }". Geçerli seçenekler: auto, always, never.
cli.validation.emoji.invalid = Geçersiz emoji ilkesi: "{ $value }". Geçerli seçenekler: auto, always, never.
cli.validation.progress.invalid = Geçersiz ilerleme ilkesi: "{ $value }". Geçerli seçenekler: auto, always, never.
cli.validation.accessibility.invalid = Geçersiz erişilebilirlik ilkesi: "{ $value }". Geçerli seçenekler: auto, on, off.
cli.validation.config.expected_object = Komut satırı değerlerinin bir nesneye serileştirilmesi bekleniyordu, { $value } alındı.

# Clap hata iletileri.
clap-error-missing-argument = Zorunlu bağımsız değişken eksik: { $argument }
clap-error-missing-subcommand = Alt komut eksik. Kullanılabilir seçenekler: { $valid_subcommands }
clap-error-unknown-argument = Bilinmeyen bağımsız değişken: { $argument }
clap-error-invalid-value = { $argument } için geçersiz değer: { $value }
clap-error-invalid-subcommand = Bilinmeyen alt komut: { $subcommand }
# Not: value-validation, özel doğrulayıcı hatalarını
# (ErrorKind::ValueValidation) tür uyuşmazlıklarından
# (ErrorKind::InvalidValue) ayırmak için invalid-value'dan farklı yazılmıştır.
clap-error-value-validation = { $argument } için doğrulama başarısız: { $value }

# Çalıştırma hataları ve bağlamı.
runner.manifest.not_found = "{ $manifest_name }" bildirimi { $directory } içinde bulunamadı.
runner.manifest.not_found.help = Bildirimin var olduğundan emin olun ya da `--file` seçeneğini doğru yolla verin.
runner.manifest.path_missing_name = "{ $path }" bildirim yolunda dosya adı yok.
runner.manifest.path_utf8 = "{ $path }" bildirim yolu geçerli UTF-8 değil.
runner.manifest.directory_utf8 = "{ $path }" bildirim dizini yolu geçerli UTF-8 değil.
runner.manifest.directory_label = `{ $directory }` dizini
runner.manifest.current_directory_label = geçerli dizin
runner.manifest.default_not_declared = '{ $default }' bildirim varsayılanı, bildirilmiş bir eylem veya hedefi adlandırmıyor.
runner.context.network_policy = Ağ ilkesi oluşturulamadı.
runner.context.load_manifest = { $path } konumundaki bildirim yüklenemedi.
runner.context.serialise_manifest = Bildirim serileştirilemedi.
runner.context.build_graph = Bildirimden çizge oluşturulamadı.
runner.context.generate_ninja = Ninja bildirimi üretilemedi.
runner.context.render_graph = Çizge ürünü işlenemedi.

runner.io.create_temp_file = Geçici Ninja dosyası oluşturulamadı.
runner.io.write_temp_ninja = Geçici Ninja dosyası yazılamadı.
runner.io.flush_temp_ninja = Geçici Ninja dosyasının arabelleği boşaltılamadı.
runner.io.sync_temp_ninja = Geçici Ninja dosyası eşitlenemedi.
runner.io.create_parent_dir = { $path } üst dizini oluşturulamadı.
runner.io.create_ninja_file = { $path } konumunda Ninja dosyası oluşturulamadı.
runner.io.write_ninja_file = { $path } konumundaki Ninja dosyası yazılamadı.
runner.io.flush_ninja_file = { $path } konumundaki Ninja dosyasının arabelleği boşaltılamadı.
runner.io.sync_ninja_file = { $path } konumundaki Ninja dosyası eşitlenemedi.
runner.io.open_ambient_dir = Çevreleyen dizin açılamadı.
runner.io.non_utf8_working_directory = Çalışma dizini yolu geçerli UTF-8 değil.
runner.io.no_existing_ancestor = { $path } için var olan bir üst dizin yok.
runner.io.derive_relative_path = Göreli Ninja yolu türetilemedi.
runner.io.non_utf8_path = UTF-8 olmayan yollar desteklenmiyor (yol: { $path }).
runner.io.write_stdout = Ninja bildirimi standart çıktıya yazılamadı.
runner.io.flush_stdout = Standart çıktının arabelleği boşaltılamadı.
runner.io.dyndep.create_dir = dyndep dizini { $path } oluşturulamadı.
runner.io.dyndep.read = { $path } konumundaki oluşturulan dyndep dosyası okunamadı.
runner.io.dyndep.write = { $path } konumundaki oluşturulan dyndep dosyasına yazılamadı.
runner.io.dyndep.rename = { $path } konumundaki oluşturulan dyndep dosyası sonlandırılamadı.
runner.io.dyndep.corrupt = { $path } konumundaki oluşturulan dyndep dosyası beklenen içerikle eşleşmiyor; yalnızca bu dosyayı kaldırıp yeniden deneyin.
runner.io.dyndep.temp_collisions = Tekrarlanan ad çakışmalarından sonra { $path } için benzersiz bir geçici dyndep dosyası oluşturulamadı.
runner.io.dyndep.too_large = { $path } konumundaki oluşturulan dyndep dosyası { $limit } baytlık doğrulama sınırını aşıyor.

# Bildirim tanılaması.
manifest.parse = Bildirimin ayrıştırılması başarısız oldu.
manifest.structure_error = { $name } konumunda bildirim yapısı hatası: { $details }
manifest.yaml.parse = { $line }. satır, { $column }. sütunda YAML ayrıştırma hatası: { $details }
manifest.yaml.label = geçersiz YAML
manifest.yaml.hint.tabs = YAML sekmelere izin vermez; girinti için boşluk kullanın.
manifest.yaml.hint.list_item = YAML liste öğeleri "-" ile başlamalı ve düzgün girintilenmelidir.
manifest.yaml.hint.expected_colon = Bu bir eşleme girdisine benziyor; anahtardan sonra ":" eksik.
manifest.yaml.hint.mapping_values = YAML eşlemeleri ":" işaretinden sonra bir değer (ya da iç içe blok) ister.
manifest.yaml.hint.invalid_token = YAML belirteci geçersiz ya da beklenmedik.
manifest.yaml.hint.escape = Ters eğik çizgileri kaçırın ya da geçersiz kaçış dizilerini kaldırın.
manifest.env.missing = Gerekli bir ortam değişkeni ayarlanmamış.
manifest.env.invalid_utf8 = Bir ortam değişkeni geçersiz UTF-8 içeriyor.
manifest.vars.not_object = Bildirimin `vars` alanı bir eşleme ya da nesne olmalıdır.
manifest.vars.reserved_name = Manifestteki `vars` anahtarı '{ $name }' yerleşik bir şablon yardımcı işlevi için ayrılmıştır; değişkeni yeniden adlandırın.
manifest.read_failed = { $path } konumundaki bildirim okunamadı.
manifest.resolve_workspace_root = Çalışma alanının kökü belirlenemedi.
manifest.workspace_non_utf8 = "{ $path }" çalışma alanı kök yolu geçerli UTF-8 değil.
manifest.path_non_utf8 = "{ $manifest }" bildiriminin yolu geçerli UTF-8 değil: { $path }.
manifest.path_missing_name = "{ $path }" bildirim yolunda dosya adı yok.
manifest.open_workspace_failed = { $manifest } bildirimi için { $workspace } çalışma alanı açılamadı.
manifest.foreach.not_iterable = `foreach` ifadesi yinelenebilir değil.
manifest.foreach.serialise_item = `foreach` öğesi serileştirilemedi.
manifest.when.empty = `when` ifadesi boş olmamalıdır.
manifest.when.eval_error = "{ $expr }" `when` ifadesi değerlendirilemedi.
manifest.when.template_error = "{ $expr }" `when` şablonu işlenemedi.
manifest.target.vars_not_object = Hedefin `vars` alanı bir nesne olmalıdır, { $value } alındı.
manifest.vars.entry_not_object = Bildirimin `vars` girdisi bir nesne olmalıdır.
manifest.field_not_string = "{ $field }" alanı bir dizge olmalıdır.
manifest.expression.parse_error = { $name } ifadesi ayrıştırılamadı.
manifest.expression.eval_error = { $name } ifadesi değerlendirilemedi.

# Bildirim makrolarının tanılaması.
manifest.macro.signature_missing_identifier = Makro imzasında tanımlayıcı eksik.
manifest.macro.signature_missing_params = Makro imzasında parametreler eksik.
manifest.macro.compile_failed = { $name } makrosu derlenemedi.
manifest.macro.sequence_invalid = Makrolar, adların şablonlara eşlenmesi biçiminde tanımlanmalıdır.
manifest.macro.register_failed = Bildirimin makroları kaydedilemedi.
manifest.macro.not_initialised = Makro ortamı hazırlanmamış.
manifest.macro.caller_invalid = Makroyu çağıran bir dizge olmalıdır.
manifest.macro.template_load_failed = Makro şablonu yüklenemedi.
manifest.macro.init_failed = Makro ortamı hazırlanamadı.
manifest.macro.missing = { $name } makrosu eksik.

# Bildirimin glob deseni hataları.
manifest.glob.unmatched_brace = Geçersiz glob deseni "{ $pattern }": { $position }. konumdaki "{ $character }" eşleşmiyor.
manifest.glob.invalid_pattern = Geçersiz glob deseni "{ $pattern }": { $detail }.
manifest.glob.unknown_pattern_error = bilinmeyen desen hatası.
manifest.glob.io_failed = "{ $pattern }" için glob başarısız oldu: { $detail }.
manifest.glob.unknown_io_error = bilinmeyen G/Ç hatası.
manifest.command_list_empty = "command" alanı boş olmamalıdır: bir komut dizesi veya boş olmayan bir liste verin.

# Ara gösterim hataları.
ir.rule_not_found = "{ $target }" hedefinin başvurduğu "{ $rule }" kuralı bulunamadı.
ir.multiple_rules = "{ $target }" hedefi tek bir kurala başvurmalıdır, { $rules } alındı.
ir.empty_rule = "{ $target }" hedefi bir kurala başvurmalıdır.
ir.duplicate_outputs = Yinelenen çıktılar bulundu: { $outputs }.
ir.circular_dependency = Döngüsel bağımlılık bulundu: { $cycle }.
ir.action_serialisation = Eylem serileştirilemedi: { $details }.
ir.invalid_command = Komutta geçersiz yerleştirme: { $snippet }.

# Ninja üretimi hataları.
ninja_gen.missing_action = Bir derleme kenarının başvurduğu "{ $id }" eylemi eksik.
ninja_gen.format = Ninja bildiriminin çıktısı biçimlendirilemedi.
ninja_gen.dyndep_files_required = Bu derleme oluşturulmuş bir Ninja paketi gerektiriyor; dyndep dosyalarını oluşturmak için `netsuke build`, `netsuke clean` veya `netsuke generate` kullanın.
ninja_gen.reserved_output_path = '{ $path }' yolu Netsuke'nin seri bağımlılık durumu için ayrılmıştır.
ninja_gen.unsupported_path_character = '{ $path }' yolu, desteklenmeyen Ninja yol karakteri '{ $character }' içeriyor.

# Makine deseni doğrulaması.
host_pattern.empty = Makine deseni boş olmamalıdır.
host_pattern.contains_scheme = "{ $pattern }" makine deseni bir URL şeması içermemelidir.
host_pattern.contains_slash = "{ $pattern }" makine deseni "/" içermemelidir.
host_pattern.missing_suffix = "{ $pattern }" makine deseni "*." işaretinden sonra bir sonek içermelidir.
host_pattern.empty_label = "{ $pattern }" makine deseni boş bir etiket içeriyor.
host_pattern.invalid_chars = "{ $pattern }" makine deseni geçersiz karakterler içeriyor.
host_pattern.invalid_label_edge = "{ $pattern }" makine deseninin etiketleri "-" ile başlamamalı ya da bitmemelidir.
host_pattern.label_too_long = "{ $pattern }" makine deseni 63 karakterden uzun bir etiket içeriyor.
host_pattern.too_long = "{ $pattern }" makine deseni 255 karakter sınırını aşıyor.

# Ağ ilkesi.
network_policy.scheme.empty = Şema boş olmamalıdır.
network_policy.scheme.invalid = "{ $scheme }" şeması geçersiz karakterler içeriyor.
network_policy.allowlist.empty = İzin verilen makineler listesi boş olmamalıdır.
network_policy.scheme.not_allowed = "{ $scheme }" şemasına izin verilmiyor.
network_policy.missing_host = URL'de makine adı eksik.
network_policy.host.blocked = "{ $host }" makinesi ilke tarafından engellendi.
network_policy.host.not_allowlisted = "{ $host }" makinesi izin verilenler listesinde değil.

# Standart kitaplık yapılandırması.
stdlib.config.default_fetch_cache_invalid = Varsayılan fetch önbellek yolu göreli olmalıdır.
stdlib.config.default_which_cache_invalid = Varsayılan which önbellek kapasitesi pozitif olmalıdır.
stdlib.config.workspace_root_absolute = Çalışma alanının kök yolu mutlak olmalıdır.
stdlib.config.fetch_response_limit_positive = fetch yanıt sınırı pozitif olmalıdır.
stdlib.config.command_output_limit_positive = Komut çıktısı yakalama sınırı pozitif olmalıdır.
stdlib.config.command_stream_limit_positive = Komut akış sınırı pozitif olmalıdır.
stdlib.config.which_cache_capacity_positive = which önbellek kapasitesi pozitif olmalıdır.
stdlib.config.skip_dir_empty = Atlanacak dizin girdileri boş olmamalıdır.
stdlib.config.skip_dir_navigation = Atlanacak dizin girdileri ".." içermemelidir.
stdlib.config.skip_dir_separator = Atlanacak dizin girdileri yol ayırıcıları içermemelidir.
stdlib.config.fetch_cache_empty = fetch önbellek yolu boş olmamalıdır.
stdlib.config.fetch_cache_not_relative = fetch önbellek yolu göreli olmalıdır, { $path } alındı.
stdlib.config.fetch_cache_escapes = fetch önbellek yolu çalışma alanının dışına çıkmamalıdır: { $path }.
stdlib.config.open_workspace_root = Geçerli dizin, stdlib çalışma alanının kökü olarak açılamadı.
stdlib.config.resolve_cwd = Geçerli dizin, stdlib çalışma alanının kökü olarak belirlenemedi.
stdlib.config.cwd_non_utf8 = Geçerli dizin UTF-8 olmayan bölümler içeriyor: { $path }.

# fetch yardımcısının tanılaması.
stdlib.fetch.url_invalid = Geçersiz URL "{ $url }": { $details }.
stdlib.fetch.disallowed = "{ $url }" adresine izin verilmiyor: { $details }.
stdlib.fetch.failed = "{ $url }" adresinden veri alınamadı: { $details }.
stdlib.fetch.cache_read_failed = "{ $name }" önbellek girdisi okunamadı: { $details }.
stdlib.fetch.cache_open_failed = "{ $name }" önbellek girdisi açılamadı: { $details }.
stdlib.fetch.response_read_failed = "{ $url }" adresinden gelen yanıt okunamadı: { $details }.
stdlib.fetch.response_buffer_overflow = "{ $url }" okunurken arabellek taştı.
stdlib.fetch.cache_write_failed = "{ $url }" için önbellek yazılamadı: { $details }.
stdlib.fetch.response_limit_exceeded = "{ $url }" adresinden gelen yanıt { $limit } baytlık sınırı aştı.
stdlib.fetch.cache_limit_exceeded = Önbelleğe alınmış "{ $name }" yanıtı { $limit } baytlık sınırı aştı.
stdlib.fetch.io_failed = "{ $action }" eylemi { $path } için başarısız oldu: { $details }.
stdlib.fetch.action.sync_cache = fetch önbelleğini eşitleme
stdlib.fetch.action.create_cache_dir = fetch önbellek dizinini oluşturma
stdlib.fetch.action.open_cache_dir = fetch önbellek dizinini açma
stdlib.fetch.action.stat_cache = fetch önbellek girdisinin bilgilerini alma
stdlib.fetch.action.open_cache_entry = fetch önbellek girdisini açma

# Komut yardımcısının tanılaması.
stdlib.command.location = "{ $template }" şablonundaki "{ $command }" komutu
stdlib.command.spawn_failed = { $location } başlatılamadı: { $details }.
stdlib.command.io_failed = { $location } başarısız oldu: { $details }.
stdlib.command.closed_input_early = Komuta yazma tamamlanmadan girdi kapandı.
stdlib.command.broken_pipe = { $location } çalıştırılırken boru hattı koptu: { $details }.
stdlib.command.terminated_by_signal = { $location } bir sinyalle sonlandırıldı.
stdlib.command.exited_with_status = { $location } { $status } durumuyla sona erdi.
stdlib.command.output_limit_exceeded = { $location }, { $stream } için { $limit } baytlık { $mode } sınırını aştı.
stdlib.command.timeout = { $location }, { $seconds } saniyelik zaman sınırını aştı.
stdlib.command.exit_status_suffix = (çıkış durumu { $status })
stdlib.command.signal_suffix = (sinyalle sonlandırıldı)
stdlib.command.shell.empty = Kabuk komutu boş olmamalıdır.
stdlib.command.grep.empty_pattern = grep deseni boş olmamalıdır.
stdlib.command.grep.flags_not_string = grep bayrakları dizge olmalıdır.
stdlib.command.quote.invalid = { $arg } tırnak içine alınamadı: { $details }.
stdlib.command.quote.line_break = Satır başı ya da satır sonu karakteri içeren bağımsız değişkenler güvenle tırnak içine alınamaz.
stdlib.command.input_undefined = Girdi değeri tanımsız.
stdlib.command.tempfile.root_required = Geçici komut dosyaları oluşturmak için çalışma alanının kökü gereklidir.
stdlib.command.tempfile.create_failed = Geçici komut dosyası oluşturulamadı: { $details }.
stdlib.command.options.invalid_utf8 = Komut seçeneği anahtarı geçerli UTF-8 olmalıdır.
stdlib.command.option.mode_not_string = Çıktı kipi bir dizge olmalıdır.
stdlib.command.options.invalid_type = Komut seçenekleri bir nesne olmalıdır.
stdlib.command.output.mode_unsupported = Desteklenmeyen çıktı kipi: "{ $mode }".
stdlib.command.output.mode.capture = yakalama
stdlib.command.output.mode.streaming = akış
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Yol yardımcısının tanılaması.
stdlib.path.io.failed = "{ $action }" eylemi { $path } için başarısız oldu ({ $label }).
stdlib.path.io.failed_with_detail = "{ $action }" eylemi { $path } için başarısız oldu: { $detail }.
stdlib.path.io.failed_with_label_and_detail = "{ $action }" eylemi { $path } için başarısız oldu ({ $label }): { $detail }.
stdlib.path.io.not_found = bulunamadı
stdlib.path.io.permission_denied = erişim reddedildi
stdlib.path.io.already_exists = zaten var
stdlib.path.io.invalid_input = geçersiz girdi
stdlib.path.io.invalid_data = geçersiz veri
stdlib.path.io.timed_out = süre doldu
stdlib.path.io.interrupted = kesildi
stdlib.path.io.would_block = engellemeye yol açardı
stdlib.path.io.write_zero = sıfır bayt yazıldı
stdlib.path.io.unexpected_eof = beklenmedik dosya sonu
stdlib.path.io.broken_pipe = kopuk boru hattı
stdlib.path.io.connection_refused = bağlantı reddedildi
stdlib.path.io.connection_reset = bağlantı sıfırlandı
stdlib.path.io.connection_aborted = bağlantı kesildi
stdlib.path.io.not_connected = bağlantı yok
stdlib.path.io.addr_in_use = adres kullanımda
stdlib.path.io.addr_not_available = adres kullanılamıyor
stdlib.path.io.out_of_memory = bellek yetersiz
stdlib.path.io.unsupported = desteklenmiyor
stdlib.path.io.file_too_large = dosya çok büyük
stdlib.path.io.resource_busy = kaynak meşgul
stdlib.path.io.executable_busy = çalıştırılabilir dosya meşgul
stdlib.path.io.deadlock = ölümcül kilitlenme
stdlib.path.io.crosses_devices = aygıt sınırını aşıyor
stdlib.path.io.too_many_links = çok fazla bağlantı
stdlib.path.io.invalid_filename = geçersiz dosya adı
stdlib.path.io.arg_list_too_long = bağımsız değişken listesi çok uzun
stdlib.path.io.stale_handle = eskimiş ağ dosyası tanıtıcısı
stdlib.path.io.storage_full = depolama dolu
stdlib.path.io.not_seekable = konumlandırılamaz
stdlib.path.io.network_down = ağ çalışmıyor
stdlib.path.io.network_unreachable = ağa erişilemiyor
stdlib.path.io.host_unreachable = makineye erişilemiyor
stdlib.path.io.other = G/Ç hatası
stdlib.path.action.canonicalize = kurallı biçime çevirme
stdlib.path.action.open_directory = dizin açma
stdlib.path.action.stat = bilgi alma
stdlib.path.action.read = okuma
stdlib.path.action.open_file = dosya açma
stdlib.path.with_suffix.empty_separator = with_suffix boş olmayan bir ayırıcı gerektirir.
stdlib.path.relative_to.mismatch = { $path }, { $root } konumuna göreli değil.
stdlib.path.expanduser.unsupported = ~ işaretinin belirli bir kullanıcı için genişletilmesi desteklenmiyor.
stdlib.path.expanduser.no_home = ~ genişletilemiyor: ev dizinine ilişkin hiçbir ortam değişkeni ayarlı değil.
stdlib.path.contents.unsupported_encoding = Desteklenmeyen kodlama: "{ $encoding }".
stdlib.path.hash.unsupported_algorithm = Desteklenmeyen özet algoritması: "{ $algorithm }".
stdlib.path.hash.unsupported_algorithm_legacy = Desteklenmeyen özet algoritması: "{ $algorithm }" ("{ $feature }" özelliğini etkinleştirin).

# Koleksiyon yardımcılarının tanılaması.
stdlib.collections.flatten.expected_sequence = flatten dizi öğeleri bekliyordu, ancak { $kind } buldu.
stdlib.collections.group_by.empty_attribute = group_by boş olmayan bir öznitelik gerektirir.
stdlib.collections.group_by.unresolved = group_by, { $kind } türündeki bir öğede "{ $attr }" özniteliğini bulamadı.

# Zaman yardımcılarının tanılaması.
stdlib.time.offset.invalid = now kayması "{ $offset }" geçersiz: "+HH:MM[:SS]" ya da "Z" bekleniyordu.
stdlib.time.timedelta.overflow = { $component } eklenirken timedelta taştı.
stdlib.time.label.weeks = hafta
stdlib.time.label.days = gün
stdlib.time.label.hours = saat
stdlib.time.label.minutes = dakika
stdlib.time.label.seconds = saniye
stdlib.time.label.milliseconds = milisaniye
stdlib.time.label.microseconds = mikrosaniye
stdlib.time.label.nanoseconds = nanosaniye

# which yardımcısının tanılaması.
stdlib.which.not_found = [netsuke::jinja::which::not_found] { $count } PATH girdisi denetlendikten sonra "{ $command }" komutu bulunamadı. Önizleme: { $preview }
stdlib.which.not_found.hint.cwd_auto = PATH'in boş bölümleri yok sayılır; çalışma dizinini katmak için cwd_mode="auto" kullanın.
stdlib.which.not_found.hint.cwd_always = Geçerli dizini katmak için cwd_mode="always" ayarlayın.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] "{ $path }" konumundaki "{ $command }" komutu yok ya da çalıştırılabilir değil.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <boş>
stdlib.which.path_entry.non_utf8 = { $index }. PATH girdisi UTF-8 olmayan karakterler içeriyor; Netsuke UTF-8 yollar gerektirir.
stdlib.which.command.empty = which boş olmayan bir dizge gerektirir.
stdlib.which.cwd_mode.invalid = cwd_mode "auto", "always" ya da "never" olmalıdır, "{ $mode }" alındı.
stdlib.which.cwd.resolve_failed = Geçerli dizin belirlenemedi: { $details }.
stdlib.which.cwd.non_utf8 = Geçerli dizin UTF-8 olmayan bölümler içeriyor.
stdlib.which.canonicalize_failed = "{ $path }" kurallı biçime çevrilemedi: { $details }.
stdlib.which.is_executable = "{ $path }" öğesinin çalıştırılabilir olup olmadığı belirlenemedi: { $details }.
stdlib.which.canonicalize_non_utf8 = Kurallı yol UTF-8 olmayan bölümler içeriyor.
stdlib.which.workspace_non_utf8 = "{ $command }" komutu çözümlenirken çalışma alanı yolu UTF-8 olmayan bölümler içeriyor: { $path }.
stdlib.which.walkdir_error = Komut çözümlenirken çalışma alanı gezilirken hata oluştu: { $details }.

# Standart kitaplığın kaydı.
stdlib.register.open_dir = stdlib kaydı için geçerli dizin açılamadı.
stdlib.register.resolve_dir = stdlib kaydı için geçerli dizin belirlenemedi.
stdlib.register.dir_non_utf8 = Geçerli dizin UTF-8 olmayan bölümler içeriyor: { $path }.

# Erişilebilir çıktı kipinde durum bildirimi.
status.state.pending = bekliyor
status.state.running = sürüyor
status.state.done = bitti
status.state.failed = başarısız
status.stage.label = { $current }/{ $total }. aşama: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = { $current }/{ $total }. görev
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Bildirim dosyası okunuyor
status.stage.initial_yaml_parsing = YAML belgesi ayrıştırılıyor
status.stage.template_expansion = Şablon yönergeleri genişletiliyor
status.stage.final_rendering = Bildirim değerleri geri çözülüp işleniyor
status.stage.ir_generation_validation = Bağımlılık çizgesi oluşturuluyor ve doğrulanıyor
status.stage.ninja_synthesis = Ninja derleme planı hazırlanıyor
status.stage.ninja_synthesis_execute = Ninja planı hazırlanıyor ve { $tool } çalıştırılıyor
status.stage.graph_rendering = Çizge ürünü işleniyor
status.stage.graph_rendering_with_tool = { $tool } işleniyor
status.complete = { $tool } tamamlandı.
status.timing.summary_header = Aşamalara göre süre özeti:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Toplam işlem hattı süresi: { $duration }
status.tool.build = Derleme
status.tool.clean = Temizleme
status.tool.graph = Çizge
status.tool.graph_html = Çizge (HTML)
status.tool.generate = Üretme
status.tool.help_targets = Hedef yardımı

# Çizgenin HTML gösterimindeki metinler.
graph.html.title = Netsuke derleme çizgesi
graph.html.heading = Netsuke derleme çizgesi
graph.html.description = Netsuke tarafından işlenen derleme çizgesi
graph.html.outline.summary = Hedefler ve bağımlılıklar (metin taslağı)
graph.html.outline.no_inputs = Girdi yok
graph.html.noscript.notice = JavaScript kapalı. Yukarıdaki metin taslağı çizgenin tamamıdır; DOT kaynağı aşağıda yer alır.

# Erişilebilir çıktı için anlamsal önekler.
semantic.prefix.error = Hata:
semantic.prefix.warning = Uyarı:
semantic.prefix.success = Başarılı:
semantic.prefix.info = Bilgi:
semantic.prefix.timing = Süre:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Çevirmenler için çoğul biçim örnekleri.
# Türkçede sayıdan sonra ad tekil kalır, bu yüzden CLDR yalnızca `one` ve
# `other` kategorilerini kullanır ve her ikisi de aynı biçimi alır.
example.files_processed = { $count ->
    [one] { $count } dosya işlendi.
   *[other] { $count } dosya işlendi.
}

example.errors_found = { $count ->
    [0] Hata bulunmadı.
    [one] { $count } hata bulundu.
   *[other] { $count } hata bulundu.
}
