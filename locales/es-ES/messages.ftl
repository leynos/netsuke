# Recursos de localización para la CLI de Netsuke.

cli.about = Netsuke compila manifiestos YAML + Jinja en planes de compilación Ninja.
cli.long_about = Netsuke transforma manifiestos YAML + Jinja en grafos Ninja reproducibles y ejecuta Ninja con valores seguros.
cli.usage = { $usage }

cli.subcommand.build.about = Compila objetivos definidos en el manifiesto (predeterminado).
cli.subcommand.build.long_about = Compila los objetivos solicitados; si no se indican, usa los predeterminados del manifiesto.
cli.subcommand.clean.about = Elimina artefactos de compilación mediante Ninja.
cli.subcommand.clean.long_about = Genera un archivo Ninja temporal y ejecuta `ninja -t clean`.
cli.subcommand.graph.about = Emite el grafo de dependencias en formato DOT.
cli.subcommand.graph.long_about = Genera un archivo Ninja temporal y ejecuta `ninja -t graph` para emitir DOT.
cli.subcommand.manifest.about = Escribe el manifiesto Ninja sin ejecutar Ninja.
cli.subcommand.manifest.long_about = Genera el archivo Ninja y lo escribe en la ruta indicada o '-' para stdout.

clap-error-missing-argument = Falta el argumento requerido: { $argument }
clap-error-missing-subcommand = Falta el subcomando. Opciones disponibles: { $valid_subcommands }
clap-error-unknown-argument = Argumento desconocido: { $argument }
clap-error-invalid-value = Valor no válido para { $argument }: { $value }
clap-error-invalid-subcommand = Subcomando desconocido: { $subcommand }
clap-error-value-validation = Valor no válido para { $argument }: { $value }
