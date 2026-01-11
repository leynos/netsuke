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

# Mensajes de error de Clap.
clap-error-missing-argument = Falta el argumento requerido: { $argument }
clap-error-missing-subcommand = Falta el subcomando. Opciones disponibles: { $valid_subcommands }
clap-error-unknown-argument = Argumento desconocido: { $argument }
clap-error-invalid-value = Valor no válido para { $argument }: { $value }
clap-error-invalid-subcommand = Subcomando desconocido: { $subcommand }
clap-error-value-validation = Valor no válido para { $argument }: { $value }
