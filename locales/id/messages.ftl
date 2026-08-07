# Sumber daya pelokalan untuk antarmuka baris perintah Netsuke.

cli.about = Netsuke mengompilasi manifes YAML + Jinja menjadi rencana build Ninja.
cli.long_about = Netsuke mengubah manifes YAML + Jinja menjadi graf Ninja yang dapat direproduksi dan menjalankan Ninja dengan nilai bawaan yang aman.
cli.usage = { $usage }

# Teks bantuan untuk opsi umum.
cli.flag.file.help = Jalur berkas manifes Netsuke yang akan digunakan.
cli.flag.directory.help = Jalankan seolah-olah dimulai di direktori ini.
cli.flag.config.help = Jalur berkas konfigurasi, melewati pencarian otomatis.
cli.flag.jobs.help = Tetapkan jumlah tugas build paralel.
cli.flag.verbose.help = Aktifkan pencatatan diagnostik terperinci dan ringkasan waktu saat selesai.
cli.flag.locale.help = Tag bahasa untuk teks baris perintah (misalnya: en-US, id).
cli.flag.fetch_allow_scheme.help = Skema URL tambahan yang diizinkan bagi pembantu fetch.
cli.flag.fetch_allow_host.help = Nama host yang diizinkan ketika penolakan bawaan aktif.
cli.flag.fetch_block_host.help = Nama host yang selalu diblokir, meski diizinkan di tempat lain.
cli.flag.fetch_default_deny.help = Tolak semua host secara bawaan; izinkan hanya daftar yang dinyatakan.
cli.flag.json.help = Hasilkan keluaran JSON yang terbaca mesin.
cli.flag.no_input.help = Jangan pernah membaca masukan interaktif.
cli.flag.color.help = Kebijakan keluaran berwarna (auto, always, never).
cli.flag.emoji.help = Kebijakan emoji (auto, always, never).
cli.flag.progress.help = Kebijakan tampilan kemajuan (auto, always, never).
cli.flag.accessibility.help = Kebijakan keluaran yang mudah diakses (auto, on, off).
cli.flag.default_targets.help = Target build bawaan ketika tidak ada yang ditentukan.

# Deskripsi subperintah.
cli.subcommand.build.about = Bangun target yang ditetapkan dalam manifes (bawaan).
cli.subcommand.build.long_about = Bangun target yang diminta; bila tidak ada, gunakan target bawaan dari manifes.
cli.subcommand.clean.about = Hapus artefak build melalui Ninja.
cli.subcommand.clean.long_about = Hasilkan berkas Ninja sementara, lalu jalankan `ninja -t clean`.
cli.subcommand.graph.about = Keluarkan graf ketergantungan build. Format bawaannya adalah DOT.
cli.subcommand.graph.long_about = Proyeksikan manifes Netsuke yang telah diurai menjadi graf build kanonis dan tulis sebagai Graphviz DOT, atau sebagai halaman HTML mandiri dengan `--html`. Gunakan `--output <BERKAS>` untuk menulis ke berkas; `-` menulis ke keluaran standar.
cli.subcommand.generate.about = Hasilkan manifes Ninja tanpa menjalankan Ninja.
cli.subcommand.generate.long_about = Tulis manifes Ninja yang dihasilkan ke keluaran standar atau ke berkas yang dipilih dengan `--output`.

# Teks bantuan untuk opsi subperintah build.
cli.subcommand.build.flag.targets.help = Target yang akan dibangun (jika dihilangkan, memakai bawaan dari manifes).

# Teks bantuan untuk opsi subperintah graph.
cli.subcommand.graph.flag.html.help = Render graf sebagai halaman HTML mandiri alih-alih DOT.
cli.subcommand.graph.flag.output.help = Tulis artefak graf ke BERKAS; gunakan `-` untuk keluaran standar.

# Teks bantuan untuk opsi subperintah generate.
cli.subcommand.generate.flag.output.help = Tulis manifes Ninja yang dihasilkan ke BERKAS alih-alih keluaran standar.

# Galat validasi baris perintah.
cli.validation.jobs.invalid_number = { $value } bukan angka yang sah.
cli.validation.jobs.out_of_range = Jumlah tugas harus berada di antara { $min } dan { $max }.
cli.validation.scheme.empty = Skema tidak boleh kosong.
cli.validation.scheme.invalid_start = Skema "{ $scheme }" harus diawali huruf ASCII.
cli.validation.scheme.invalid = Skema tidak sah: "{ $scheme }".
cli.validation.locale.empty = Tag bahasa tidak boleh kosong.
cli.validation.locale.invalid = Tag bahasa tidak sah: "{ $locale }".
cli.validation.color.invalid = Kebijakan warna tidak sah: "{ $value }". Pilihan yang sah: auto, always, never.
cli.validation.emoji.invalid = Kebijakan emoji tidak sah: "{ $value }". Pilihan yang sah: auto, always, never.
cli.validation.progress.invalid = Kebijakan kemajuan tidak sah: "{ $value }". Pilihan yang sah: auto, always, never.
cli.validation.accessibility.invalid = Kebijakan aksesibilitas tidak sah: "{ $value }". Pilihan yang sah: auto, on, off.
cli.validation.config.expected_object = Nilai baris perintah seharusnya diserialkan menjadi objek, tetapi diperoleh { $value }.

# Pesan galat dari Clap.
clap-error-missing-argument = Argumen wajib tidak ada: { $argument }
clap-error-missing-subcommand = Subperintah tidak ada. Pilihan yang tersedia: { $valid_subcommands }
clap-error-unknown-argument = Argumen tidak dikenal: { $argument }
clap-error-invalid-value = Nilai tidak sah untuk { $argument }: { $value }
clap-error-invalid-subcommand = Subperintah tidak dikenal: { $subcommand }
# Catatan: value-validation dirumuskan berbeda dari invalid-value agar galat
# validator khusus (ErrorKind::ValueValidation) terbedakan dari ketidakcocokan
# tipe (ErrorKind::InvalidValue).
clap-error-value-validation = Validasi gagal untuk { $argument }: { $value }

# Galat dan konteks saat berjalan.
runner.manifest.not_found = Manifes "{ $manifest_name }" tidak ditemukan di { $directory }.
runner.manifest.not_found.help = Pastikan manifes ada, atau berikan `--file` dengan jalur yang benar.
runner.manifest.path_missing_name = Jalur manifes "{ $path }" tidak memuat nama berkas.
runner.manifest.path_utf8 = Jalur manifes "{ $path }" bukan UTF-8 yang sah.
runner.manifest.directory_utf8 = Jalur direktori manifes "{ $path }" bukan UTF-8 yang sah.
runner.manifest.directory_label = direktori `{ $directory }`
runner.manifest.current_directory_label = direktori saat ini
runner.context.network_policy = Kebijakan jaringan tidak dapat dibangun.
runner.context.load_manifest = Manifes di { $path } tidak dapat dimuat.
runner.context.serialise_manifest = Manifes tidak dapat diserialkan.
runner.context.build_graph = Graf tidak dapat dibangun dari manifes.
runner.context.generate_ninja = Manifes Ninja tidak dapat dihasilkan.
runner.context.render_graph = Artefak graf tidak dapat dirender.

runner.io.create_temp_file = Berkas Ninja sementara tidak dapat dibuat.
runner.io.write_temp_ninja = Berkas Ninja sementara tidak dapat ditulis.
runner.io.flush_temp_ninja = Penyangga berkas Ninja sementara tidak dapat dikosongkan.
runner.io.sync_temp_ninja = Berkas Ninja sementara tidak dapat disinkronkan.
runner.io.create_parent_dir = Direktori induk { $path } tidak dapat dibuat.
runner.io.create_ninja_file = Berkas Ninja di { $path } tidak dapat dibuat.
runner.io.write_ninja_file = Berkas Ninja di { $path } tidak dapat ditulis.
runner.io.flush_ninja_file = Penyangga berkas Ninja di { $path } tidak dapat dikosongkan.
runner.io.sync_ninja_file = Berkas Ninja di { $path } tidak dapat disinkronkan.
runner.io.open_ambient_dir = Direktori sekitar tidak dapat dibuka.
runner.io.no_existing_ancestor = Tidak ada direktori induk yang ada untuk { $path }.
runner.io.derive_relative_path = Jalur Ninja relatif tidak dapat diturunkan.
runner.io.non_utf8_path = Jalur yang bukan UTF-8 tidak didukung (jalur: { $path }).
runner.io.write_stdout = Manifes Ninja tidak dapat ditulis ke keluaran standar.
runner.io.flush_stdout = Penyangga keluaran standar tidak dapat dikosongkan.

# Diagnostik manifes.
manifest.parse = Penguraian manifes gagal.
manifest.structure_error = Galat struktur manifes pada { $name }: { $details }
manifest.yaml.parse = Galat penguraian YAML pada baris { $line }, kolom { $column }: { $details }
manifest.yaml.label = YAML tidak sah
manifest.yaml.hint.tabs = YAML tidak mengizinkan tab; gunakan spasi untuk indentasi.
manifest.yaml.hint.list_item = Butir daftar YAML harus diawali "-" dan diindentasi dengan benar.
manifest.yaml.hint.expected_colon = Ini tampak seperti entri pemetaan; ":" hilang setelah kunci.
manifest.yaml.hint.mapping_values = Pemetaan YAML memerlukan nilai setelah ":" (atau blok bersarang).
manifest.yaml.hint.invalid_token = Token YAML tidak sah atau tidak terduga.
manifest.yaml.hint.escape = Lakukan escape pada garis miring terbalik atau hapus urutan pelolosan yang tidak sah.
manifest.env.missing = Variabel lingkungan wajib belum disetel.
manifest.env.invalid_utf8 = Variabel lingkungan memuat UTF-8 yang tidak sah.
manifest.vars.not_object = `vars` pada manifes harus berupa pemetaan atau objek.
manifest.vars.reserved_name = Kunci `vars` '{ $name }' pada manifes dicadangkan untuk fungsi bantu templat bawaan; ganti nama variabel tersebut.
manifest.read_failed = Manifes di { $path } tidak dapat dibaca.
manifest.resolve_workspace_root = Akar ruang kerja tidak dapat ditentukan.
manifest.workspace_non_utf8 = Jalur akar ruang kerja "{ $path }" bukan UTF-8 yang sah.
manifest.path_non_utf8 = Jalur manifes "{ $manifest }" bukan UTF-8 yang sah: { $path }.
manifest.path_missing_name = Jalur manifes "{ $path }" tidak memuat nama berkas.
manifest.open_workspace_failed = Ruang kerja { $workspace } tidak dapat dibuka untuk manifes { $manifest }.
manifest.foreach.not_iterable = Ekspresi `foreach` tidak dapat diiterasi.
manifest.foreach.serialise_item = Butir `foreach` tidak dapat diserialkan.
manifest.when.empty = Ekspresi `when` tidak boleh kosong.
manifest.when.eval_error = Ekspresi `when` "{ $expr }" tidak dapat dievaluasi.
manifest.when.template_error = Templat `when` "{ $expr }" tidak dapat dirender.
manifest.target.vars_not_object = `vars` pada target harus berupa objek, tetapi diperoleh { $value }.
manifest.vars.entry_not_object = Entri `vars` pada manifes harus berupa objek.
manifest.field_not_string = Ruas "{ $field }" harus berupa untai.
manifest.expression.parse_error = Ekspresi { $name } tidak dapat diurai.
manifest.expression.eval_error = Ekspresi { $name } tidak dapat dievaluasi.

# Diagnostik makro manifes.
manifest.macro.signature_missing_identifier = Tanda tangan makro tidak memuat pengenal.
manifest.macro.signature_missing_params = Tanda tangan makro tidak memuat parameter.
manifest.macro.compile_failed = Makro { $name } tidak dapat dikompilasi.
manifest.macro.sequence_invalid = Makro harus ditetapkan sebagai pemetaan nama ke templat.
manifest.macro.register_failed = Makro manifes tidak dapat didaftarkan.
manifest.macro.not_initialised = Lingkungan makro belum disiapkan.
manifest.macro.caller_invalid = Pemanggil makro harus berupa untai.
manifest.macro.template_load_failed = Templat makro tidak dapat dimuat.
manifest.macro.init_failed = Lingkungan makro tidak dapat disiapkan.
manifest.macro.missing = Makro { $name } tidak ada.

# Galat pola glob pada manifes.
manifest.glob.unmatched_brace = Pola glob tidak sah "{ $pattern }": "{ $character }" tanpa pasangan pada posisi { $position }.
manifest.glob.invalid_pattern = Pola glob tidak sah "{ $pattern }": { $detail }.
manifest.glob.unknown_pattern_error = galat pola yang tidak dikenal.
manifest.glob.io_failed = Glob gagal untuk "{ $pattern }": { $detail }.
manifest.glob.unknown_io_error = galat masukan/keluaran yang tidak dikenal.

# Galat representasi antara.
ir.rule_not_found = Aturan "{ $rule }" yang dirujuk target "{ $target }" tidak ditemukan.
ir.multiple_rules = Target "{ $target }" harus merujuk tepat satu aturan, tetapi diperoleh { $rules }.
ir.empty_rule = Target "{ $target }" harus merujuk sebuah aturan.
ir.duplicate_outputs = Terdeteksi keluaran ganda: { $outputs }.
ir.circular_dependency = Terdeteksi ketergantungan melingkar: { $cycle }.
ir.action_serialisation = Tindakan tidak dapat diserialkan: { $details }.
ir.invalid_command = Penyisipan tidak sah pada perintah: { $snippet }.

# Galat pembuatan berkas Ninja.
ninja_gen.missing_action = Tindakan "{ $id }" yang dirujuk sebuah sisi build tidak ada.
ninja_gen.format = Keluaran manifes Ninja tidak dapat diformat.

# Validasi pola host.
host_pattern.empty = Pola host tidak boleh kosong.
host_pattern.contains_scheme = Pola host "{ $pattern }" tidak boleh memuat skema URL.
host_pattern.contains_slash = Pola host "{ $pattern }" tidak boleh memuat "/".
host_pattern.missing_suffix = Pola host "{ $pattern }" harus memuat akhiran setelah "*.".
host_pattern.empty_label = Pola host "{ $pattern }" memuat label kosong.
host_pattern.invalid_chars = Pola host "{ $pattern }" memuat karakter yang tidak sah.
host_pattern.invalid_label_edge = Label pada pola host "{ $pattern }" tidak boleh diawali atau diakhiri "-".
host_pattern.label_too_long = Pola host "{ $pattern }" memuat label yang lebih panjang dari 63 karakter.
host_pattern.too_long = Pola host "{ $pattern }" melampaui batas 255 karakter.

# Kebijakan jaringan.
network_policy.scheme.empty = Skema tidak boleh kosong.
network_policy.scheme.invalid = Skema "{ $scheme }" memuat karakter yang tidak sah.
network_policy.allowlist.empty = Daftar host yang diizinkan tidak boleh kosong.
network_policy.scheme.not_allowed = Skema "{ $scheme }" tidak diizinkan.
network_policy.missing_host = URL tidak memuat host.
network_policy.host.blocked = Host "{ $host }" diblokir oleh kebijakan.
network_policy.host.not_allowlisted = Host "{ $host }" tidak ada dalam daftar yang diizinkan.

# Konfigurasi pustaka standar.
stdlib.config.default_fetch_cache_invalid = Jalur bawaan singgahan fetch harus relatif.
stdlib.config.default_which_cache_invalid = Kapasitas bawaan singgahan which harus positif.
stdlib.config.workspace_root_absolute = Jalur akar ruang kerja harus absolut.
stdlib.config.fetch_response_limit_positive = Batas tanggapan fetch harus positif.
stdlib.config.command_output_limit_positive = Batas penangkapan keluaran perintah harus positif.
stdlib.config.command_stream_limit_positive = Batas aliran perintah harus positif.
stdlib.config.which_cache_capacity_positive = Kapasitas singgahan which harus positif.
stdlib.config.skip_dir_empty = Entri direktori yang dilewati tidak boleh kosong.
stdlib.config.skip_dir_navigation = Entri direktori yang dilewati tidak boleh memuat "..".
stdlib.config.skip_dir_separator = Entri direktori yang dilewati tidak boleh memuat pemisah jalur.
stdlib.config.fetch_cache_empty = Jalur singgahan fetch tidak boleh kosong.
stdlib.config.fetch_cache_not_relative = Jalur singgahan fetch harus relatif, tetapi diperoleh { $path }.
stdlib.config.fetch_cache_escapes = Jalur singgahan fetch tidak boleh keluar dari ruang kerja: { $path }.
stdlib.config.open_workspace_root = Direktori saat ini tidak dapat dibuka sebagai akar ruang kerja stdlib.
stdlib.config.resolve_cwd = Direktori saat ini tidak dapat ditentukan sebagai akar ruang kerja stdlib.
stdlib.config.cwd_non_utf8 = Direktori saat ini memuat bagian yang bukan UTF-8: { $path }.

# Diagnostik pembantu fetch.
stdlib.fetch.url_invalid = URL tidak sah "{ $url }": { $details }.
stdlib.fetch.disallowed = URL "{ $url }" tidak diizinkan: { $details }.
stdlib.fetch.failed = Gagal mengambil "{ $url }": { $details }.
stdlib.fetch.cache_read_failed = Entri singgahan "{ $name }" tidak dapat dibaca: { $details }.
stdlib.fetch.cache_open_failed = Entri singgahan "{ $name }" tidak dapat dibuka: { $details }.
stdlib.fetch.response_read_failed = Tanggapan dari "{ $url }" tidak dapat dibaca: { $details }.
stdlib.fetch.response_buffer_overflow = Penyangga meluap saat membaca "{ $url }".
stdlib.fetch.cache_write_failed = Singgahan untuk "{ $url }" tidak dapat ditulis: { $details }.
stdlib.fetch.response_limit_exceeded = Tanggapan dari "{ $url }" melampaui batas { $limit } bita.
stdlib.fetch.cache_limit_exceeded = Tanggapan tersinggah "{ $name }" melampaui batas { $limit } bita.
stdlib.fetch.io_failed = Tindakan "{ $action }" gagal untuk { $path }: { $details }.
stdlib.fetch.action.sync_cache = menyinkronkan singgahan fetch
stdlib.fetch.action.create_cache_dir = membuat direktori singgahan fetch
stdlib.fetch.action.open_cache_dir = membuka direktori singgahan fetch
stdlib.fetch.action.stat_cache = membaca keterangan entri singgahan fetch
stdlib.fetch.action.open_cache_entry = membuka entri singgahan fetch

# Diagnostik pembantu perintah.
stdlib.command.location = perintah "{ $command }" dalam templat "{ $template }"
stdlib.command.spawn_failed = { $location } tidak dapat dijalankan: { $details }.
stdlib.command.io_failed = { $location } gagal: { $details }.
stdlib.command.closed_input_early = Masukan tertutup sebelum penulisan ke perintah selesai.
stdlib.command.broken_pipe = Pipa terputus saat menjalankan { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } dihentikan oleh sinyal.
stdlib.command.exited_with_status = { $location } berakhir dengan status { $status }.
stdlib.command.output_limit_exceeded = { $location } melampaui batas { $mode } sebesar { $limit } bita untuk { $stream }.
stdlib.command.timeout = { $location } melampaui batas waktu { $seconds } detik.
stdlib.command.exit_status_suffix = (status keluar { $status })
stdlib.command.signal_suffix = (dihentikan oleh sinyal)
stdlib.command.shell.empty = Perintah shell tidak boleh kosong.
stdlib.command.grep.empty_pattern = Pola grep tidak boleh kosong.
stdlib.command.grep.flags_not_string = Bendera grep harus berupa untai.
stdlib.command.quote.invalid = { $arg } tidak dapat diberi tanda kutip: { $details }.
stdlib.command.quote.line_break = Argumen yang memuat retur kereta atau ganti baris tidak dapat diberi tanda kutip dengan aman.
stdlib.command.input_undefined = Nilai masukan tidak terdefinisi.
stdlib.command.tempfile.root_required = Akar ruang kerja diperlukan untuk membuat berkas perintah sementara.
stdlib.command.tempfile.create_failed = Berkas perintah sementara tidak dapat dibuat: { $details }.
stdlib.command.options.invalid_utf8 = Kunci opsi perintah harus berupa UTF-8 yang sah.
stdlib.command.option.mode_not_string = Mode keluaran harus berupa untai.
stdlib.command.options.invalid_type = Opsi perintah harus berupa objek.
stdlib.command.output.mode_unsupported = Mode keluaran tidak didukung: "{ $mode }".
stdlib.command.output.mode.capture = penangkapan
stdlib.command.output.mode.streaming = penstriman
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostik pembantu jalur.
stdlib.path.io.failed = Tindakan "{ $action }" gagal untuk { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Tindakan "{ $action }" gagal untuk { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Tindakan "{ $action }" gagal untuk { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = tidak ditemukan
stdlib.path.io.permission_denied = akses ditolak
stdlib.path.io.already_exists = sudah ada
stdlib.path.io.invalid_input = masukan tidak sah
stdlib.path.io.invalid_data = data tidak sah
stdlib.path.io.timed_out = waktu habis
stdlib.path.io.interrupted = terputus
stdlib.path.io.would_block = akan memblokir
stdlib.path.io.write_zero = nol bita tertulis
stdlib.path.io.unexpected_eof = akhir berkas tak terduga
stdlib.path.io.broken_pipe = pipa terputus
stdlib.path.io.connection_refused = koneksi ditolak
stdlib.path.io.connection_reset = koneksi disetel ulang
stdlib.path.io.connection_aborted = koneksi dibatalkan
stdlib.path.io.not_connected = tidak terhubung
stdlib.path.io.addr_in_use = alamat sedang dipakai
stdlib.path.io.addr_not_available = alamat tidak tersedia
stdlib.path.io.out_of_memory = memori habis
stdlib.path.io.unsupported = tidak didukung
stdlib.path.io.file_too_large = berkas terlalu besar
stdlib.path.io.resource_busy = sumber daya sibuk
stdlib.path.io.executable_busy = berkas eksekusi sibuk
stdlib.path.io.deadlock = kebuntuan
stdlib.path.io.crosses_devices = melintasi perangkat
stdlib.path.io.too_many_links = terlalu banyak tautan
stdlib.path.io.invalid_filename = nama berkas tidak sah
stdlib.path.io.arg_list_too_long = daftar argumen terlalu panjang
stdlib.path.io.stale_handle = tangkai berkas jaringan usang
stdlib.path.io.storage_full = penyimpanan penuh
stdlib.path.io.not_seekable = tidak dapat diposisikan
stdlib.path.io.network_down = jaringan mati
stdlib.path.io.network_unreachable = jaringan tak terjangkau
stdlib.path.io.host_unreachable = host tak terjangkau
stdlib.path.io.other = galat masukan/keluaran
stdlib.path.action.canonicalize = kanonikalisasi
stdlib.path.action.open_directory = membuka direktori
stdlib.path.action.stat = membaca keterangan
stdlib.path.action.read = membaca
stdlib.path.action.open_file = membuka berkas
stdlib.path.with_suffix.empty_separator = with_suffix memerlukan pemisah yang tidak kosong.
stdlib.path.relative_to.mismatch = { $path } tidak relatif terhadap { $root }.
stdlib.path.expanduser.unsupported = Ekspansi ~ untuk pengguna tertentu tidak didukung.
stdlib.path.expanduser.no_home = ~ tidak dapat diekspansi: tidak ada variabel lingkungan direktori beranda yang disetel.
stdlib.path.contents.unsupported_encoding = Pengodean tidak didukung: "{ $encoding }".
stdlib.path.hash.unsupported_algorithm = Algoritme hash tidak didukung: "{ $algorithm }".
stdlib.path.hash.unsupported_algorithm_legacy = Algoritme hash tidak didukung: "{ $algorithm }" (aktifkan fitur "{ $feature }").

# Diagnostik pembantu koleksi.
stdlib.collections.flatten.expected_sequence = flatten mengharapkan butir urutan tetapi menemukan { $kind }.
stdlib.collections.group_by.empty_attribute = group_by memerlukan atribut yang tidak kosong.
stdlib.collections.group_by.unresolved = group_by tidak dapat menemukan "{ $attr }" pada butir bertipe { $kind }.

# Diagnostik pembantu waktu.
stdlib.time.offset.invalid = Ofset now "{ $offset }" tidak sah: diharapkan "+HH:MM[:SS]" atau "Z".
stdlib.time.timedelta.overflow = timedelta meluap saat menambahkan { $component }.
stdlib.time.label.weeks = minggu
stdlib.time.label.days = hari
stdlib.time.label.hours = jam
stdlib.time.label.minutes = menit
stdlib.time.label.seconds = detik
stdlib.time.label.milliseconds = milidetik
stdlib.time.label.microseconds = mikrodetik
stdlib.time.label.nanoseconds = nanodetik

# Diagnostik pembantu which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] perintah "{ $command }" tidak ditemukan setelah memeriksa { $count } entri PATH. Pratinjau: { $preview }
stdlib.which.not_found.hint.cwd_auto = Ruas PATH yang kosong diabaikan; gunakan cwd_mode="auto" untuk menyertakan direktori kerja.
stdlib.which.not_found.hint.cwd_always = Setel cwd_mode="always" untuk menyertakan direktori saat ini.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] perintah "{ $command }" di "{ $path }" tidak ada atau tidak dapat dieksekusi.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <kosong>
stdlib.which.path_entry.non_utf8 = Entri PATH ke-{ $index } memuat karakter yang bukan UTF-8; Netsuke memerlukan jalur UTF-8.
stdlib.which.command.empty = which memerlukan untai yang tidak kosong.
stdlib.which.cwd_mode.invalid = cwd_mode harus "auto", "always", atau "never", tetapi diperoleh "{ $mode }".
stdlib.which.cwd.resolve_failed = Direktori saat ini tidak dapat ditentukan: { $details }.
stdlib.which.cwd.non_utf8 = Direktori saat ini memuat bagian yang bukan UTF-8.
stdlib.which.canonicalize_failed = "{ $path }" tidak dapat dikanonikalisasi: { $details }.
stdlib.which.is_executable = Tidak dapat memastikan apakah "{ $path }" dapat dieksekusi: { $details }.
stdlib.which.canonicalize_non_utf8 = Jalur kanonis memuat bagian yang bukan UTF-8.
stdlib.which.workspace_non_utf8 = Jalur ruang kerja memuat bagian yang bukan UTF-8 saat menyelesaikan perintah "{ $command }": { $path }.
stdlib.which.walkdir_error = Galat saat menelusuri ruang kerja ketika menyelesaikan perintah: { $details }.

# Pendaftaran pustaka standar.
stdlib.register.open_dir = Direktori saat ini tidak dapat dibuka untuk pendaftaran stdlib.
stdlib.register.resolve_dir = Direktori saat ini tidak dapat ditentukan untuk pendaftaran stdlib.
stdlib.register.dir_non_utf8 = Direktori saat ini memuat bagian yang bukan UTF-8: { $path }.

# Pelaporan status untuk mode keluaran yang mudah diakses.
status.state.pending = menunggu
status.state.running = sedang berjalan
status.state.done = selesai
status.state.failed = gagal
status.stage.label = Tahap { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Tugas { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Membaca berkas manifes
status.stage.initial_yaml_parsing = Mengurai dokumen YAML
status.stage.template_expansion = Mengembangkan arahan templat
status.stage.final_rendering = Mendeserialkan dan merender nilai manifes
status.stage.ir_generation_validation = Membangun dan memvalidasi graf ketergantungan
status.stage.ninja_synthesis = Menyusun rencana build Ninja
status.stage.ninja_synthesis_execute = Menyusun rencana Ninja dan menjalankan { $tool }
status.stage.graph_rendering = Merender artefak graf
status.stage.graph_rendering_with_tool = Merender { $tool }
status.complete = { $tool } selesai.
status.timing.summary_header = Ringkasan waktu per tahap:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Total waktu alur: { $duration }
status.tool.build = Build
status.tool.clean = Pembersihan
status.tool.graph = Graf
status.tool.graph_html = Graf (HTML)
status.tool.generate = Pembuatan

# Teks perender HTML untuk graf.
graph.html.title = Graf build Netsuke
graph.html.heading = Graf build Netsuke
graph.html.description = Graf build yang dirender oleh Netsuke
graph.html.outline.summary = Target dan ketergantungan (kerangka teks)
graph.html.outline.no_inputs = Tidak ada masukan
graph.html.noscript.notice = JavaScript dinonaktifkan. Kerangka teks di atas memuat seluruh graf; sumber DOT menyusul di bawah.

# Awalan semantik untuk keluaran yang mudah diakses.
semantic.prefix.error = Galat:
semantic.prefix.warning = Peringatan:
semantic.prefix.success = Berhasil:
semantic.prefix.info = Info:
semantic.prefix.timing = Waktu:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Contoh bentuk jamak untuk penerjemah.
# Bahasa Indonesia hanya memakai kategori CLDR `other`, karena bilangan tidak
# mengubah bentuk nomina.
example.files_processed = { $count ->
   *[other] { $count } berkas diproses.
}

example.errors_found = { $count ->
    [0] Tidak ada galat yang ditemukan.
   *[other] { $count } galat ditemukan.
}
