# Recursos de localización para la CLI de Netsuke.

cli.about = Netsuke compila manifiestos YAML + Jinja en planes de compilación Ninja.
cli.long_about = Netsuke transforma manifiestos YAML + Jinja en grafos Ninja reproducibles y ejecuta Ninja con valores seguros.
cli.usage = { $usage }

# Texto de ayuda para opciones globales.
cli.flag.file.help = Ruta al archivo de manifiesto Netsuke.
cli.flag.directory.help = Ejecutar como si se iniciara en este directorio.
cli.flag.jobs.help = Número de trabajos de compilación en paralelo.
cli.flag.verbose.help = Habilitar registro de diagnóstico detallado.
cli.flag.locale.help = Etiqueta de idioma para la CLI (por ejemplo: en-US, es-ES).
cli.flag.fetch_allow_scheme.help = Esquemas de URL adicionales permitidos para el ayudante fetch.
cli.flag.fetch_allow_host.help = Nombres de host permitidos cuando la denegación predeterminada está habilitada.
cli.flag.fetch_block_host.help = Nombres de host siempre bloqueados, incluso cuando están permitidos.
cli.flag.fetch_default_deny.help = Denegar todos los hosts por defecto; solo permitir la lista de permitidos.

# Descripciones de subcomandos.
cli.subcommand.build.about = Compila objetivos definidos en el manifiesto (predeterminado).
cli.subcommand.build.long_about = Compila los objetivos solicitados; si no se indican, usa los predeterminados del manifiesto.
cli.subcommand.clean.about = Elimina artefactos de compilación mediante Ninja.
cli.subcommand.clean.long_about = Genera un archivo Ninja temporal y ejecuta `ninja -t clean`.
cli.subcommand.graph.about = Emite el grafo de dependencias en formato DOT.
cli.subcommand.graph.long_about = Genera un archivo Ninja temporal y ejecuta `ninja -t graph` para emitir DOT.
cli.subcommand.manifest.about = Escribe el manifiesto Ninja sin ejecutar Ninja.
cli.subcommand.manifest.long_about = Genera el archivo Ninja y lo escribe en la ruta indicada o '-' para stdout.

# Texto de ayuda para opciones del subcomando build.
cli.subcommand.build.flag.emit.help = Escribir el archivo Ninja generado en esta ruta y conservarlo.
cli.subcommand.build.flag.targets.help = Objetivos a compilar (usa los predeterminados del manifiesto si se omite).

# Texto de ayuda para argumentos del subcomando manifest.
cli.subcommand.manifest.flag.file.help = Ruta de salida para el archivo Ninja (use '-' para stdout).

# Errores de validación de la CLI.
cli.validation.jobs.invalid_number = { $value } no es un número válido.
cli.validation.jobs.out_of_range = El número de trabajos debe estar entre { $min } y { $max }.
cli.validation.scheme.empty = El esquema no debe estar vacío.
cli.validation.scheme.invalid_start = El esquema '{ $scheme }' debe comenzar con una letra ASCII.
cli.validation.scheme.invalid = Esquema no válido '{ $scheme }'.
cli.validation.locale.empty = La configuración regional no debe estar vacía.
cli.validation.locale.invalid = Configuración regional no válida '{ $locale }'.
cli.validation.config.expected_object = Se esperaba que los valores de la CLI se serializaran como un objeto, se obtuvo { $value }.

# Mensajes de error de Clap.
clap-error-missing-argument = Falta el argumento requerido: { $argument }
clap-error-missing-subcommand = Falta el subcomando. Opciones disponibles: { $valid_subcommands }
clap-error-unknown-argument = Argumento desconocido: { $argument }
clap-error-invalid-value = Valor no válido para { $argument }: { $value }
clap-error-invalid-subcommand = Subcomando desconocido: { $subcommand }
# Nota: value-validation usa una redacción distinta de invalid-value para
# diferenciar errores de validadores personalizados (ErrorKind::ValueValidation)
# de errores de tipo (ErrorKind::InvalidValue).
clap-error-value-validation = Validación fallida para { $argument }: { $value }

# Errores y contextos del runner.
runner.manifest.not_found = No se encontró el manifiesto '{ $manifest_name }' en { $directory }.
runner.manifest.not_found.help = Asegúrese de que el manifiesto exista o use `--file` con la ruta correcta.
runner.manifest.path_missing_name = La ruta del manifiesto '{ $path }' no tiene nombre de archivo.
runner.manifest.path_utf8 = La ruta del manifiesto '{ $path }' no es UTF-8 válida.
runner.manifest.directory_utf8 = La ruta del directorio del manifiesto '{ $path }' no es UTF-8 válida.
runner.manifest.directory_label = directorio `{ $directory }`
runner.manifest.current_directory_label = el directorio actual
runner.context.network_policy = No se pudo construir la política de red.
runner.context.load_manifest = No se pudo cargar el manifiesto en { $path }.
runner.context.serialise_manifest = No se pudo serializar el manifiesto.
runner.context.build_graph = No se pudo construir el grafo desde el manifiesto.
runner.context.generate_ninja = No se pudo generar el manifiesto Ninja.

runner.io.create_temp_file = No se pudo crear el archivo Ninja temporal.
runner.io.write_temp_ninja = No se pudo escribir el archivo Ninja temporal.
runner.io.flush_temp_ninja = No se pudo vaciar el archivo Ninja temporal.
runner.io.sync_temp_ninja = No se pudo sincronizar el archivo Ninja temporal.
runner.io.create_parent_dir = No se pudo crear el directorio padre { $path }.
runner.io.create_ninja_file = No se pudo crear el archivo Ninja en { $path }.
runner.io.write_ninja_file = No se pudo escribir el archivo Ninja en { $path }.
runner.io.flush_ninja_file = No se pudo vaciar el archivo Ninja en { $path }.
runner.io.sync_ninja_file = No se pudo sincronizar el archivo Ninja en { $path }.
runner.io.open_ambient_dir = No se pudo abrir el directorio ambiental.
runner.io.no_existing_ancestor = No hay un directorio antecesor existente para { $path }.
runner.io.derive_relative_path = No se pudo derivar la ruta relativa de Ninja.
runner.io.non_utf8_path = No se admiten rutas no UTF-8 (ruta: { $path }).
runner.io.write_stdout = No se pudo escribir el manifiesto Ninja en stdout.
runner.io.flush_stdout = No se pudo vaciar stdout.

# Diagnósticos del manifiesto.
manifest.parse = Falló el análisis del manifiesto.
manifest.structure_error = Error de estructura del manifiesto en { $name }: { $details }
manifest.yaml.parse = Error de YAML en la línea { $line }, columna { $column }: { $details }
manifest.yaml.label = YAML inválido
manifest.yaml.hint.tabs = YAML no permite tabulaciones; use espacios para la sangría.
manifest.yaml.hint.list_item = Los elementos de lista YAML deben comenzar con '-' y estar bien indentados.
manifest.yaml.hint.expected_colon = Falta un ':' después de la clave en el mapeo.
manifest.yaml.hint.mapping_values = Los mapeos YAML requieren valores después de ':'.
manifest.yaml.hint.invalid_token = Token YAML inválido o inesperado.
manifest.yaml.hint.escape = Escapa las barras inversas o elimina secuencias inválidas.
manifest.env.missing = La variable de entorno '{ $name }' no está establecida.
manifest.env.invalid_utf8 = La variable de entorno '{ $name }' contiene UTF-8 inválido.
manifest.vars.not_object = `vars` del manifiesto debe ser un mapa/objeto.
manifest.read_failed = No se pudo leer el manifiesto en { $path }.
manifest.resolve_workspace_root = No se pudo resolver la raíz del workspace.
manifest.workspace_non_utf8 = La ruta de la raíz del workspace '{ $path }' no es UTF-8 válida.
manifest.path_non_utf8 = La ruta del manifiesto '{ $manifest }' no es UTF-8 válida: { $path }.
manifest.open_workspace_failed = No se pudo abrir el workspace { $workspace } para el manifiesto { $manifest }.
manifest.foreach.not_iterable = La expresión `foreach` no es iterable.
manifest.foreach.serialise_item = No se pudo serializar el elemento de foreach.
manifest.when.empty = La expresión `when` no debe estar vacía.
manifest.when.eval_error = No se pudo evaluar la expresión `when` '{ $expr }'.
manifest.when.template_error = No se pudo renderizar la plantilla `when` '{ $expr }'.
manifest.target.vars_not_object = `vars` del objetivo debe ser un objeto, se obtuvo { $value }.
manifest.vars.entry_not_object = La entrada `vars` del manifiesto debe ser un objeto.
manifest.field_not_string = El campo '{ $field }' debe ser una cadena.
manifest.expression.parse_error = No se pudo analizar la expresión { $name }.
manifest.expression.eval_error = No se pudo evaluar la expresión { $name }.

# Diagnósticos de macros del manifiesto.
manifest.macro.signature_missing_identifier = La firma de la macro no tiene identificador.
manifest.macro.signature_missing_params = La firma de la macro no tiene parámetros.
manifest.macro.compile_failed = No se pudo compilar la macro { $name }.
manifest.macro.sequence_invalid = Las macros deben definirse como un mapa de nombres a plantillas.
manifest.macro.register_failed = No se pudieron registrar las macros del manifiesto.
manifest.macro.not_initialised = El entorno de macros no está inicializado.
manifest.macro.caller_invalid = El llamador de la macro debe ser una cadena.
manifest.macro.template_load_failed = No se pudo cargar la plantilla de macro.
manifest.macro.init_failed = No se pudo inicializar el entorno de macros.
manifest.macro.missing = Falta la macro { $name }.

# Errores de glob del manifiesto.
manifest.glob.unmatched_brace = Patrón glob inválido '{ $pattern }': llave '{ $character }' sin pareja en la posición { $position }.
manifest.glob.invalid_pattern = Patrón glob inválido '{ $pattern }': { $detail }.
manifest.glob.unknown_pattern_error = error de patrón desconocido.
manifest.glob.io_failed = Falló el glob para '{ $pattern }': { $detail }.
manifest.glob.unknown_io_error = error de E/S desconocido.

# Errores de IR.
ir.rule_not_found = No se encontró la regla '{ $rule }' referenciada por el objetivo '{ $target }'.
ir.multiple_rules = El objetivo '{ $target }' debe referenciar una sola regla, se obtuvo { $rules }.
ir.empty_rule = El objetivo '{ $target }' debe referenciar una regla.
ir.duplicate_outputs = Salidas duplicadas detectadas: { $outputs }.
ir.circular_dependency = Dependencia circular detectada: { $cycle }.
ir.action_serialisation = No se pudo serializar la acción: { $details }.
ir.invalid_command = Interpolación de comando inválida: { $snippet }.

# Errores de generación de Ninja.
ninja_gen.missing_action = Falta la acción '{ $id }' referenciada por un borde de compilación.
ninja_gen.format = No se pudo formatear la salida del manifiesto Ninja.

# Validación de patrones de host.
host_pattern.empty = El patrón de host no debe estar vacío.
host_pattern.contains_scheme = El patrón de host '{ $pattern }' no debe incluir un esquema URL.
host_pattern.contains_slash = El patrón de host '{ $pattern }' no debe incluir '/'.
host_pattern.missing_suffix = El patrón de host '{ $pattern }' debe incluir un sufijo después de '*.'.
host_pattern.empty_label = El patrón de host '{ $pattern }' contiene una etiqueta vacía.
host_pattern.invalid_chars = El patrón de host '{ $pattern }' contiene caracteres inválidos.
host_pattern.invalid_label_edge = Las etiquetas del patrón de host '{ $pattern }' no deben comenzar ni terminar con '-'.
host_pattern.label_too_long = El patrón de host '{ $pattern }' contiene una etiqueta de más de 63 caracteres.
host_pattern.too_long = El patrón de host '{ $pattern }' supera el límite de 255 caracteres.

# Política de red.
network_policy.scheme.empty = El esquema no debe estar vacío.
network_policy.scheme.invalid = El esquema '{ $scheme }' contiene caracteres inválidos.
network_policy.allowlist.empty = La lista de permitidos no debe estar vacía.
network_policy.scheme.not_allowed = El esquema '{ $scheme }' no está permitido.
network_policy.missing_host = La URL no contiene host.
network_policy.host.blocked = El host '{ $host }' está bloqueado por la política.
network_policy.host.not_allowlisted = El host '{ $host }' no está en la lista de permitidos.

# Configuración de la stdlib.
stdlib.config.default_fetch_cache_invalid = La ruta de caché de fetch por defecto debe ser relativa.
stdlib.config.default_which_cache_invalid = La capacidad de caché de which por defecto debe ser positiva.
stdlib.config.workspace_root_absolute = La ruta raíz del workspace debe ser absoluta.
stdlib.config.fetch_response_limit_positive = El límite de respuesta de fetch debe ser positivo.
stdlib.config.command_output_limit_positive = El límite de captura de salida del comando debe ser positivo.
stdlib.config.command_stream_limit_positive = El límite de transmisión del comando debe ser positivo.
stdlib.config.which_cache_capacity_positive = La capacidad de caché de which debe ser positiva.
stdlib.config.skip_dir_empty = Las entradas de directorio a omitir no deben estar vacías.
stdlib.config.skip_dir_navigation = Las entradas de directorio a omitir no deben contener '..'.
stdlib.config.skip_dir_separator = Las entradas de directorio a omitir no deben contener separadores de ruta.
stdlib.config.fetch_cache_empty = La ruta de caché de fetch no debe estar vacía.
stdlib.config.fetch_cache_not_relative = La ruta de caché de fetch debe ser relativa, se obtuvo { $path }.
stdlib.config.fetch_cache_escapes = La ruta de caché de fetch no debe salir del workspace: { $path }.

# Diagnósticos de fetch.
stdlib.fetch.url_invalid = URL inválida '{ $url }': { $details }.
stdlib.fetch.disallowed = URL '{ $url }' no permitida: { $details }.
stdlib.fetch.failed = No se pudo obtener '{ $url }': { $details }.
stdlib.fetch.cache_read_failed = No se pudo leer la entrada de caché '{ $name }': { $details }.
stdlib.fetch.cache_open_failed = No se pudo abrir la entrada de caché '{ $name }': { $details }.
stdlib.fetch.response_read_failed = No se pudo leer la respuesta de '{ $url }': { $details }.
stdlib.fetch.response_buffer_overflow = Desbordamiento del búfer al leer '{ $url }'.
stdlib.fetch.cache_write_failed = No se pudo escribir la caché para '{ $url }': { $details }.
stdlib.fetch.response_limit_exceeded = La respuesta de '{ $url }' superó el límite de { $limit } bytes.
stdlib.fetch.cache_limit_exceeded = La respuesta en caché '{ $name }' superó el límite de { $limit } bytes.
stdlib.fetch.io_failed = { $action } falló para { $path }: { $details }.
stdlib.fetch.action.sync_cache = sincronizar caché de fetch
stdlib.fetch.action.create_cache_dir = crear directorio de caché de fetch
stdlib.fetch.action.open_cache_dir = abrir directorio de caché de fetch
stdlib.fetch.action.stat_cache = consultar caché de fetch
stdlib.fetch.action.open_cache_entry = abrir entrada de caché de fetch

# Diagnósticos de comandos.
stdlib.command.location = comando '{ $command }' en la plantilla '{ $template }'
stdlib.command.spawn_failed = No se pudo iniciar { $location }: { $details }.
stdlib.command.io_failed = { $location } falló: { $details }.
stdlib.command.closed_input_early = La entrada se cerró antes de terminar de escribir al comando.
stdlib.command.broken_pipe = Tubería rota al ejecutar { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } terminó por señal.
stdlib.command.exited_with_status = { $location } salió con estado { $status }.
stdlib.command.output_limit_exceeded = { $location } superó el límite { $mode } de { $stream } de { $limit } bytes.
stdlib.command.timeout = { $location } excedió el tiempo de espera de { $seconds } segundos.
stdlib.command.exit_status_suffix = (estado de salida { $status })
stdlib.command.signal_suffix = (terminado por señal)
stdlib.command.shell.empty = El comando de shell no debe estar vacío.
stdlib.command.grep.empty_pattern = El patrón de grep no debe estar vacío.
stdlib.command.grep.flags_not_string = Las banderas de grep deben ser cadenas.
stdlib.command.quote.invalid = No se pudo entrecomillar { $arg }: { $details }.
stdlib.command.quote.line_break = Los argumentos con retornos de carro o saltos de línea no se pueden entrecomillar de forma segura.
stdlib.command.input_undefined = El valor de entrada está indefinido.
stdlib.command.tempfile.root_required = Se requiere la raíz del workspace para crear archivos temporales de comandos.
stdlib.command.tempfile.create_failed = No se pudo crear el archivo temporal de comando: { $details }.
stdlib.command.options.invalid_utf8 = La clave de opción del comando debe ser UTF-8 válida.
stdlib.command.option.mode_not_string = El modo de salida debe ser una cadena.
stdlib.command.options.invalid_type = Las opciones del comando deben ser un objeto.
stdlib.command.output.mode_unsupported = Modo de salida no compatible '{ $mode }'.
stdlib.command.output.mode.capture = captura
stdlib.command.output.mode.streaming = streaming
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnósticos de rutas.
stdlib.path.io.failed = { $action } falló para { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } falló para { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } falló para { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = no encontrado
stdlib.path.io.permission_denied = permiso denegado
stdlib.path.io.already_exists = ya existe
stdlib.path.io.invalid_input = entrada inválida
stdlib.path.io.invalid_data = datos inválidos
stdlib.path.io.timed_out = tiempo de espera agotado
stdlib.path.io.interrupted = interrumpido
stdlib.path.io.would_block = se bloquearía
stdlib.path.io.write_zero = escritura cero
stdlib.path.io.unexpected_eof = fin de archivo inesperado
stdlib.path.io.broken_pipe = tubería rota
stdlib.path.io.connection_refused = conexión rechazada
stdlib.path.io.connection_reset = conexión reiniciada
stdlib.path.io.connection_aborted = conexión abortada
stdlib.path.io.not_connected = no conectado
stdlib.path.io.addr_in_use = dirección en uso
stdlib.path.io.addr_not_available = dirección no disponible
stdlib.path.io.out_of_memory = sin memoria
stdlib.path.io.unsupported = no compatible
stdlib.path.io.file_too_large = archivo demasiado grande
stdlib.path.io.resource_busy = recurso ocupado
stdlib.path.io.executable_busy = ejecutable ocupado
stdlib.path.io.deadlock = interbloqueo
stdlib.path.io.crosses_devices = cruza dispositivos
stdlib.path.io.too_many_links = demasiados enlaces
stdlib.path.io.invalid_filename = nombre de archivo inválido
stdlib.path.io.arg_list_too_long = lista de argumentos demasiado larga
stdlib.path.io.stale_handle = identificador de red obsoleto
stdlib.path.io.storage_full = almacenamiento lleno
stdlib.path.io.not_seekable = no se puede posicionar
stdlib.path.io.network_down = red caída
stdlib.path.io.network_unreachable = red inalcanzable
stdlib.path.io.host_unreachable = host inalcanzable
stdlib.path.io.other = error de E/S
stdlib.path.action.canonicalise = canonicalizar
stdlib.path.action.open_directory = abrir directorio
stdlib.path.action.stat = consultar
stdlib.path.action.read = leer
stdlib.path.action.open_file = abrir archivo
stdlib.path.with_suffix.empty_separator = with_suffix requiere un separador no vacío.
stdlib.path.relative_to.mismatch = { $path } no es relativo a { $root }.
stdlib.path.expanduser.unsupported = La expansión ~ específica de usuario no es compatible.
stdlib.path.expanduser.no_home = No se puede expandir ~: no hay variables de entorno de hogar.
stdlib.path.contents.unsupported_encoding = Codificación no compatible '{ $encoding }'.
stdlib.path.hash.unsupported_algorithm = Algoritmo hash no compatible '{ $algorithm }'.
stdlib.path.hash.unsupported_algorithm_legacy = Algoritmo hash no compatible '{ $algorithm }' (habilite la función '{ $feature }').

# Diagnósticos de colecciones.
stdlib.collections.flatten.expected_sequence = Flatten esperaba elementos de secuencia pero encontró { $kind }.
stdlib.collections.group_by.empty_attribute = group_by requiere un atributo no vacío.
stdlib.collections.group_by.unresolved = group_by no pudo resolver '{ $attr }' en un elemento de tipo { $kind }.

# Diagnósticos de tiempo.
stdlib.time.offset.invalid = El offset de now '{ $offset }' es inválido: se esperaba '+HH:MM[:SS]' o 'Z'.
stdlib.time.timedelta.overflow = Desbordamiento de timedelta al añadir { $component }.
stdlib.time.label.weeks = semanas
stdlib.time.label.days = días
stdlib.time.label.hours = horas
stdlib.time.label.minutes = minutos
stdlib.time.label.seconds = segundos
stdlib.time.label.milliseconds = milisegundos
stdlib.time.label.microseconds = microsegundos
stdlib.time.label.nanoseconds = nanosegundos

# Diagnósticos de which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] comando '{ $command }' no encontrado tras revisar { $count } entradas PATH. Vista previa: { $preview }
stdlib.which.not_found.hint.cwd_auto = Los segmentos vacíos de PATH se ignoran; use cwd_mode="auto" para incluir el directorio de trabajo.
stdlib.which.not_found.hint.cwd_always = Establezca cwd_mode="always" para incluir el directorio actual.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] el comando '{ $command }' en '{ $path }' no existe o no es ejecutable.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <vacío>
stdlib.which.path_entry.non_utf8 = La entrada PATH #{ $index } contiene caracteres no UTF-8; Netsuke requiere rutas UTF-8.
stdlib.which.command.empty = which requiere una cadena no vacía.
stdlib.which.cwd_mode.invalid = cwd_mode debe ser 'auto', 'always' o 'never', se obtuvo '{ $mode }'.
stdlib.which.cwd.resolve_failed = No se pudo resolver el directorio actual: { $details }.
stdlib.which.cwd.non_utf8 = El directorio actual contiene componentes no UTF-8.
stdlib.which.canonicalise_failed = No se pudo canonicalizar '{ $path }': { $details }.
stdlib.which.canonicalise_non_utf8 = La ruta canónica contiene componentes no UTF-8.
stdlib.which.workspace_non_utf8 = La ruta del workspace contiene componentes no UTF-8 al resolver el comando '{ $command }': { $path }.

# Registro de la stdlib.
stdlib.register.open_dir = No se pudo abrir el directorio actual para registrar la stdlib.
stdlib.register.resolve_dir = No se pudo resolver el directorio actual para registrar la stdlib.
stdlib.register.dir_non_utf8 = El directorio actual contiene componentes no UTF-8: { $path }.

# Ejemplos de formas plurales para traductores.
# Estos mensajes demuestran la sintaxis de expresiones select de Fluent
# usando categorías plurales CLDR. Nota: Estas requieren argumentos
# FluentValue numéricos para seleccionar correctamente las variantes;
# la API de localización de netsuke actualmente pasa todos los argumentos
# como cadenas, por lo que la selección usa la variante predeterminada.
example.files_processed = { $count ->
    [one] Se procesó { $count } archivo.
   *[other] Se procesaron { $count } archivos.
}

example.errors_found = { $count ->
    [zero] No se encontraron errores.
    [one] Se encontró { $count } error.
   *[other] Se encontraron { $count } errores.
}
