# Recursos de localización para la CLI de Netsuke (español de América Latina).

cli.about = Netsuke compila manifiestos YAML + Jinja en planes de compilación de Ninja.
cli.long_about = Netsuke transforma manifiestos YAML + Jinja en grafos de Ninja reproducibles y ejecuta Ninja con valores predeterminados seguros.
cli.usage = { $usage }

# Texto de ayuda de las opciones globales.
cli.flag.file.help = Ruta al archivo de manifiesto de Netsuke que se va a usar.
cli.flag.directory.help = Ejecutar como si se hubiera iniciado en este directorio.
cli.flag.config.help = Ruta a un archivo de configuración, omitiendo la detección automática.
cli.flag.jobs.help = Establecer la cantidad de trabajos de compilación en paralelo.
cli.flag.verbose.help = Habilitar registros de diagnóstico detallados y resúmenes de tiempos al finalizar.
cli.flag.locale.help = Etiqueta de idioma para los textos de la CLI (por ejemplo: en-US, es-419).
cli.flag.fetch_allow_scheme.help = Esquemas de URL adicionales permitidos para el asistente fetch.
cli.flag.fetch_allow_host.help = Nombres de host permitidos cuando el rechazo predeterminado está activo.
cli.flag.fetch_block_host.help = Nombres de host siempre bloqueados, incluso si se permiten en otro lugar.
cli.flag.fetch_default_deny.help = Rechazar todos los hosts de forma predeterminada; permitir solo la lista declarada.
cli.flag.json.help = Emitir salida JSON legible por máquinas.
cli.flag.no_input.help = Nunca leer entrada interactiva.
cli.flag.color.help = Política de color en la salida (auto, always, never).
cli.flag.emoji.help = Política de emojis (auto, always, never).
cli.flag.progress.help = Política de visualización del progreso (auto, always, never).
cli.flag.accessibility.help = Política de salida accesible (auto, on, off).
cli.flag.default_targets.help = Objetivos de compilación predeterminados cuando no se indica ninguno.

# Descripciones de los subcomandos.
cli.subcommand.build.about = Compilar los objetivos definidos en el manifiesto (predeterminado).
cli.subcommand.build.long_about = Compilar los objetivos solicitados; si no se indican, usar los predeterminados del manifiesto.
cli.subcommand.clean.about = Eliminar los artefactos de compilación mediante Ninja.
cli.subcommand.clean.long_about = Generar un archivo Ninja temporal y luego ejecutar `ninja -t clean`.
cli.subcommand.graph.about = Emitir el grafo de dependencias de compilación. El formato predeterminado es DOT.
cli.subcommand.graph.long_about = Proyectar el manifiesto de Netsuke analizado en un grafo de compilación canónico y escribirlo como Graphviz DOT, o como página HTML autónoma con `--html`. Use `--output <ARCHIVO>` para escribir en un archivo; `-` escribe en stdout.
cli.subcommand.generate.about = Generar el manifiesto de Ninja sin ejecutar Ninja.
cli.subcommand.generate.long_about = Escribir el manifiesto de Ninja generado en stdout o en el archivo elegido con `--output`.

# Texto de ayuda de las opciones del subcomando build.
cli.subcommand.build.flag.targets.help = Objetivos que se van a compilar (si se omite, usa los predeterminados del manifiesto).

# Texto de ayuda de las opciones del subcomando graph.
cli.subcommand.graph.flag.html.help = Representar el grafo como página HTML autónoma en lugar de DOT.
cli.subcommand.graph.flag.output.help = Escribir el artefacto del grafo en ARCHIVO; use `-` para stdout.

# Texto de ayuda de las opciones del subcomando generate.
cli.subcommand.generate.flag.output.help = Escribir el manifiesto de Ninja generado en ARCHIVO en lugar de stdout.

# Errores de validación de la CLI.
cli.validation.jobs.invalid_number = { $value } no es un número válido.
cli.validation.jobs.out_of_range = La cantidad de trabajos debe estar entre { $min } y { $max }.
cli.validation.scheme.empty = El esquema no debe estar vacío.
cli.validation.scheme.invalid_start = El esquema '{ $scheme }' debe comenzar con una letra ASCII.
cli.validation.scheme.invalid = Esquema no válido '{ $scheme }'.
cli.validation.locale.empty = La etiqueta de idioma no debe estar vacía.
cli.validation.locale.invalid = Etiqueta de idioma no válida '{ $locale }'.
cli.validation.color.invalid = Política de color no válida '{ $value }'. Opciones válidas: auto, always, never.
cli.validation.emoji.invalid = Política de emojis no válida '{ $value }'. Opciones válidas: auto, always, never.
cli.validation.progress.invalid = Política de progreso no válida '{ $value }'. Opciones válidas: auto, always, never.
cli.validation.accessibility.invalid = Política de accesibilidad no válida '{ $value }'. Opciones válidas: auto, on, off.
cli.validation.config.expected_object = Se esperaba que los valores de la CLI se serializaran como un objeto, se obtuvo { $value }.

# Mensajes de error de Clap.
clap-error-missing-argument = Falta un argumento obligatorio: { $argument }
clap-error-missing-subcommand = Falta el subcomando. Opciones disponibles: { $valid_subcommands }
clap-error-unknown-argument = Argumento desconocido: { $argument }
clap-error-invalid-value = Valor no válido para { $argument }: { $value }
clap-error-invalid-subcommand = Subcomando desconocido: { $subcommand }
# Nota: value-validation usa una redacción distinta de invalid-value para
# diferenciar los errores de validadores personalizados
# (ErrorKind::ValueValidation) de las incompatibilidades de tipo
# (ErrorKind::InvalidValue).
clap-error-value-validation = La validación falló para { $argument }: { $value }

# Errores y contextos del ejecutor.
runner.manifest.not_found = No se encontró el manifiesto '{ $manifest_name }' en { $directory }.
runner.manifest.not_found.help = Verifique que el manifiesto exista o indique `--file` con la ruta correcta.
runner.manifest.path_missing_name = La ruta del manifiesto '{ $path }' no tiene nombre de archivo.
runner.manifest.path_utf8 = La ruta del manifiesto '{ $path }' no es UTF-8 válido.
runner.manifest.directory_utf8 = La ruta del directorio del manifiesto '{ $path }' no es UTF-8 válido.
runner.manifest.directory_label = directorio `{ $directory }`
runner.manifest.current_directory_label = el directorio actual
runner.context.network_policy = No se pudo construir la política de red.
runner.context.load_manifest = No se pudo cargar el manifiesto en { $path }.
runner.context.serialise_manifest = No se pudo serializar el manifiesto.
runner.context.build_graph = No se pudo construir el grafo a partir del manifiesto.
runner.context.generate_ninja = No se pudo generar el manifiesto de Ninja.
runner.context.render_graph = No se pudo representar el artefacto del grafo.

runner.io.create_temp_file = No se pudo crear el archivo Ninja temporal.
runner.io.write_temp_ninja = No se pudo escribir el archivo Ninja temporal.
runner.io.flush_temp_ninja = No se pudo vaciar el archivo Ninja temporal.
runner.io.sync_temp_ninja = No se pudo sincronizar el archivo Ninja temporal.
runner.io.create_parent_dir = No se pudo crear el directorio principal { $path }.
runner.io.create_ninja_file = No se pudo crear el archivo Ninja en { $path }.
runner.io.write_ninja_file = No se pudo escribir el archivo Ninja en { $path }.
runner.io.flush_ninja_file = No se pudo vaciar el archivo Ninja en { $path }.
runner.io.sync_ninja_file = No se pudo sincronizar el archivo Ninja en { $path }.
runner.io.open_ambient_dir = No se pudo abrir el directorio del entorno.
runner.io.no_existing_ancestor = No existe un directorio antecesor para { $path }.
runner.io.derive_relative_path = No se pudo derivar la ruta relativa de Ninja.
runner.io.non_utf8_path = No se admiten rutas que no sean UTF-8 (ruta: { $path }).
runner.io.write_stdout = No se pudo escribir el manifiesto de Ninja en stdout.
runner.io.flush_stdout = No se pudo vaciar stdout.

# Diagnósticos del manifiesto.
manifest.parse = Falló el análisis del manifiesto.
manifest.structure_error = Error de estructura del manifiesto en { $name }: { $details }
manifest.yaml.parse = Error de análisis de YAML en la línea { $line }, columna { $column }: { $details }
manifest.yaml.label = YAML no válido
manifest.yaml.hint.tabs = YAML no permite tabulaciones; use espacios para la sangría.
manifest.yaml.hint.list_item = Los elementos de lista de YAML deben comenzar con '-' y estar bien sangrados.
manifest.yaml.hint.expected_colon = Esto parece una entrada de mapeo; falta un ':' después de la clave.
manifest.yaml.hint.mapping_values = Los mapeos de YAML requieren un valor después de ':' (o un bloque anidado).
manifest.yaml.hint.invalid_token = El token de YAML no es válido o es inesperado.
manifest.yaml.hint.escape = Escape las barras invertidas o elimine las secuencias de escape no válidas.
manifest.env.missing = Una variable de entorno requerida no está definida.
manifest.env.invalid_utf8 = Una variable de entorno contiene UTF-8 no válido.
manifest.vars.not_object = `vars` del manifiesto debe ser un mapa u objeto.
manifest.vars.reserved_name = La clave `vars` '{ $name }' del manifiesto está reservada para una función auxiliar de plantillas integrada; cambie el nombre de la variable.
manifest.read_failed = No se pudo leer el manifiesto en { $path }.
manifest.resolve_workspace_root = No se pudo resolver la raíz del espacio de trabajo.
manifest.workspace_non_utf8 = La ruta raíz del espacio de trabajo '{ $path }' no es UTF-8 válido.
manifest.path_non_utf8 = La ruta del manifiesto '{ $manifest }' no es UTF-8 válido: { $path }.
manifest.path_missing_name = La ruta del manifiesto '{ $path }' no tiene nombre de archivo.
manifest.open_workspace_failed = No se pudo abrir el espacio de trabajo { $workspace } para el manifiesto { $manifest }.
manifest.foreach.not_iterable = La expresión `foreach` no es iterable.
manifest.foreach.serialise_item = No se pudo serializar el elemento de `foreach`.
manifest.when.empty = La expresión `when` no debe estar vacía.
manifest.when.eval_error = No se pudo evaluar la expresión `when` '{ $expr }'.
manifest.when.template_error = No se pudo representar la plantilla `when` '{ $expr }'.
manifest.target.vars_not_object = `vars` del objetivo debe ser un objeto, se obtuvo { $value }.
manifest.vars.entry_not_object = Una entrada `vars` del manifiesto debe ser un objeto.
manifest.field_not_string = El campo '{ $field }' debe ser una cadena.
manifest.expression.parse_error = No se pudo analizar la expresión { $name }.
manifest.expression.eval_error = No se pudo evaluar la expresión { $name }.

# Diagnósticos de las macros del manifiesto.
manifest.macro.signature_missing_identifier = A la firma de la macro le falta un identificador.
manifest.macro.signature_missing_params = A la firma de la macro le faltan parámetros.
manifest.macro.compile_failed = No se pudo compilar la macro { $name }.
manifest.macro.sequence_invalid = Las macros deben definirse como un mapeo de nombres a plantillas.
manifest.macro.register_failed = No se pudieron registrar las macros del manifiesto.
manifest.macro.not_initialised = El entorno de macros no está inicializado.
manifest.macro.caller_invalid = El llamador de la macro debe ser una cadena.
manifest.macro.template_load_failed = No se pudo cargar la plantilla de la macro.
manifest.macro.init_failed = No se pudo inicializar el entorno de macros.
manifest.macro.missing = Falta la macro { $name }.

# Errores de glob del manifiesto.
manifest.glob.unmatched_brace = Patrón glob no válido '{ $pattern }': '{ $character }' sin pareja en la posición { $position }.
manifest.glob.invalid_pattern = Patrón glob no válido '{ $pattern }': { $detail }.
manifest.glob.unknown_pattern_error = error de patrón desconocido.
manifest.glob.io_failed = El glob falló para '{ $pattern }': { $detail }.
manifest.glob.unknown_io_error = error de E/S desconocido.
manifest.command_list_empty = El campo 'command' no debe estar vacío: proporcione una cadena de comando o una lista no vacía.

# Errores de la representación intermedia.
ir.rule_not_found = No se encontró la regla '{ $rule }' referenciada por el objetivo '{ $target }'.
ir.multiple_rules = El objetivo '{ $target }' debe referenciar una sola regla, se obtuvo { $rules }.
ir.empty_rule = El objetivo '{ $target }' debe referenciar una regla.
ir.duplicate_outputs = Se detectaron salidas duplicadas: { $outputs }.
ir.circular_dependency = Se detectó una dependencia circular: { $cycle }.
ir.action_serialisation = No se pudo serializar la acción: { $details }.
ir.invalid_command = Interpolación de comando no válida: { $snippet }.

# Errores de generación de Ninja.
ninja_gen.missing_action = Falta la acción '{ $id }' referenciada por una arista de compilación.
ninja_gen.format = No se pudo dar formato a la salida del manifiesto de Ninja.

# Validación de patrones de host.
host_pattern.empty = El patrón de host no debe estar vacío.
host_pattern.contains_scheme = El patrón de host '{ $pattern }' no debe incluir un esquema de URL.
host_pattern.contains_slash = El patrón de host '{ $pattern }' no debe incluir '/'.
host_pattern.missing_suffix = El patrón de host '{ $pattern }' debe incluir un sufijo después de '*.'.
host_pattern.empty_label = El patrón de host '{ $pattern }' contiene una etiqueta vacía.
host_pattern.invalid_chars = El patrón de host '{ $pattern }' contiene caracteres no válidos.
host_pattern.invalid_label_edge = Las etiquetas del patrón de host '{ $pattern }' no deben comenzar ni terminar con '-'.
host_pattern.label_too_long = El patrón de host '{ $pattern }' contiene una etiqueta de más de 63 caracteres.
host_pattern.too_long = El patrón de host '{ $pattern }' supera el límite de 255 caracteres.

# Política de red.
network_policy.scheme.empty = El esquema no debe estar vacío.
network_policy.scheme.invalid = El esquema '{ $scheme }' contiene caracteres no válidos.
network_policy.allowlist.empty = La lista de hosts permitidos no debe estar vacía.
network_policy.scheme.not_allowed = El esquema '{ $scheme }' no está permitido.
network_policy.missing_host = A la URL le falta el host.
network_policy.host.blocked = El host '{ $host }' está bloqueado por la política.
network_policy.host.not_allowlisted = El host '{ $host }' no está en la lista de permitidos.

# Configuración de la biblioteca estándar.
stdlib.config.default_fetch_cache_invalid = La ruta predeterminada de la caché de fetch debe ser relativa.
stdlib.config.default_which_cache_invalid = La capacidad predeterminada de la caché de which debe ser positiva.
stdlib.config.workspace_root_absolute = La ruta raíz del espacio de trabajo debe ser absoluta.
stdlib.config.fetch_response_limit_positive = El límite de respuesta de fetch debe ser positivo.
stdlib.config.command_output_limit_positive = El límite de captura de salida de comandos debe ser positivo.
stdlib.config.command_stream_limit_positive = El límite de transmisión de comandos debe ser positivo.
stdlib.config.which_cache_capacity_positive = La capacidad de la caché de which debe ser positiva.
stdlib.config.skip_dir_empty = Las entradas de directorios omitidos no deben estar vacías.
stdlib.config.skip_dir_navigation = Las entradas de directorios omitidos no deben contener '..'.
stdlib.config.skip_dir_separator = Las entradas de directorios omitidos no deben contener separadores de ruta.
stdlib.config.fetch_cache_empty = La ruta de la caché de fetch no debe estar vacía.
stdlib.config.fetch_cache_not_relative = La ruta de la caché de fetch debe ser relativa, se obtuvo { $path }.
stdlib.config.fetch_cache_escapes = La ruta de la caché de fetch no debe salir del espacio de trabajo: { $path }.
stdlib.config.open_workspace_root = No se pudo abrir el directorio actual como raíz del espacio de trabajo de la stdlib.
stdlib.config.resolve_cwd = No se pudo resolver el directorio actual como raíz del espacio de trabajo de la stdlib.
stdlib.config.cwd_non_utf8 = El directorio actual contiene componentes que no son UTF-8: { $path }.

# Diagnósticos del asistente fetch.
stdlib.fetch.url_invalid = URL no válida '{ $url }': { $details }.
stdlib.fetch.disallowed = La URL '{ $url }' no está permitida: { $details }.
stdlib.fetch.failed = No se pudo descargar '{ $url }': { $details }.
stdlib.fetch.cache_read_failed = No se pudo leer la entrada de caché '{ $name }': { $details }.
stdlib.fetch.cache_open_failed = No se pudo abrir la entrada de caché '{ $name }': { $details }.
stdlib.fetch.response_read_failed = No se pudo leer la respuesta de '{ $url }': { $details }.
stdlib.fetch.response_buffer_overflow = Desbordamiento del búfer al leer '{ $url }'.
stdlib.fetch.cache_write_failed = No se pudo escribir la caché para '{ $url }': { $details }.
stdlib.fetch.response_limit_exceeded = La respuesta de '{ $url }' superó el límite de { $limit } bytes.
stdlib.fetch.cache_limit_exceeded = La respuesta en caché '{ $name }' superó el límite de { $limit } bytes.
stdlib.fetch.io_failed = { $action } falló para { $path }: { $details }.
stdlib.fetch.action.sync_cache = sincronizar la caché de fetch
stdlib.fetch.action.create_cache_dir = crear el directorio de caché de fetch
stdlib.fetch.action.open_cache_dir = abrir el directorio de caché de fetch
stdlib.fetch.action.stat_cache = consultar la entrada de caché de fetch
stdlib.fetch.action.open_cache_entry = abrir la entrada de caché de fetch

# Diagnósticos del asistente de comandos.
stdlib.command.location = comando '{ $command }' en la plantilla '{ $template }'
stdlib.command.spawn_failed = No se pudo iniciar { $location }: { $details }.
stdlib.command.io_failed = { $location } falló: { $details }.
stdlib.command.closed_input_early = La entrada se cerró antes de terminar de escribir en el comando.
stdlib.command.broken_pipe = Canalización rota al ejecutar { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } terminó por una señal.
stdlib.command.exited_with_status = { $location } salió con el estado { $status }.
stdlib.command.output_limit_exceeded = { $location } superó el límite de { $mode } de { $limit } bytes para { $stream }.
stdlib.command.timeout = { $location } excedió el tiempo de espera de { $seconds } segundos.
stdlib.command.exit_status_suffix = (estado de salida { $status })
stdlib.command.signal_suffix = (terminado por una señal)
stdlib.command.shell.empty = El comando de shell no debe estar vacío.
stdlib.command.grep.empty_pattern = El patrón de grep no debe estar vacío.
stdlib.command.grep.flags_not_string = Las banderas de grep deben ser cadenas.
stdlib.command.quote.invalid = No se pudo entrecomillar { $arg }: { $details }.
stdlib.command.quote.line_break = Los argumentos con retornos de carro o saltos de línea no se pueden entrecomillar de forma segura.
stdlib.command.input_undefined = El valor de entrada no está definido.
stdlib.command.tempfile.root_required = Se requiere la raíz del espacio de trabajo para crear archivos temporales de comandos.
stdlib.command.tempfile.create_failed = No se pudo crear el archivo temporal del comando: { $details }.
stdlib.command.options.invalid_utf8 = La clave de una opción del comando debe ser UTF-8 válido.
stdlib.command.option.mode_not_string = El modo de salida debe ser una cadena.
stdlib.command.options.invalid_type = Las opciones del comando deben ser un objeto.
stdlib.command.output.mode_unsupported = Modo de salida no admitido '{ $mode }'.
stdlib.command.output.mode.capture = captura
stdlib.command.output.mode.streaming = transmisión
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnósticos del asistente de rutas.
stdlib.path.io.failed = { $action } falló para { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } falló para { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } falló para { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = no encontrado
stdlib.path.io.permission_denied = permiso denegado
stdlib.path.io.already_exists = ya existe
stdlib.path.io.invalid_input = entrada no válida
stdlib.path.io.invalid_data = datos no válidos
stdlib.path.io.timed_out = se agotó el tiempo de espera
stdlib.path.io.interrupted = interrumpido
stdlib.path.io.would_block = se bloquearía
stdlib.path.io.write_zero = escritura nula
stdlib.path.io.unexpected_eof = fin de archivo inesperado
stdlib.path.io.broken_pipe = canalización rota
stdlib.path.io.connection_refused = conexión rechazada
stdlib.path.io.connection_reset = conexión restablecida
stdlib.path.io.connection_aborted = conexión anulada
stdlib.path.io.not_connected = sin conexión
stdlib.path.io.addr_in_use = dirección en uso
stdlib.path.io.addr_not_available = dirección no disponible
stdlib.path.io.out_of_memory = sin memoria
stdlib.path.io.unsupported = no admitido
stdlib.path.io.file_too_large = archivo demasiado grande
stdlib.path.io.resource_busy = recurso ocupado
stdlib.path.io.executable_busy = ejecutable ocupado
stdlib.path.io.deadlock = bloqueo mutuo
stdlib.path.io.crosses_devices = cruza dispositivos
stdlib.path.io.too_many_links = demasiados enlaces
stdlib.path.io.invalid_filename = nombre de archivo no válido
stdlib.path.io.arg_list_too_long = lista de argumentos demasiado larga
stdlib.path.io.stale_handle = descriptor de archivo de red obsoleto
stdlib.path.io.storage_full = almacenamiento lleno
stdlib.path.io.not_seekable = no admite posicionamiento
stdlib.path.io.network_down = red caída
stdlib.path.io.network_unreachable = red inalcanzable
stdlib.path.io.host_unreachable = host inalcanzable
stdlib.path.io.other = error de E/S
stdlib.path.action.canonicalize = canonicalizar
stdlib.path.action.open_directory = abrir el directorio
stdlib.path.action.stat = consultar
stdlib.path.action.read = leer
stdlib.path.action.open_file = abrir el archivo
stdlib.path.with_suffix.empty_separator = with_suffix requiere un separador no vacío.
stdlib.path.relative_to.mismatch = { $path } no es relativo a { $root }.
stdlib.path.expanduser.unsupported = La expansión de ~ para un usuario específico no es compatible.
stdlib.path.expanduser.no_home = No se puede expandir ~: no hay variables de entorno del directorio de inicio definidas.
stdlib.path.contents.unsupported_encoding = Codificación no admitida '{ $encoding }'.
stdlib.path.hash.unsupported_algorithm = Algoritmo de hash no admitido '{ $algorithm }'.
stdlib.path.hash.unsupported_algorithm_legacy = Algoritmo de hash no admitido '{ $algorithm }' (habilite la característica '{ $feature }').

# Diagnósticos de los asistentes de colecciones.
stdlib.collections.flatten.expected_sequence = flatten esperaba elementos de una secuencia, pero encontró { $kind }.
stdlib.collections.group_by.empty_attribute = group_by requiere un atributo no vacío.
stdlib.collections.group_by.unresolved = group_by no pudo resolver '{ $attr }' en un elemento de tipo { $kind }.

# Diagnósticos de los asistentes de tiempo.
stdlib.time.offset.invalid = El desplazamiento de now '{ $offset }' no es válido: se esperaba '+HH:MM[:SS]' o 'Z'.
stdlib.time.timedelta.overflow = Desbordamiento de timedelta al sumar { $component }.
stdlib.time.label.weeks = semanas
stdlib.time.label.days = días
stdlib.time.label.hours = horas
stdlib.time.label.minutes = minutos
stdlib.time.label.seconds = segundos
stdlib.time.label.milliseconds = milisegundos
stdlib.time.label.microseconds = microsegundos
stdlib.time.label.nanoseconds = nanosegundos

# Diagnósticos del asistente which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] no se encontró el comando '{ $command }' tras revisar { $count } entradas de PATH. Vista previa: { $preview }
stdlib.which.not_found.hint.cwd_auto = Los segmentos vacíos de PATH se ignoran; use cwd_mode="auto" para incluir el directorio de trabajo.
stdlib.which.not_found.hint.cwd_always = Establezca cwd_mode="always" para incluir el directorio actual.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] el comando '{ $command }' en '{ $path }' no existe o no es ejecutable.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <vacío>
stdlib.which.path_entry.non_utf8 = La entrada n.º { $index } de PATH contiene caracteres que no son UTF-8; Netsuke requiere rutas UTF-8.
stdlib.which.command.empty = which requiere una cadena no vacía.
stdlib.which.cwd_mode.invalid = cwd_mode debe ser 'auto', 'always' o 'never', se obtuvo '{ $mode }'.
stdlib.which.cwd.resolve_failed = No se pudo resolver el directorio actual: { $details }.
stdlib.which.cwd.non_utf8 = El directorio actual contiene componentes que no son UTF-8.
stdlib.which.canonicalize_failed = No se pudo canonicalizar '{ $path }': { $details }.
stdlib.which.is_executable = No se pudo comprobar si '{ $path }' es ejecutable: { $details }.
stdlib.which.canonicalize_non_utf8 = La ruta canónica contiene componentes que no son UTF-8.
stdlib.which.workspace_non_utf8 = La ruta del espacio de trabajo contiene componentes que no son UTF-8 al resolver el comando '{ $command }': { $path }.
stdlib.which.walkdir_error = Error al recorrer el espacio de trabajo mientras se resolvía el comando: { $details }.

# Registro de la biblioteca estándar.
stdlib.register.open_dir = No se pudo abrir el directorio actual para registrar la stdlib.
stdlib.register.resolve_dir = No se pudo resolver el directorio actual para registrar la stdlib.
stdlib.register.dir_non_utf8 = El directorio actual contiene componentes que no son UTF-8: { $path }.

# Informes de estado para el modo de salida accesible.
status.state.pending = pendiente
status.state.running = en curso
status.state.done = completada
status.state.failed = fallida
status.stage.label = Etapa { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Tarea { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = Leyendo el archivo de manifiesto
status.stage.initial_yaml_parsing = Analizando el documento YAML
status.stage.template_expansion = Expandiendo las directivas de plantilla
status.stage.final_rendering = Deserializando y representando los valores del manifiesto
status.stage.ir_generation_validation = Construyendo y validando el grafo de dependencias
status.stage.ninja_synthesis = Sintetizando el plan de compilación de Ninja
status.stage.ninja_synthesis_execute = Sintetizando el plan de Ninja y ejecutando { $tool }
status.stage.graph_rendering = Representando el artefacto del grafo
status.stage.graph_rendering_with_tool = Representando { $tool }
status.complete = { $tool }: operación finalizada.
status.timing.summary_header = Resumen de tiempos por etapa:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Tiempo total de la canalización: { $duration }
status.tool.build = Compilación
status.tool.clean = Limpieza
status.tool.graph = Grafo
status.tool.graph_html = Grafo (HTML)
status.tool.generate = Generación

# Cadenas del representador HTML del grafo.
graph.html.title = Grafo de compilación de Netsuke
graph.html.heading = Grafo de compilación de Netsuke
graph.html.description = Grafo de compilación representado por Netsuke
graph.html.outline.summary = Objetivos y dependencias (esquema de texto)
graph.html.outline.no_inputs = Sin entradas
graph.html.noscript.notice = JavaScript está desactivado. El esquema de texto anterior contiene el grafo completo; a continuación sigue el código DOT.

# Prefijos semánticos para la salida accesible.
semantic.prefix.error = Error:
semantic.prefix.warning = Advertencia:
semantic.prefix.success = Éxito:
semantic.prefix.info = Info:
semantic.prefix.timing = Tiempos:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Ejemplos de formas plurales para traductores.
# El español usa las categorías CLDR `one` y `other`, igual que el idioma
# de origen; cambian tanto la conjugación del verbo como el número del
# sustantivo.
example.files_processed = { $count ->
    [one] Se procesó { $count } archivo.
   *[other] Se procesaron { $count } archivos.
}

example.errors_found = { $count ->
    [0] No se encontraron errores.
    [one] Se encontró { $count } error.
   *[other] Se encontraron { $count } errores.
}
