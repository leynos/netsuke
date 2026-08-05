# Ресурсы локализации командной строки Netsuke.

cli.about = Netsuke компилирует манифесты YAML + Jinja в планы сборки Ninja.
cli.long_about = Netsuke преобразует манифесты YAML + Jinja в воспроизводимые графы Ninja и запускает Ninja с безопасными значениями по умолчанию.
cli.usage = { $usage }

# Текст справки для общих параметров.
cli.flag.file.help = Путь к используемому файлу манифеста Netsuke.
cli.flag.directory.help = Выполнить так, как если бы запуск произошёл в этом каталоге.
cli.flag.config.help = Путь к файлу конфигурации в обход автоматического поиска.
cli.flag.jobs.help = Задать количество параллельных заданий сборки.
cli.flag.verbose.help = Включить подробное диагностическое журналирование и сводки по времени при завершении.
cli.flag.locale.help = Языковой тег для текстов командной строки (например: en-US, ru).
cli.flag.fetch_allow_scheme.help = Дополнительные схемы URL, разрешённые для помощника fetch.
cli.flag.fetch_allow_host.help = Имена узлов, разрешённые при включённом запрете по умолчанию.
cli.flag.fetch_block_host.help = Имена узлов, которые блокируются всегда, даже если разрешены в другом месте.
cli.flag.fetch_default_deny.help = Запрещать все узлы по умолчанию; разрешать только объявленный список.
cli.flag.json.help = Выводить машиночитаемый JSON.
cli.flag.no_input.help = Никогда не читать интерактивный ввод.
cli.flag.color.help = Политика цветного вывода (auto, always, never).
cli.flag.emoji.help = Политика использования эмодзи (auto, always, never).
cli.flag.progress.help = Политика отображения хода выполнения (auto, always, never).
cli.flag.accessibility.help = Политика доступного вывода (auto, on, off).
cli.flag.default_targets.help = Цели сборки по умолчанию, когда ни одна не указана.

# Описания подкоманд.
cli.subcommand.build.about = Собрать цели, объявленные в манифесте (по умолчанию).
cli.subcommand.build.long_about = Собрать запрошенные цели; если ни одна не указана, использовать цели манифеста по умолчанию.
cli.subcommand.clean.about = Удалить артефакты сборки средствами Ninja.
cli.subcommand.clean.long_about = Создать временный файл Ninja, затем выполнить `ninja -t clean`.
cli.subcommand.graph.about = Вывести граф зависимостей сборки. Формат по умолчанию — DOT.
cli.subcommand.graph.long_about = Преобразовать разобранный манифест Netsuke в канонический граф сборки и записать его в формате Graphviz DOT либо, с параметром `--html`, как самостоятельную HTML-страницу. Используйте `--output <ФАЙЛ>` для записи в файл; `-` выводит в стандартный поток.
cli.subcommand.generate.about = Создать манифест Ninja, не запуская Ninja.
cli.subcommand.generate.long_about = Записать созданный манифест Ninja в стандартный поток вывода либо в файл, выбранный параметром `--output`.

# Текст справки для параметров подкоманды build.
cli.subcommand.build.flag.targets.help = Цели для сборки (если не указаны, берутся цели манифеста по умолчанию).

# Текст справки для параметров подкоманды graph.
cli.subcommand.graph.flag.html.help = Отобразить граф как самостоятельную HTML-страницу вместо формата DOT.
cli.subcommand.graph.flag.output.help = Записать артефакт графа в ФАЙЛ; используйте `-` для стандартного потока вывода.

# Текст справки для параметров подкоманды generate.
cli.subcommand.generate.flag.output.help = Записать созданный манифест Ninja в ФАЙЛ вместо стандартного потока вывода.

# Ошибки проверки в командной строке.
cli.validation.jobs.invalid_number = { $value } не является допустимым числом.
cli.validation.jobs.out_of_range = Количество заданий должно быть в диапазоне от { $min } до { $max }.
cli.validation.scheme.empty = Схема не должна быть пустой.
cli.validation.scheme.invalid_start = Схема «{ $scheme }» должна начинаться с буквы ASCII.
cli.validation.scheme.invalid = Недопустимая схема «{ $scheme }».
cli.validation.locale.empty = Языковой тег не должен быть пустым.
cli.validation.locale.invalid = Недопустимый языковой тег «{ $locale }».
cli.validation.color.invalid = Недопустимая политика цвета «{ $value }». Допустимые значения: auto, always, never.
cli.validation.emoji.invalid = Недопустимая политика эмодзи «{ $value }». Допустимые значения: auto, always, never.
cli.validation.progress.invalid = Недопустимая политика хода выполнения «{ $value }». Допустимые значения: auto, always, never.
cli.validation.accessibility.invalid = Недопустимая политика доступности «{ $value }». Допустимые значения: auto, on, off.
cli.validation.config.expected_object = Значения командной строки должны были сериализоваться в объект, получено { $value }.

# Сообщения об ошибках Clap.
clap-error-missing-argument = Отсутствует обязательный аргумент: { $argument }
clap-error-missing-subcommand = Отсутствует подкоманда. Доступные варианты: { $valid_subcommands }
clap-error-unknown-argument = Неизвестный аргумент: { $argument }
clap-error-invalid-value = Недопустимое значение для { $argument }: { $value }
clap-error-invalid-subcommand = Неизвестная подкоманда: { $subcommand }
# Примечание: формулировка value-validation отличается от invalid-value, чтобы
# различать ошибки собственных проверяющих (ErrorKind::ValueValidation) и
# несовпадение типов (ErrorKind::InvalidValue).
clap-error-value-validation = Проверка не пройдена для { $argument }: { $value }

# Ошибки и контекст выполнения.
runner.manifest.not_found = Манифест «{ $manifest_name }» не найден в каталоге { $directory }.
runner.manifest.not_found.help = Убедитесь, что манифест существует, либо укажите `--file` с правильным путём.
runner.manifest.path_missing_name = В пути к манифесту «{ $path }» нет имени файла.
runner.manifest.path_utf8 = Путь к манифесту «{ $path }» не является корректным UTF-8.
runner.manifest.directory_utf8 = Путь к каталогу манифеста «{ $path }» не является корректным UTF-8.
runner.manifest.directory_label = каталог `{ $directory }`
runner.manifest.current_directory_label = текущий каталог
runner.context.network_policy = Не удалось построить сетевую политику.
runner.context.load_manifest = Не удалось загрузить манифест по пути { $path }.
runner.context.serialise_manifest = Не удалось сериализовать манифест.
runner.context.build_graph = Не удалось построить граф по манифесту.
runner.context.generate_ninja = Не удалось создать манифест Ninja.
runner.context.render_graph = Не удалось отобразить артефакт графа.

runner.io.create_temp_file = Не удалось создать временный файл Ninja.
runner.io.write_temp_ninja = Не удалось записать временный файл Ninja.
runner.io.flush_temp_ninja = Не удалось сбросить буфер временного файла Ninja.
runner.io.sync_temp_ninja = Не удалось синхронизировать временный файл Ninja.
runner.io.create_parent_dir = Не удалось создать родительский каталог { $path }.
runner.io.create_ninja_file = Не удалось создать файл Ninja в { $path }.
runner.io.write_ninja_file = Не удалось записать файл Ninja в { $path }.
runner.io.flush_ninja_file = Не удалось сбросить буфер файла Ninja в { $path }.
runner.io.sync_ninja_file = Не удалось синхронизировать файл Ninja в { $path }.
runner.io.open_ambient_dir = Не удалось открыть окружающий каталог.
runner.io.no_existing_ancestor = Для { $path } не существует родительского каталога.
runner.io.derive_relative_path = Не удалось вывести относительный путь Ninja.
runner.io.non_utf8_path = Пути, отличные от UTF-8, не поддерживаются (путь: { $path }).
runner.io.write_stdout = Не удалось записать манифест Ninja в стандартный поток вывода.
runner.io.flush_stdout = Не удалось сбросить буфер стандартного потока вывода.

# Диагностика манифеста.
manifest.parse = Не удалось разобрать манифест.
manifest.structure_error = Ошибка структуры манифеста в { $name }: { $details }
manifest.yaml.parse = Ошибка разбора YAML в строке { $line }, столбце { $column }: { $details }
manifest.yaml.label = некорректный YAML
manifest.yaml.hint.tabs = YAML не допускает табуляции; используйте для отступов пробелы.
manifest.yaml.hint.list_item = Элементы списка YAML должны начинаться с «-» и иметь правильный отступ.
manifest.yaml.hint.expected_colon = Похоже на элемент отображения; после ключа не хватает «:».
manifest.yaml.hint.mapping_values = Отображения YAML требуют значение после «:» (либо вложенный блок).
manifest.yaml.hint.invalid_token = Лексема YAML некорректна или неожиданна.
manifest.yaml.hint.escape = Экранируйте обратные косые черты либо удалите некорректные escape-последовательности.
manifest.env.missing = Обязательная переменная окружения не задана.
manifest.env.invalid_utf8 = Переменная окружения содержит некорректный UTF-8.
manifest.vars.not_object = Поле `vars` манифеста должно быть отображением или объектом.
manifest.read_failed = Не удалось прочитать манифест по пути { $path }.
manifest.resolve_workspace_root = Не удалось определить корень рабочего пространства.
manifest.workspace_non_utf8 = Корневой путь рабочего пространства «{ $path }» не является корректным UTF-8.
manifest.path_non_utf8 = Путь манифеста «{ $manifest }» не является корректным UTF-8: { $path }.
manifest.path_missing_name = В пути к манифесту «{ $path }» нет имени файла.
manifest.open_workspace_failed = Не удалось открыть рабочее пространство { $workspace } для манифеста { $manifest }.
manifest.foreach.not_iterable = Выражение `foreach` не является перебираемым.
manifest.foreach.serialise_item = Не удалось сериализовать элемент `foreach`.
manifest.when.empty = Выражение `when` не должно быть пустым.
manifest.when.eval_error = Не удалось вычислить выражение `when` «{ $expr }».
manifest.when.template_error = Не удалось отобразить шаблон `when` «{ $expr }».
manifest.target.vars_not_object = Поле `vars` цели должно быть объектом, получено { $value }.
manifest.vars.entry_not_object = Запись `vars` манифеста должна быть объектом.
manifest.field_not_string = Поле «{ $field }» должно быть строкой.
manifest.expression.parse_error = Не удалось разобрать выражение { $name }.
manifest.expression.eval_error = Не удалось вычислить выражение { $name }.

# Диагностика макросов манифеста.
manifest.macro.signature_missing_identifier = В сигнатуре макроса отсутствует идентификатор.
manifest.macro.signature_missing_params = В сигнатуре макроса отсутствуют параметры.
manifest.macro.compile_failed = Не удалось скомпилировать макрос { $name }.
manifest.macro.sequence_invalid = Макросы должны задаваться как отображение имён на шаблоны.
manifest.macro.register_failed = Не удалось зарегистрировать макросы манифеста.
manifest.macro.not_initialised = Окружение макросов не инициализировано.
manifest.macro.caller_invalid = Вызывающий макрос должен быть строкой.
manifest.macro.template_load_failed = Не удалось загрузить шаблон макроса.
manifest.macro.init_failed = Не удалось инициализировать окружение макросов.
manifest.macro.missing = Макрос { $name } отсутствует.

# Ошибки шаблонов glob в манифесте.
manifest.glob.unmatched_brace = Некорректный шаблон glob «{ $pattern }»: «{ $character }» без пары в позиции { $position }.
manifest.glob.invalid_pattern = Некорректный шаблон glob «{ $pattern }»: { $detail }.
manifest.glob.unknown_pattern_error = неизвестная ошибка шаблона.
manifest.glob.io_failed = Сбой glob для «{ $pattern }»: { $detail }.
manifest.glob.unknown_io_error = неизвестная ошибка ввода-вывода.

# Ошибки промежуточного представления.
ir.rule_not_found = Правило «{ $rule }», на которое ссылается цель «{ $target }», не найдено.
ir.multiple_rules = Цель «{ $target }» должна ссылаться ровно на одно правило, получено { $rules }.
ir.empty_rule = Цель «{ $target }» должна ссылаться на правило.
ir.duplicate_outputs = Обнаружены повторяющиеся выходные файлы: { $outputs }.
ir.circular_dependency = Обнаружена циклическая зависимость: { $cycle }.
ir.action_serialisation = Не удалось сериализовать действие: { $details }.
ir.invalid_command = Некорректная подстановка в команде: { $snippet }.

# Ошибки генерации файлов Ninja.
ninja_gen.missing_action = Отсутствует действие «{ $id }», на которое ссылается ребро сборки.
ninja_gen.format = Не удалось отформатировать вывод манифеста Ninja.

# Проверка шаблонов узлов.
host_pattern.empty = Шаблон узла не должен быть пустым.
host_pattern.contains_scheme = Шаблон узла «{ $pattern }» не должен содержать схему URL.
host_pattern.contains_slash = Шаблон узла «{ $pattern }» не должен содержать «/».
host_pattern.missing_suffix = Шаблон узла «{ $pattern }» должен содержать суффикс после «*.».
host_pattern.empty_label = Шаблон узла «{ $pattern }» содержит пустую метку.
host_pattern.invalid_chars = Шаблон узла «{ $pattern }» содержит недопустимые символы.
host_pattern.invalid_label_edge = Метки шаблона узла «{ $pattern }» не должны начинаться или заканчиваться символом «-».
host_pattern.label_too_long = Шаблон узла «{ $pattern }» содержит метку длиннее 63 символов.
host_pattern.too_long = Шаблон узла «{ $pattern }» превышает ограничение в 255 символов.

# Сетевая политика.
network_policy.scheme.empty = Схема не должна быть пустой.
network_policy.scheme.invalid = Схема «{ $scheme }» содержит недопустимые символы.
network_policy.allowlist.empty = Список разрешённых узлов не должен быть пустым.
network_policy.scheme.not_allowed = Схема «{ $scheme }» не разрешена.
network_policy.missing_host = В URL отсутствует узел.
network_policy.host.blocked = Узел «{ $host }» заблокирован политикой.
network_policy.host.not_allowlisted = Узла «{ $host }» нет в списке разрешённых.

# Конфигурация стандартной библиотеки.
stdlib.config.default_fetch_cache_invalid = Путь кэша fetch по умолчанию должен быть относительным.
stdlib.config.default_which_cache_invalid = Ёмкость кэша which по умолчанию должна быть положительной.
stdlib.config.workspace_root_absolute = Корневой путь рабочего пространства должен быть абсолютным.
stdlib.config.fetch_response_limit_positive = Ограничение на ответ fetch должно быть положительным.
stdlib.config.command_output_limit_positive = Ограничение на перехватываемый вывод команд должно быть положительным.
stdlib.config.command_stream_limit_positive = Ограничение на поток команд должно быть положительным.
stdlib.config.which_cache_capacity_positive = Ёмкость кэша which должна быть положительной.
stdlib.config.skip_dir_empty = Записи пропускаемых каталогов не должны быть пустыми.
stdlib.config.skip_dir_navigation = Записи пропускаемых каталогов не должны содержать «..».
stdlib.config.skip_dir_separator = Записи пропускаемых каталогов не должны содержать разделители пути.
stdlib.config.fetch_cache_empty = Путь кэша fetch не должен быть пустым.
stdlib.config.fetch_cache_not_relative = Путь кэша fetch должен быть относительным, получено { $path }.
stdlib.config.fetch_cache_escapes = Путь кэша fetch не должен выходить за пределы рабочего пространства: { $path }.
stdlib.config.open_workspace_root = Не удалось открыть текущий каталог как корень рабочего пространства stdlib.
stdlib.config.resolve_cwd = Не удалось определить текущий каталог как корень рабочего пространства stdlib.
stdlib.config.cwd_non_utf8 = Текущий каталог содержит части, не являющиеся UTF-8: { $path }.

# Диагностика помощника fetch.
stdlib.fetch.url_invalid = Некорректный URL «{ $url }»: { $details }.
stdlib.fetch.disallowed = URL «{ $url }» не разрешён: { $details }.
stdlib.fetch.failed = Не удалось загрузить «{ $url }»: { $details }.
stdlib.fetch.cache_read_failed = Не удалось прочитать запись кэша «{ $name }»: { $details }.
stdlib.fetch.cache_open_failed = Не удалось открыть запись кэша «{ $name }»: { $details }.
stdlib.fetch.response_read_failed = Не удалось прочитать ответ от «{ $url }»: { $details }.
stdlib.fetch.response_buffer_overflow = Переполнение буфера при чтении «{ $url }».
stdlib.fetch.cache_write_failed = Не удалось записать кэш для «{ $url }»: { $details }.
stdlib.fetch.response_limit_exceeded = Ответ от «{ $url }» превысил ограничение в { $limit } байт.
stdlib.fetch.cache_limit_exceeded = Кэшированный ответ «{ $name }» превысил ограничение в { $limit } байт.
stdlib.fetch.io_failed = Не удалось выполнить действие «{ $action }» для { $path }: { $details }.
stdlib.fetch.action.sync_cache = синхронизация кэша fetch
stdlib.fetch.action.create_cache_dir = создание каталога кэша fetch
stdlib.fetch.action.open_cache_dir = открытие каталога кэша fetch
stdlib.fetch.action.stat_cache = получение сведений о записи кэша fetch
stdlib.fetch.action.open_cache_entry = открытие записи кэша fetch

# Диагностика помощника для команд.
stdlib.command.location = команда «{ $command }» в шаблоне «{ $template }»
stdlib.command.spawn_failed = Не удалось запустить { $location }: { $details }.
stdlib.command.io_failed = Сбой { $location }: { $details }.
stdlib.command.closed_input_early = Ввод закрылся до завершения записи в команду.
stdlib.command.broken_pipe = Разорванный канал при выполнении { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } завершена сигналом.
stdlib.command.exited_with_status = { $location } завершилась с кодом { $status }.
stdlib.command.output_limit_exceeded = { $location } превысила ограничение режима «{ $mode }» в { $limit } байт для { $stream }.
stdlib.command.timeout = { $location } превысила предельное время в { $seconds } с.
stdlib.command.exit_status_suffix = (код завершения { $status })
stdlib.command.signal_suffix = (завершено сигналом)
stdlib.command.shell.empty = Команда оболочки не должна быть пустой.
stdlib.command.grep.empty_pattern = Шаблон grep не должен быть пустым.
stdlib.command.grep.flags_not_string = Флаги grep должны быть строками.
stdlib.command.quote.invalid = Не удалось заключить { $arg } в кавычки: { $details }.
stdlib.command.quote.line_break = Аргументы с возвратом каретки или переводом строки нельзя безопасно заключить в кавычки.
stdlib.command.input_undefined = Входное значение не определено.
stdlib.command.tempfile.root_required = Для создания временных файлов команд требуется корень рабочего пространства.
stdlib.command.tempfile.create_failed = Не удалось создать временный файл команды: { $details }.
stdlib.command.options.invalid_utf8 = Ключ параметра команды должен быть корректным UTF-8.
stdlib.command.option.mode_not_string = Режим вывода должен быть строкой.
stdlib.command.options.invalid_type = Параметры команды должны быть объектом.
stdlib.command.output.mode_unsupported = Неподдерживаемый режим вывода «{ $mode }».
stdlib.command.output.mode.capture = перехват
stdlib.command.output.mode.streaming = потоковая передача
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Диагностика помощника для путей.
stdlib.path.io.failed = Не удалось выполнить действие «{ $action }» для { $path } ({ $label }).
stdlib.path.io.failed_with_detail = Не удалось выполнить действие «{ $action }» для { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = Не удалось выполнить действие «{ $action }» для { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = не найдено
stdlib.path.io.permission_denied = доступ запрещён
stdlib.path.io.already_exists = уже существует
stdlib.path.io.invalid_input = некорректный ввод
stdlib.path.io.invalid_data = некорректные данные
stdlib.path.io.timed_out = истекло время ожидания
stdlib.path.io.interrupted = прервано
stdlib.path.io.would_block = привело бы к блокировке
stdlib.path.io.write_zero = записано ноль байт
stdlib.path.io.unexpected_eof = неожиданный конец файла
stdlib.path.io.broken_pipe = разорванный канал
stdlib.path.io.connection_refused = в соединении отказано
stdlib.path.io.connection_reset = соединение сброшено
stdlib.path.io.connection_aborted = соединение прервано
stdlib.path.io.not_connected = нет соединения
stdlib.path.io.addr_in_use = адрес уже используется
stdlib.path.io.addr_not_available = адрес недоступен
stdlib.path.io.out_of_memory = недостаточно памяти
stdlib.path.io.unsupported = не поддерживается
stdlib.path.io.file_too_large = файл слишком велик
stdlib.path.io.resource_busy = ресурс занят
stdlib.path.io.executable_busy = исполняемый файл занят
stdlib.path.io.deadlock = взаимная блокировка
stdlib.path.io.crosses_devices = пересекает границу устройств
stdlib.path.io.too_many_links = слишком много ссылок
stdlib.path.io.invalid_filename = некорректное имя файла
stdlib.path.io.arg_list_too_long = слишком длинный список аргументов
stdlib.path.io.stale_handle = устаревший дескриптор сетевого файла
stdlib.path.io.storage_full = хранилище заполнено
stdlib.path.io.not_seekable = позиционирование недоступно
stdlib.path.io.network_down = сеть не работает
stdlib.path.io.network_unreachable = сеть недоступна
stdlib.path.io.host_unreachable = узел недоступен
stdlib.path.io.other = ошибка ввода-вывода
stdlib.path.action.canonicalize = канонизация
stdlib.path.action.open_directory = открытие каталога
stdlib.path.action.stat = получение сведений
stdlib.path.action.read = чтение
stdlib.path.action.open_file = открытие файла
stdlib.path.with_suffix.empty_separator = with_suffix требует непустой разделитель.
stdlib.path.relative_to.mismatch = { $path } не является относительным к { $root }.
stdlib.path.expanduser.unsupported = Раскрытие ~ для конкретного пользователя не поддерживается.
stdlib.path.expanduser.no_home = Не удаётся раскрыть ~: не задана ни одна переменная окружения домашнего каталога.
stdlib.path.contents.unsupported_encoding = Неподдерживаемая кодировка «{ $encoding }».
stdlib.path.hash.unsupported_algorithm = Неподдерживаемый алгоритм хеширования «{ $algorithm }».
stdlib.path.hash.unsupported_algorithm_legacy = Неподдерживаемый алгоритм хеширования «{ $algorithm }» (включите возможность «{ $feature }»).

# Диагностика помощников для коллекций.
stdlib.collections.flatten.expected_sequence = flatten ожидал элементы последовательности, но обнаружил { $kind }.
stdlib.collections.group_by.empty_attribute = group_by требует непустой атрибут.
stdlib.collections.group_by.unresolved = group_by не смог найти «{ $attr }» у элемента типа { $kind }.

# Диагностика помощников для времени.
stdlib.time.offset.invalid = Смещение now «{ $offset }» некорректно: ожидалось «+HH:MM[:SS]» или «Z».
stdlib.time.timedelta.overflow = Переполнение timedelta при добавлении компонента { $component }.
stdlib.time.label.weeks = недели
stdlib.time.label.days = дни
stdlib.time.label.hours = часы
stdlib.time.label.minutes = минуты
stdlib.time.label.seconds = секунды
stdlib.time.label.milliseconds = миллисекунды
stdlib.time.label.microseconds = микросекунды
stdlib.time.label.nanoseconds = наносекунды

# Диагностика помощника which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] команда «{ $command }» не найдена после проверки { $count } записей PATH. Предпросмотр: { $preview }
stdlib.which.not_found.hint.cwd_auto = Пустые сегменты PATH игнорируются; задайте cwd_mode="auto", чтобы включить рабочий каталог.
stdlib.which.not_found.hint.cwd_always = Задайте cwd_mode="always", чтобы включить текущий каталог.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] команда «{ $command }» по пути «{ $path }» отсутствует или не является исполняемой.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <пусто>
stdlib.which.path_entry.non_utf8 = Запись PATH № { $index } содержит символы, не являющиеся UTF-8; Netsuke требует пути в UTF-8.
stdlib.which.command.empty = which требует непустую строку.
stdlib.which.cwd_mode.invalid = cwd_mode должен быть «auto», «always» или «never», получено «{ $mode }».
stdlib.which.cwd.resolve_failed = Не удалось определить текущий каталог: { $details }.
stdlib.which.cwd.non_utf8 = Текущий каталог содержит части, не являющиеся UTF-8.
stdlib.which.canonicalize_failed = Не удалось канонизировать «{ $path }»: { $details }.
stdlib.which.is_executable = Не удалось проверить, является ли «{ $path }» исполняемым: { $details }.
stdlib.which.canonicalize_non_utf8 = Канонический путь содержит части, не являющиеся UTF-8.
stdlib.which.workspace_non_utf8 = Путь рабочего пространства содержит части, не являющиеся UTF-8, при поиске команды «{ $command }»: { $path }.
stdlib.which.walkdir_error = Ошибка обхода рабочего пространства при поиске команды: { $details }.

# Регистрация стандартной библиотеки.
stdlib.register.open_dir = Не удалось открыть текущий каталог для регистрации stdlib.
stdlib.register.resolve_dir = Не удалось определить текущий каталог для регистрации stdlib.
stdlib.register.dir_non_utf8 = Текущий каталог содержит части, не являющиеся UTF-8: { $path }.

# Отчёт о состоянии в доступном режиме вывода.
status.state.pending = ожидает
status.state.running = выполняется
status.state.done = готово
status.state.failed = сбой
status.stage.label = Этап { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Задача { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Чтение файла манифеста
status.stage.initial_yaml_parsing = Разбор документа YAML
status.stage.template_expansion = Раскрытие директив шаблона
status.stage.final_rendering = Десериализация и отображение значений манифеста
status.stage.ir_generation_validation = Построение и проверка графа зависимостей
status.stage.ninja_synthesis = Построение плана сборки Ninja
status.stage.ninja_synthesis_execute = Построение плана Ninja и запуск { $tool }
status.stage.graph_rendering = Отображение артефакта графа
status.stage.graph_rendering_with_tool = Отображение { $tool }
status.complete = { $tool }: операция завершена.
status.timing.summary_header = Сводка времени по этапам:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Общее время конвейера: { $duration }
status.tool.build = Сборка
status.tool.clean = Очистка
status.tool.graph = Граф
status.tool.graph_html = Граф (HTML)
status.tool.generate = Генерация

# Строки HTML-представления графа.
graph.html.title = Граф сборки Netsuke
graph.html.heading = Граф сборки Netsuke
graph.html.description = Граф сборки, отображённый Netsuke
graph.html.outline.summary = Цели и зависимости (текстовая структура)
graph.html.outline.no_inputs = Нет входных данных
graph.html.noscript.notice = JavaScript отключён. Текстовая структура выше содержит весь граф; ниже приведён исходный код DOT.

# Семантические префиксы доступного вывода.
semantic.prefix.error = Ошибка:
semantic.prefix.warning = Предупреждение:
semantic.prefix.success = Успешно:
semantic.prefix.info = Сведения:
semantic.prefix.timing = Время:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Примеры форм множественного числа для переводчиков.
# Русский использует категории CLDR `one`, `few`, `many` и `other`. Целые
# числа распределяются так: `one` — 1, 21, 31…, `few` — 2–4, 22–24…,
# `many` — 0, 5–20, 25–30… Категория `other` относится к дробным значениям,
# поэтому она же служит вариантом по умолчанию.
example.files_processed = { $count ->
    [one] Обработан { $count } файл.
    [few] Обработано { $count } файла.
    [many] Обработано { $count } файлов.
   *[other] Обработано { $count } файла.
}

example.errors_found = { $count ->
    [0] Ошибок не найдено.
    [one] Найдена { $count } ошибка.
    [few] Найдено { $count } ошибки.
    [many] Найдено { $count } ошибок.
   *[other] Найдено { $count } ошибки.
}
