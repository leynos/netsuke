# Ресурси локалізації командного рядка Netsuke.

cli.about = Netsuke компілює маніфести YAML + Jinja у плани збирання Ninja.
cli.long_about = Netsuke перетворює маніфести YAML + Jinja на відтворювані графи Ninja й запускає Ninja з безпечними типовими значеннями.
cli.usage = { $usage }

# Текст довідки для загальних параметрів.
cli.flag.file.help = Шлях до файлу маніфесту Netsuke, який слід використати.
cli.flag.directory.help = Виконати так, ніби запуск відбувся в цьому каталозі.
cli.flag.config.help = Шлях до файлу конфігурації в обхід автоматичного пошуку.
cli.flag.jobs.help = Задати кількість паралельних завдань збирання.
cli.flag.verbose.help = Увімкнути докладне діагностичне журналювання та підсумки часу після завершення.
cli.flag.locale.help = Мовна мітка для текстів командного рядка (наприклад: en-US, uk).
cli.flag.fetch_allow_scheme.help = Додаткові схеми URL, дозволені для помічника fetch.
cli.flag.fetch_allow_host.help = Назви вузлів, дозволені за увімкненої типової заборони.
cli.flag.fetch_block_host.help = Назви вузлів, які блокуються завжди, навіть якщо дозволені деінде.
cli.flag.fetch_default_deny.help = Типово забороняти всі вузли; дозволяти лише оголошений перелік.
cli.flag.json.help = Виводити машиночитний JSON.
cli.flag.no_input.help = Ніколи не читати інтерактивне введення.
cli.flag.color.help = Політика кольорового виводу (auto, always, never).
cli.flag.emoji.help = Політика використання емодзі (auto, always, never).
cli.flag.progress.help = Політика показу поступу (auto, always, never).
cli.flag.accessibility.help = Політика доступного виводу (auto, on, off).
cli.flag.default_targets.help = Типові цілі збирання, коли не вказано жодної.

# Описи підкоманд.
cli.subcommand.build.about = Зібрати цілі, визначені в маніфесті (типово).
cli.subcommand.build.long_about = Зібрати запитані цілі; якщо жодної не вказано, узяти типові цілі маніфесту.
cli.subcommand.clean.about = Видалити артефакти збирання засобами Ninja.
cli.subcommand.clean.long_about = Створити тимчасовий файл Ninja, а потім виконати `ninja -t clean`.
cli.subcommand.graph.about = Вивести граф залежностей збирання. Типовий формат — DOT.
cli.subcommand.graph.long_about = Перетворити розібраний маніфест Netsuke на канонічний граф збирання та записати його у форматі Graphviz DOT або, з параметром `--html`, як самостійну сторінку HTML. Використайте `--output <ФАЙЛ>`, щоб записати у файл; `-` виводить у стандартний потік.
cli.subcommand.generate.about = Створити маніфест Ninja, не запускаючи Ninja.
cli.subcommand.generate.long_about = Записати створений маніфест Ninja у стандартний потік виводу або у файл, вибраний параметром `--output`.
cli.subcommand.help.about = Друкувати довідку верхнього рівня або довідку для вказаної теми.
cli.subcommand.help.long_about = Без теми це відповідає `--help`. Використовуйте `help targets`, щоб надрукувати каталог цілей і дій для вибраного маніфесту.

# Help catalogue headings and markers.
cli.help.actions_heading = Дії:
cli.help.targets_heading = Цілі:
cli.help.targets.about = Вивести список цілей і дій у вибраному маніфесті.
cli.help.default_marker = за замовчуванням

# Текст довідки для параметрів підкоманди build.
cli.subcommand.build.flag.targets.help = Цілі для збирання (якщо не вказано, беруться типові цілі маніфесту).

# Текст довідки для параметрів підкоманди graph.
cli.subcommand.graph.flag.html.help = Показати граф як самостійну сторінку HTML замість формату DOT.
cli.subcommand.graph.flag.output.help = Записати артефакт графа у ФАЙЛ; для стандартного потоку використайте `-`.

# Текст довідки для параметрів підкоманди generate.
cli.subcommand.generate.flag.output.help = Записати створений маніфест Ninja у ФАЙЛ замість стандартного потоку виводу.

# Помилки перевірки в командному рядку.
cli.validation.jobs.invalid_number = { $value } не є припустимим числом.
cli.validation.jobs.out_of_range = Кількість завдань має бути в межах від { $min } до { $max }.
cli.validation.scheme.empty = Схема не повинна бути порожньою.
cli.validation.scheme.invalid_start = Схема «{ $scheme }» має починатися з літери ASCII.
cli.validation.scheme.invalid = Неприпустима схема «{ $scheme }».
cli.validation.locale.empty = Мовна мітка не повинна бути порожньою.
cli.validation.locale.invalid = Неприпустима мовна мітка «{ $locale }».
cli.validation.color.invalid = Неприпустима політика кольору «{ $value }». Припустимі значення: auto, always, never.
cli.validation.emoji.invalid = Неприпустима політика емодзі «{ $value }». Припустимі значення: auto, always, never.
cli.validation.progress.invalid = Неприпустима політика поступу «{ $value }». Припустимі значення: auto, always, never.
cli.validation.accessibility.invalid = Неприпустима політика доступності «{ $value }». Припустимі значення: auto, on, off.
cli.validation.config.expected_object = Значення командного рядка мали серіалізуватися в об’єкт, отримано { $value }.

# Повідомлення про помилки від Clap.
clap-error-missing-argument = Відсутній обов’язковий аргумент: { $argument }
clap-error-missing-subcommand = Відсутня підкоманда. Доступні варіанти: { $valid_subcommands }
clap-error-unknown-argument = Невідомий аргумент: { $argument }
clap-error-invalid-value = Неприпустиме значення для { $argument }: { $value }
clap-error-invalid-subcommand = Невідома підкоманда: { $subcommand }
# Примітка: формулювання value-validation відрізняється від invalid-value, щоб
# відрізняти помилки власних перевіряльників (ErrorKind::ValueValidation) від
# невідповідності типів (ErrorKind::InvalidValue).
clap-error-value-validation = Перевірку не пройдено для { $argument }: { $value }

# Помилки та контекст виконання.
runner.manifest.not_found = Маніфест «{ $manifest_name }» не знайдено в каталозі { $directory }.
runner.manifest.not_found.help = Переконайтеся, що маніфест існує, або вкажіть `--file` з правильним шляхом.
runner.manifest.path_missing_name = У шляху до маніфесту «{ $path }» немає імені файлу.
runner.manifest.path_utf8 = Шлях до маніфесту «{ $path }» не є коректним UTF-8.
runner.manifest.directory_utf8 = Шлях до каталогу маніфесту «{ $path }» не є коректним UTF-8.
runner.manifest.directory_label = каталог `{ $directory }`
runner.manifest.current_directory_label = поточний каталог
runner.manifest.default_not_declared = Типове значення маніфесту '{ $default }' не називає оголошену дію або ціль.
runner.context.network_policy = Не вдалося побудувати мережеву політику.
runner.context.load_manifest = Не вдалося завантажити маніфест за шляхом { $path }.
runner.context.serialise_manifest = Не вдалося серіалізувати маніфест.
runner.context.build_graph = Не вдалося побудувати граф за маніфестом.
runner.context.generate_ninja = Не вдалося створити маніфест Ninja.
runner.context.render_graph = Не вдалося відобразити артефакт графа.

runner.io.create_temp_file = Не вдалося створити тимчасовий файл Ninja.
runner.io.write_temp_ninja = Не вдалося записати тимчасовий файл Ninja.
runner.io.flush_temp_ninja = Не вдалося скинути буфер тимчасового файлу Ninja.
runner.io.sync_temp_ninja = Не вдалося синхронізувати тимчасовий файл Ninja.
runner.io.create_parent_dir = Не вдалося створити батьківський каталог { $path }.
runner.io.create_ninja_file = Не вдалося створити файл Ninja у { $path }.
runner.io.write_ninja_file = Не вдалося записати файл Ninja у { $path }.
runner.io.flush_ninja_file = Не вдалося скинути буфер файлу Ninja у { $path }.
runner.io.sync_ninja_file = Не вдалося синхронізувати файл Ninja у { $path }.
runner.io.open_ambient_dir = Не вдалося відкрити навколишній каталог.
runner.io.non_utf8_working_directory = Шлях робочого каталогу не є коректним UTF-8.
runner.io.no_existing_ancestor = Для { $path } не існує батьківського каталогу.
runner.io.derive_relative_path = Не вдалося вивести відносний шлях Ninja.
runner.io.non_utf8_path = Шляхи, відмінні від UTF-8, не підтримуються (шлях: { $path }).
runner.io.write_stdout = Не вдалося записати маніфест Ninja у стандартний потік виводу.
runner.io.flush_stdout = Не вдалося скинути буфер стандартного потоку виводу.
runner.io.dyndep.create_dir = Не вдалося створити каталог dyndep { $path }.
runner.io.dyndep.read = Не вдалося прочитати створений файл dyndep у { $path }.
runner.io.dyndep.write = Не вдалося записати створений файл dyndep у { $path }.
runner.io.dyndep.rename = Не вдалося завершити створений файл dyndep у { $path }.
runner.io.dyndep.corrupt = Створений файл dyndep у { $path } не відповідає очікуваному вмісту; видаліть лише цей файл і повторіть спробу.
runner.io.dyndep.race = Інший процес записав файл dyndep { $path }, але його вміст не вдалося перевірити.
runner.io.dyndep.temp_collisions = Не вдалося створити унікальний тимчасовий файл dyndep для { $path } після повторних колізій імен.

# Діагностика маніфесту.
manifest.parse = Не вдалося розібрати маніфест.
manifest.structure_error = Помилка структури маніфесту в { $name }: { $details }
manifest.yaml.parse = Помилка розбору YAML у рядку { $line }, стовпці { $column }: { $details }
manifest.yaml.label = некоректний YAML
manifest.yaml.hint.tabs = YAML не допускає табуляції; для відступів використовуйте пробіли.
manifest.yaml.hint.list_item = Елементи списку YAML мають починатися з «-» і мати правильний відступ.
manifest.yaml.hint.expected_colon = Схоже на елемент відображення; після ключа бракує «:».
manifest.yaml.hint.mapping_values = Відображення YAML потребують значення після «:» (або вкладеного блоку).
manifest.yaml.hint.invalid_token = Лексема YAML некоректна або несподівана.
manifest.yaml.hint.escape = Екрануйте зворотні скісні риски або вилучіть некоректні escape-послідовності.
manifest.env.missing = Обов’язкову змінну середовища не задано.
manifest.env.invalid_utf8 = Змінна середовища містить некоректний UTF-8.
manifest.vars.not_object = Поле `vars` маніфесту має бути відображенням або об’єктом.
manifest.vars.reserved_name = Ключ `vars` '{ $name }' у маніфесті зарезервовано для вбудованої допоміжної функції шаблонів; перейменуйте змінну.
manifest.read_failed = Не вдалося прочитати маніфест за шляхом { $path }.
manifest.resolve_workspace_root = Не вдалося визначити корінь робочої області.
manifest.workspace_non_utf8 = Кореневий шлях робочої області «{ $path }» не є коректним UTF-8.
manifest.path_non_utf8 = Шлях маніфесту «{ $manifest }» не є коректним UTF-8: { $path }.
manifest.path_missing_name = У шляху до маніфесту «{ $path }» немає імені файлу.
manifest.open_workspace_failed = Не вдалося відкрити робочу область { $workspace } для маніфесту { $manifest }.
manifest.foreach.not_iterable = Вираз `foreach` не є ітерованим.
manifest.foreach.serialise_item = Не вдалося серіалізувати елемент `foreach`.
manifest.when.empty = Вираз `when` не повинен бути порожнім.
manifest.when.eval_error = Не вдалося обчислити вираз `when` «{ $expr }».
manifest.when.template_error = Не вдалося відобразити шаблон `when` «{ $expr }».
manifest.target.vars_not_object = Поле `vars` цілі має бути об’єктом, отримано { $value }.
manifest.vars.entry_not_object = Запис `vars` маніфесту має бути об’єктом.
manifest.field_not_string = Поле «{ $field }» має бути рядком.
manifest.expression.parse_error = Не вдалося розібрати вираз { $name }.
manifest.expression.eval_error = Не вдалося обчислити вираз { $name }.

# Діагностика макросів маніфесту.
manifest.macro.signature_missing_identifier = У сигнатурі макроса відсутній ідентифікатор.
manifest.macro.signature_missing_params = У сигнатурі макроса відсутні параметри.
manifest.macro.compile_failed = Не вдалося скомпілювати макрос { $name }.
manifest.macro.sequence_invalid = Макроси мають задаватися як відображення імен на шаблони.
manifest.macro.register_failed = Не вдалося зареєструвати макроси маніфесту.
manifest.macro.not_initialised = Середовище макросів не ініціалізовано.
manifest.macro.caller_invalid = Викликач макроса має бути рядком.
manifest.macro.template_load_failed = Не вдалося завантажити шаблон макроса.
manifest.macro.init_failed = Не вдалося ініціалізувати середовище макросів.
manifest.macro.missing = Макрос { $name } відсутній.

# Помилки шаблонів glob у маніфесті.
manifest.glob.unmatched_brace = Некоректний шаблон glob «{ $pattern }»: «{ $character }» без пари в позиції { $position }.
manifest.glob.invalid_pattern = Некоректний шаблон glob «{ $pattern }»: { $detail }.
manifest.glob.unknown_pattern_error = невідома помилка шаблону.
manifest.glob.io_failed = Збій glob для «{ $pattern }»: { $detail }.
manifest.glob.unknown_io_error = невідома помилка вводу-виводу.
manifest.command_list_empty = Поле «command» не має бути порожнім: укажіть рядок команди або непорожній список.

# Помилки проміжного подання.
ir.rule_not_found = Правило «{ $rule }», на яке посилається ціль «{ $target }», не знайдено.
ir.multiple_rules = Ціль «{ $target }» має посилатися рівно на одне правило, отримано { $rules }.
ir.empty_rule = Ціль «{ $target }» має посилатися на правило.
ir.duplicate_outputs = Виявлено повторювані вихідні файли: { $outputs }.
ir.circular_dependency = Виявлено циклічну залежність: { $cycle }.
ir.action_serialisation = Не вдалося серіалізувати дію: { $details }.
ir.invalid_command = Некоректна підстановка в команді: { $snippet }.

# Помилки створення файлів Ninja.
ninja_gen.missing_action = Відсутня дія «{ $id }», на яку посилається ребро збирання.
ninja_gen.format = Не вдалося відформатувати вивід маніфесту Ninja.
ninja_gen.dyndep_files_required = Для цієї збірки потрібен створений пакет Ninja; використовуйте `netsuke build`, `netsuke clean` або `netsuke generate`, щоб матеріалізувати файли dyndep.
ninja_gen.reserved_output_path = Шлях '{ $path }' зарезервовано для стану послідовних залежностей Netsuke.
ninja_gen.unsupported_path_character = Шлях '{ $path }' містить непідтримуваний символ шляху Ninja: '{ $character }'.

# Перевірка шаблонів вузлів.
host_pattern.empty = Шаблон вузла не повинен бути порожнім.
host_pattern.contains_scheme = Шаблон вузла «{ $pattern }» не повинен містити схему URL.
host_pattern.contains_slash = Шаблон вузла «{ $pattern }» не повинен містити «/».
host_pattern.missing_suffix = Шаблон вузла «{ $pattern }» має містити суфікс після «*.».
host_pattern.empty_label = Шаблон вузла «{ $pattern }» містить порожню мітку.
host_pattern.invalid_chars = Шаблон вузла «{ $pattern }» містить неприпустимі символи.
host_pattern.invalid_label_edge = Мітки шаблону вузла «{ $pattern }» не повинні починатися чи закінчуватися символом «-».
host_pattern.label_too_long = Шаблон вузла «{ $pattern }» містить мітку, довшу за 63 символи.
host_pattern.too_long = Шаблон вузла «{ $pattern }» перевищує обмеження у 255 символів.

# Мережева політика.
network_policy.scheme.empty = Схема не повинна бути порожньою.
network_policy.scheme.invalid = Схема «{ $scheme }» містить неприпустимі символи.
network_policy.allowlist.empty = Перелік дозволених вузлів не повинен бути порожнім.
network_policy.scheme.not_allowed = Схема «{ $scheme }» не дозволена.
network_policy.missing_host = В URL відсутній вузол.
network_policy.host.blocked = Вузол «{ $host }» заблоковано політикою.
network_policy.host.not_allowlisted = Вузла «{ $host }» немає в переліку дозволених.

# Конфігурація стандартної бібліотеки.
stdlib.config.default_fetch_cache_invalid = Типовий шлях кешу fetch має бути відносним.
stdlib.config.default_which_cache_invalid = Типова місткість кешу which має бути додатною.
stdlib.config.workspace_root_absolute = Кореневий шлях робочої області має бути абсолютним.
stdlib.config.fetch_response_limit_positive = Обмеження на відповідь fetch має бути додатним.
stdlib.config.command_output_limit_positive = Обмеження на перехоплений вивід команд має бути додатним.
stdlib.config.command_stream_limit_positive = Обмеження на потік команд має бути додатним.
stdlib.config.which_cache_capacity_positive = Місткість кешу which має бути додатною.
stdlib.config.skip_dir_empty = Записи пропущених каталогів не повинні бути порожніми.
stdlib.config.skip_dir_navigation = Записи пропущених каталогів не повинні містити «..».
stdlib.config.skip_dir_separator = Записи пропущених каталогів не повинні містити роздільники шляху.
stdlib.config.fetch_cache_empty = Шлях кешу fetch не повинен бути порожнім.
stdlib.config.fetch_cache_not_relative = Шлях кешу fetch має бути відносним, отримано { $path }.
stdlib.config.fetch_cache_escapes = Шлях кешу fetch не повинен виходити за межі робочої області: { $path }.
stdlib.config.open_workspace_root = Не вдалося відкрити поточний каталог як корінь робочої області stdlib.
stdlib.config.resolve_cwd = Не вдалося визначити поточний каталог як корінь робочої області stdlib.
stdlib.config.cwd_non_utf8 = Поточний каталог містить частини, які не є UTF-8: { $path }.

# Діагностика помічника fetch.
stdlib.fetch.url_invalid = Некоректний URL «{ $url }»: { $details }.
stdlib.fetch.disallowed = URL «{ $url }» не дозволено: { $details }.
stdlib.fetch.failed = Не вдалося завантажити «{ $url }»: { $details }.
stdlib.fetch.cache_read_failed = Не вдалося прочитати запис кешу «{ $name }»: { $details }.
stdlib.fetch.cache_open_failed = Не вдалося відкрити запис кешу «{ $name }»: { $details }.
stdlib.fetch.response_read_failed = Не вдалося прочитати відповідь від «{ $url }»: { $details }.
stdlib.fetch.response_buffer_overflow = Переповнення буфера під час читання «{ $url }».
stdlib.fetch.cache_write_failed = Не вдалося записати кеш для «{ $url }»: { $details }.
stdlib.fetch.response_limit_exceeded = Відповідь від «{ $url }» перевищила обмеження у { $limit } байтів.
stdlib.fetch.cache_limit_exceeded = Кешована відповідь «{ $name }» перевищила обмеження у { $limit } байтів.
stdlib.fetch.io_failed = Не вдалося виконати дію «{ $action }» для { $path }: { $details }.
stdlib.fetch.action.sync_cache = синхронізація кешу fetch
stdlib.fetch.action.create_cache_dir = створення каталогу кешу fetch
stdlib.fetch.action.open_cache_dir = відкриття каталогу кешу fetch
stdlib.fetch.action.stat_cache = отримання відомостей про запис кешу fetch
stdlib.fetch.action.open_cache_entry = відкриття запису кешу fetch

# Діагностика помічника для команд.
stdlib.command.location = команда «{ $command }» у шаблоні «{ $template }»
stdlib.command.spawn_failed = { $location } не запустилася: { $details }.
stdlib.command.io_failed = { $location } зазнала збою: { $details }.
stdlib.command.closed_input_early = Введення закрилося до завершення запису в команду.
stdlib.command.broken_pipe = Розірвано канал, поки виконувалася { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } завершена сигналом.
stdlib.command.exited_with_status = { $location } завершилася з кодом { $status }.
stdlib.command.output_limit_exceeded = { $location } перевищила обмеження режиму «{ $mode }» у { $limit } байтів для { $stream }.
stdlib.command.timeout = { $location } перевищила граничний час у { $seconds } с.
stdlib.command.exit_status_suffix = (код завершення { $status })
stdlib.command.signal_suffix = (завершено сигналом)
stdlib.command.shell.empty = Команда оболонки не повинна бути порожньою.
stdlib.command.grep.empty_pattern = Шаблон grep не повинен бути порожнім.
stdlib.command.grep.flags_not_string = Прапорці grep мають бути рядками.
stdlib.command.quote.invalid = Не вдалося взяти { $arg } у лапки: { $details }.
stdlib.command.quote.line_break = Аргументи з поверненням каретки чи переведенням рядка не можна безпечно взяти в лапки.
stdlib.command.input_undefined = Вхідне значення не визначено.
stdlib.command.tempfile.root_required = Для створення тимчасових файлів команд потрібен корінь робочої області.
stdlib.command.tempfile.create_failed = Не вдалося створити тимчасовий файл команди: { $details }.
stdlib.command.options.invalid_utf8 = Ключ параметра команди має бути коректним UTF-8.
stdlib.command.option.mode_not_string = Режим виводу має бути рядком.
stdlib.command.options.invalid_type = Параметри команди мають бути об’єктом.
stdlib.command.output.mode_unsupported = Непідтримуваний режим виводу «{ $mode }».
stdlib.command.output.mode.capture = перехоплення
stdlib.command.output.mode.streaming = потокова передача
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Діагностика помічника для шляхів.
stdlib.path.io.failed = Не вдалося виконати дію «{ $action }» для { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Не вдалося виконати дію «{ $action }» для { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Не вдалося виконати дію «{ $action }» для { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = не знайдено
stdlib.path.io.permission_denied = доступ заборонено
stdlib.path.io.already_exists = вже існує
stdlib.path.io.invalid_input = некоректне введення
stdlib.path.io.invalid_data = некоректні дані
stdlib.path.io.timed_out = час очікування минув
stdlib.path.io.interrupted = перервано
stdlib.path.io.would_block = призвело б до блокування
stdlib.path.io.write_zero = записано нуль байтів
stdlib.path.io.unexpected_eof = несподіваний кінець файлу
stdlib.path.io.broken_pipe = розірваний канал
stdlib.path.io.connection_refused = у з’єднанні відмовлено
stdlib.path.io.connection_reset = з’єднання скинуто
stdlib.path.io.connection_aborted = з’єднання перервано
stdlib.path.io.not_connected = немає з’єднання
stdlib.path.io.addr_in_use = адреса вже використовується
stdlib.path.io.addr_not_available = адреса недоступна
stdlib.path.io.out_of_memory = бракує пам’яті
stdlib.path.io.unsupported = не підтримується
stdlib.path.io.file_too_large = файл завеликий
stdlib.path.io.resource_busy = ресурс зайнятий
stdlib.path.io.executable_busy = виконуваний файл зайнятий
stdlib.path.io.deadlock = взаємне блокування
stdlib.path.io.crosses_devices = перетинає межу пристроїв
stdlib.path.io.too_many_links = забагато посилань
stdlib.path.io.invalid_filename = некоректне ім’я файлу
stdlib.path.io.arg_list_too_long = завеликий перелік аргументів
stdlib.path.io.stale_handle = застарілий дескриптор мережевого файлу
stdlib.path.io.storage_full = сховище заповнено
stdlib.path.io.not_seekable = позиціювання недоступне
stdlib.path.io.network_down = мережа не працює
stdlib.path.io.network_unreachable = мережа недосяжна
stdlib.path.io.host_unreachable = вузол недосяжний
stdlib.path.io.other = помилка вводу-виводу
stdlib.path.action.canonicalize = канонізація
stdlib.path.action.open_directory = відкриття каталогу
stdlib.path.action.stat = отримання відомостей
stdlib.path.action.read = читання
stdlib.path.action.open_file = відкриття файлу
stdlib.path.with_suffix.empty_separator = with_suffix потребує непорожнього роздільника.
stdlib.path.relative_to.mismatch = { $path } не є відносним до { $root }.
stdlib.path.expanduser.unsupported = Розкриття ~ для конкретного користувача не підтримується.
stdlib.path.expanduser.no_home = Не вдається розкрити ~: не задано жодної змінної середовища домашнього каталогу.
stdlib.path.contents.unsupported_encoding = Непідтримуване кодування «{ $encoding }».
stdlib.path.hash.unsupported_algorithm = Непідтримуваний алгоритм хешування «{ $algorithm }».
stdlib.path.hash.unsupported_algorithm_legacy = Непідтримуваний алгоритм хешування «{ $algorithm }» (увімкніть можливість «{ $feature }»).

# Діагностика помічників для колекцій.
stdlib.collections.flatten.expected_sequence = flatten очікував елементи послідовності, але знайшов { $kind }.
stdlib.collections.group_by.empty_attribute = group_by потребує непорожнього атрибута.
stdlib.collections.group_by.unresolved = group_by не зміг знайти «{ $attr }» в елементі типу { $kind }.

# Діагностика помічників для часу.
stdlib.time.offset.invalid = Зсув now «{ $offset }» некоректний: очікувалося «+HH:MM[:SS]» або «Z».
stdlib.time.timedelta.overflow = Переповнення timedelta під час додавання компонента { $component }.
stdlib.time.label.weeks = тижні
stdlib.time.label.days = дні
stdlib.time.label.hours = години
stdlib.time.label.minutes = хвилини
stdlib.time.label.seconds = секунди
stdlib.time.label.milliseconds = мілісекунди
stdlib.time.label.microseconds = мікросекунди
stdlib.time.label.nanoseconds = наносекунди

# Діагностика помічника which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] команду «{ $command }» не знайдено після перевірки { $count } записів PATH. Попередній перегляд: { $preview }
stdlib.which.not_found.hint.cwd_auto = Порожні сегменти PATH ігноруються; задайте cwd_mode="auto", щоб урахувати робочий каталог.
stdlib.which.not_found.hint.cwd_always = Задайте cwd_mode="always", щоб урахувати поточний каталог.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] команда «{ $command }» за шляхом «{ $path }» відсутня або не є виконуваною.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <порожньо>
stdlib.which.path_entry.non_utf8 = Запис PATH № { $index } містить символи, які не є UTF-8; Netsuke потребує шляхів у UTF-8.
stdlib.which.command.empty = which потребує непорожнього рядка.
stdlib.which.cwd_mode.invalid = cwd_mode має бути «auto», «always» або «never», отримано «{ $mode }».
stdlib.which.cwd.resolve_failed = Не вдалося визначити поточний каталог: { $details }.
stdlib.which.cwd.non_utf8 = Поточний каталог містить частини, які не є UTF-8.
stdlib.which.canonicalize_failed = Не вдалося канонізувати «{ $path }»: { $details }.
stdlib.which.is_executable = Не вдалося перевірити, чи є «{ $path }» виконуваним: { $details }.
stdlib.which.canonicalize_non_utf8 = Канонічний шлях містить частини, які не є UTF-8.
stdlib.which.workspace_non_utf8 = Шлях робочої області містить частини, які не є UTF-8, під час пошуку команди «{ $command }»: { $path }.
stdlib.which.walkdir_error = Помилка обходу робочої області під час пошуку команди: { $details }.

# Реєстрація стандартної бібліотеки.
stdlib.register.open_dir = Не вдалося відкрити поточний каталог для реєстрації stdlib.
stdlib.register.resolve_dir = Не вдалося визначити поточний каталог для реєстрації stdlib.
stdlib.register.dir_non_utf8 = Поточний каталог містить частини, які не є UTF-8: { $path }.

# Звіт про стан у доступному режимі виводу.
status.state.pending = очікує
status.state.running = виконується
status.state.done = готово
status.state.failed = збій
status.stage.label = Етап { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Завдання { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Читання файлу маніфесту
status.stage.initial_yaml_parsing = Розбір документа YAML
status.stage.template_expansion = Розкриття директив шаблону
status.stage.final_rendering = Десеріалізація та відображення значень маніфесту
status.stage.ir_generation_validation = Побудова та перевірка графа залежностей
status.stage.ninja_synthesis = Побудова плану збирання Ninja
status.stage.ninja_synthesis_execute = Побудова плану Ninja та запуск { $tool }
status.stage.graph_rendering = Відображення артефакта графа
status.stage.graph_rendering_with_tool = Відображення { $tool }
status.complete = { $tool }: завершено.
status.timing.summary_header = Підсумок часу за етапами:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Загальний час конвеєра: { $duration }
status.tool.build = Збирання
status.tool.clean = Очищення
status.tool.graph = Граф
status.tool.graph_html = Граф (HTML)
status.tool.generate = Генерація
status.tool.help_targets = Довідка цілей

# Рядки HTML-подання графа.
graph.html.title = Граф збирання Netsuke
graph.html.heading = Граф збирання Netsuke
graph.html.description = Граф збирання, відображений Netsuke
graph.html.outline.summary = Цілі та залежності (текстова структура)
graph.html.outline.no_inputs = Немає вхідних даних
graph.html.noscript.notice = JavaScript вимкнено. Текстова структура вище містить увесь граф; нижче наведено вихідний код DOT.

# Семантичні префікси доступного виводу.
semantic.prefix.error = Помилка:
semantic.prefix.warning = Попередження:
semantic.prefix.success = Успішно:
semantic.prefix.info = Відомості:
semantic.prefix.timing = Час:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Приклади форм множини для перекладачів.
# Українська використовує категорії CLDR `one`, `few`, `many` та `other`.
# Цілі числа розподіляються так: `one` — 1, 21, 31…, `few` — 2–4, 22–24…,
# `many` — 0, 5–20, 25–30… Категорія `other` стосується дробових значень,
# тому вона ж є варіантом за замовчуванням.
example.files_processed = { $count ->
    [one] Оброблено { $count } файл.
    [few] Оброблено { $count } файли.
    [many] Оброблено { $count } файлів.
   *[other] Оброблено { $count } файла.
}

example.errors_found = { $count ->
    [0] Помилок не знайдено.
    [one] Знайдено { $count } помилку.
    [few] Знайдено { $count } помилки.
    [many] Знайдено { $count } помилок.
   *[other] Знайдено { $count } помилки.
}
