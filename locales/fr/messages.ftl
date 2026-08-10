# Ressources de localisation de l'interface en ligne de commande Netsuke.

cli.about = Netsuke compile des manifestes YAML + Jinja en plans de compilation Ninja.
cli.long_about = Netsuke transforme des manifestes YAML + Jinja en graphes Ninja reproductibles et exécute Ninja avec des valeurs par défaut sûres.
cli.usage = { $usage }

# Texte d'aide des options globales.
cli.flag.file.help = Chemin du fichier manifeste Netsuke à utiliser.
cli.flag.directory.help = Exécuter comme si le démarrage avait eu lieu dans ce répertoire.
cli.flag.config.help = Chemin d'un fichier de configuration, sans passer par la détection automatique.
cli.flag.jobs.help = Définir le nombre de tâches de compilation en parallèle.
cli.flag.verbose.help = Activer les journaux de diagnostic détaillés et les résumés de durée en fin d'exécution.
cli.flag.locale.help = Étiquette de langue pour les textes de la CLI (par exemple : en-US, fr).
cli.flag.fetch_allow_scheme.help = Schémas d'URL supplémentaires autorisés pour l'assistant fetch.
cli.flag.fetch_allow_host.help = Noms d'hôte autorisés lorsque le refus par défaut est activé.
cli.flag.fetch_block_host.help = Noms d'hôte toujours bloqués, même s'ils sont autorisés par ailleurs.
cli.flag.fetch_default_deny.help = Refuser tous les hôtes par défaut ; n'autoriser que la liste déclarée.
cli.flag.json.help = Produire une sortie JSON exploitable par une machine.
cli.flag.no_input.help = Ne jamais lire d'entrée interactive.
cli.flag.color.help = Politique de couleur en sortie (auto, always, never).
cli.flag.emoji.help = Politique d'émojis (auto, always, never).
cli.flag.progress.help = Politique d'affichage de la progression (auto, always, never).
cli.flag.accessibility.help = Politique de sortie accessible (auto, on, off).
cli.flag.default_targets.help = Cibles de compilation par défaut lorsqu'aucune n'est indiquée.

# Descriptions des sous-commandes.
cli.subcommand.build.about = Compiler les cibles définies dans le manifeste (par défaut).
cli.subcommand.build.long_about = Compiler les cibles demandées ; à défaut, utiliser celles du manifeste.
cli.subcommand.clean.about = Supprimer les artefacts de compilation via Ninja.
cli.subcommand.clean.long_about = Générer un fichier Ninja temporaire, puis exécuter `ninja -t clean`.
cli.subcommand.graph.about = Émettre le graphe de dépendances de compilation. Le format par défaut est DOT.
cli.subcommand.graph.long_about = Projeter le manifeste Netsuke analysé en un graphe de compilation canonique et l'écrire au format Graphviz DOT, ou en page HTML autonome avec `--html`. Utilisez `--output <FICHIER>` pour écrire dans un fichier ; `-` écrit sur la sortie standard.
cli.subcommand.generate.about = Générer le manifeste Ninja sans exécuter Ninja.
cli.subcommand.generate.long_about = Écrire le manifeste Ninja généré sur la sortie standard, ou dans un fichier choisi avec `--output`.
cli.subcommand.help.about = Afficher l'aide de premier niveau, ou l'aide d'un sujet nommé.
cli.subcommand.help.long_about = Sans sujet, ceci correspond à `--help`. Utilisez `help targets` pour afficher le catalogue des cibles et actions du fichier sélectionné.

# Help catalogue headings and markers.
cli.help.actions_heading = Actions :
cli.help.targets_heading = Cibles :
cli.help.targets.about = Lister les cibles et actions du manifeste sélectionné.
cli.help.default_marker = par défaut

# Texte d'aide des options de la sous-commande build.
cli.subcommand.build.flag.targets.help = Cibles à compiler (utilise celles du manifeste si omis).

# Texte d'aide des options de la sous-commande graph.
cli.subcommand.graph.flag.html.help = Restituer le graphe en page HTML autonome plutôt qu'en DOT.
cli.subcommand.graph.flag.output.help = Écrire l'artefact de graphe dans FICHIER ; utilisez `-` pour la sortie standard.

# Texte d'aide des options de la sous-commande generate.
cli.subcommand.generate.flag.output.help = Écrire le manifeste Ninja généré dans FICHIER plutôt que sur la sortie standard.

# Erreurs de validation de la CLI.
cli.validation.jobs.invalid_number = { $value } n'est pas un nombre valide.
cli.validation.jobs.out_of_range = Le nombre de tâches doit être compris entre { $min } et { $max }.
cli.validation.scheme.empty = Le schéma ne doit pas être vide.
cli.validation.scheme.invalid_start = Le schéma « { $scheme } » doit commencer par une lettre ASCII.
cli.validation.scheme.invalid = Schéma non valide « { $scheme } ».
cli.validation.locale.empty = L'étiquette de langue ne doit pas être vide.
cli.validation.locale.invalid = Étiquette de langue non valide « { $locale } ».
cli.validation.color.invalid = Politique de couleur non valide « { $value } ». Options valides : auto, always, never.
cli.validation.emoji.invalid = Politique d'émojis non valide « { $value } ». Options valides : auto, always, never.
cli.validation.progress.invalid = Politique de progression non valide « { $value } ». Options valides : auto, always, never.
cli.validation.accessibility.invalid = Politique d'accessibilité non valide « { $value } ». Options valides : auto, on, off.
cli.validation.config.expected_object = Les valeurs de la CLI devaient être sérialisées en objet, reçu { $value }.

# Messages d'erreur de Clap.
clap-error-missing-argument = Argument requis manquant : { $argument }
clap-error-missing-subcommand = Sous-commande manquante. Options disponibles : { $valid_subcommands }
clap-error-unknown-argument = Argument inconnu : { $argument }
clap-error-invalid-value = Valeur non valide pour { $argument } : { $value }
clap-error-invalid-subcommand = Sous-commande inconnue : { $subcommand }
# Remarque : value-validation emploie une formulation distincte d'invalid-value
# afin de différencier les échecs de validateurs personnalisés
# (ErrorKind::ValueValidation) des incompatibilités de type
# (ErrorKind::InvalidValue).
clap-error-value-validation = Échec de la validation de { $argument } : { $value }

# Erreurs et contextes de l'exécuteur.
runner.manifest.not_found = Manifeste « { $manifest_name } » introuvable dans { $directory }.
runner.manifest.not_found.help = Vérifiez que le manifeste existe ou indiquez `--file` avec le bon chemin.
runner.manifest.path_missing_name = Le chemin de manifeste « { $path } » ne comporte pas de nom de fichier.
runner.manifest.path_utf8 = Le chemin de manifeste « { $path } » n'est pas de l'UTF-8 valide.
runner.manifest.directory_utf8 = Le chemin du répertoire de manifeste « { $path } » n'est pas de l'UTF-8 valide.
runner.manifest.directory_label = répertoire `{ $directory }`
runner.manifest.current_directory_label = le répertoire courant
runner.manifest.default_not_declared = La valeur par défaut du manifeste '{ $default }' ne désigne aucune action ni cible déclarée.
runner.context.network_policy = Impossible de construire la politique réseau.
runner.context.load_manifest = Impossible de charger le manifeste depuis { $path }.
runner.context.serialise_manifest = Impossible de sérialiser le manifeste.
runner.context.build_graph = Impossible de construire le graphe à partir du manifeste.
runner.context.generate_ninja = Impossible de générer le manifeste Ninja.
runner.context.render_graph = Impossible de restituer l'artefact de graphe.

runner.io.create_temp_file = Impossible de créer le fichier Ninja temporaire.
runner.io.write_temp_ninja = Impossible d'écrire le fichier Ninja temporaire.
runner.io.flush_temp_ninja = Impossible de vider le tampon du fichier Ninja temporaire.
runner.io.sync_temp_ninja = Impossible de synchroniser le fichier Ninja temporaire.
runner.io.create_parent_dir = Impossible de créer le répertoire parent { $path }.
runner.io.create_ninja_file = Impossible de créer le fichier Ninja dans { $path }.
runner.io.write_ninja_file = Impossible d'écrire le fichier Ninja dans { $path }.
runner.io.flush_ninja_file = Impossible de vider le tampon du fichier Ninja dans { $path }.
runner.io.sync_ninja_file = Impossible de synchroniser le fichier Ninja dans { $path }.
runner.io.open_ambient_dir = Impossible d'ouvrir le répertoire ambiant.
runner.io.no_existing_ancestor = Aucun répertoire ancêtre existant pour { $path }.
runner.io.derive_relative_path = Impossible de déduire le chemin Ninja relatif.
runner.io.non_utf8_path = Les chemins non UTF-8 ne sont pas pris en charge (chemin : { $path }).
runner.io.write_stdout = Impossible d'écrire le manifeste Ninja sur la sortie standard.
runner.io.flush_stdout = Impossible de vider le tampon de la sortie standard.

# Diagnostics du manifeste.
manifest.parse = L'analyse du manifeste a échoué.
manifest.structure_error = Erreur de structure du manifeste dans { $name } : { $details }
manifest.yaml.parse = Erreur d'analyse YAML à la ligne { $line }, colonne { $column } : { $details }
manifest.yaml.label = YAML non valide
manifest.yaml.hint.tabs = YAML n'autorise pas les tabulations ; utilisez des espaces pour l'indentation.
manifest.yaml.hint.list_item = Les éléments de liste YAML doivent commencer par « - » et être correctement indentés.
manifest.yaml.hint.expected_colon = Cela ressemble à une entrée de mappage ; il manque un « : » après la clé.
manifest.yaml.hint.mapping_values = Les mappages YAML exigent une valeur après « : » (ou un bloc imbriqué).
manifest.yaml.hint.invalid_token = Le jeton YAML est non valide ou inattendu.
manifest.yaml.hint.escape = Échappez les barres obliques inverses ou supprimez les séquences d'échappement non valides.
manifest.env.missing = Une variable d'environnement requise n'est pas définie.
manifest.env.invalid_utf8 = Une variable d'environnement contient de l'UTF-8 non valide.
manifest.vars.not_object = `vars` du manifeste doit être une table ou un objet.
manifest.vars.reserved_name = La clé `vars` '{ $name }' du manifeste est réservée à une fonction utilitaire de gabarit intégrée ; renommez la variable.
manifest.read_failed = Impossible de lire le manifeste depuis { $path }.
manifest.resolve_workspace_root = Impossible de résoudre la racine de l'espace de travail.
manifest.workspace_non_utf8 = Le chemin racine de l'espace de travail « { $path } » n'est pas de l'UTF-8 valide.
manifest.path_non_utf8 = Le chemin du manifeste « { $manifest } » n'est pas de l'UTF-8 valide : { $path }.
manifest.path_missing_name = Le chemin de manifeste « { $path } » ne comporte pas de nom de fichier.
manifest.open_workspace_failed = Impossible d'ouvrir l'espace de travail { $workspace } pour le manifeste { $manifest }.
manifest.foreach.not_iterable = L'expression `foreach` n'est pas itérable.
manifest.foreach.serialise_item = Impossible de sérialiser l'élément de `foreach`.
manifest.when.empty = L'expression `when` ne doit pas être vide.
manifest.when.eval_error = Impossible d'évaluer l'expression `when` « { $expr } ».
manifest.when.template_error = Impossible de restituer le gabarit `when` « { $expr } ».
manifest.target.vars_not_object = `vars` de la cible doit être un objet, reçu { $value }.
manifest.vars.entry_not_object = Une entrée `vars` du manifeste doit être un objet.
manifest.field_not_string = Le champ « { $field } » doit être une chaîne.
manifest.expression.parse_error = Impossible d'analyser l'expression { $name }.
manifest.expression.eval_error = Impossible d'évaluer l'expression { $name }.

# Diagnostics des macros du manifeste.
manifest.macro.signature_missing_identifier = La signature de la macro ne comporte pas d'identifiant.
manifest.macro.signature_missing_params = La signature de la macro ne comporte pas de paramètres.
manifest.macro.compile_failed = Impossible de compiler la macro { $name }.
manifest.macro.sequence_invalid = Les macros doivent être définies comme un mappage de noms vers des gabarits.
manifest.macro.register_failed = Impossible d'enregistrer les macros du manifeste.
manifest.macro.not_initialised = L'environnement de macros n'est pas initialisé.
manifest.macro.caller_invalid = L'appelant de la macro doit être une chaîne.
manifest.macro.template_load_failed = Impossible de charger le gabarit de macro.
manifest.macro.init_failed = Impossible d'initialiser l'environnement de macros.
manifest.macro.missing = La macro { $name } est absente.

# Erreurs de motifs glob du manifeste.
manifest.glob.unmatched_brace = Motif glob non valide « { $pattern } » : « { $character } » non apparié à la position { $position }.
manifest.glob.invalid_pattern = Motif glob non valide « { $pattern } » : { $detail }.
manifest.glob.unknown_pattern_error = erreur de motif inconnue.
manifest.glob.io_failed = Échec du glob pour « { $pattern } » : { $detail }.
manifest.glob.unknown_io_error = erreur d'E/S inconnue.
manifest.command_list_empty = Le champ « command » ne doit pas être vide : indiquez une chaîne de commande ou une liste non vide.

# Erreurs de la représentation intermédiaire.
ir.rule_not_found = La règle « { $rule } » référencée par la cible « { $target } » est introuvable.
ir.multiple_rules = La cible « { $target } » doit référencer une seule règle, reçu { $rules }.
ir.empty_rule = La cible « { $target } » doit référencer une règle.
ir.duplicate_outputs = Sorties en double détectées : { $outputs }.
ir.circular_dependency = Dépendance circulaire détectée : { $cycle }.
ir.action_serialisation = Impossible de sérialiser l'action : { $details }.
ir.invalid_command = Interpolation de commande non valide : { $snippet }.

# Erreurs de génération Ninja.
ninja_gen.missing_action = Action « { $id } » manquante alors qu'une arête de compilation la référence.
ninja_gen.format = Impossible de formater la sortie du manifeste Ninja.
ninja_gen.dyndep_files_required = This build requires a generated Ninja bundle; use `netsuke build`, `netsuke clean`, or `netsuke generate` so the dyndep files are materialized.
ninja_gen.reserved_output_path = The path '{ $path }' is reserved for Netsuke's serial dependency state.

# Validation des motifs d'hôte.
host_pattern.empty = Le motif d'hôte ne doit pas être vide.
host_pattern.contains_scheme = Le motif d'hôte « { $pattern } » ne doit pas inclure de schéma d'URL.
host_pattern.contains_slash = Le motif d'hôte « { $pattern } » ne doit pas contenir « / ».
host_pattern.missing_suffix = Le motif d'hôte « { $pattern } » doit comporter un suffixe après « *. ».
host_pattern.empty_label = Le motif d'hôte « { $pattern } » contient une étiquette vide.
host_pattern.invalid_chars = Le motif d'hôte « { $pattern } » contient des caractères non valides.
host_pattern.invalid_label_edge = Les étiquettes du motif d'hôte « { $pattern } » ne doivent ni commencer ni finir par « - ».
host_pattern.label_too_long = Le motif d'hôte « { $pattern } » contient une étiquette de plus de 63 caractères.
host_pattern.too_long = Le motif d'hôte « { $pattern } » dépasse la limite de 255 caractères.

# Politique réseau.
network_policy.scheme.empty = Le schéma ne doit pas être vide.
network_policy.scheme.invalid = Le schéma « { $scheme } » contient des caractères non valides.
network_policy.allowlist.empty = La liste d'hôtes autorisés ne doit pas être vide.
network_policy.scheme.not_allowed = Le schéma « { $scheme } » n'est pas autorisé.
network_policy.missing_host = L'URL ne comporte pas d'hôte.
network_policy.host.blocked = L'hôte « { $host } » est bloqué par la politique.
network_policy.host.not_allowlisted = L'hôte « { $host } » ne figure pas dans la liste des hôtes autorisés.

# Configuration de la bibliothèque standard.
stdlib.config.default_fetch_cache_invalid = Le chemin de cache fetch par défaut doit être relatif.
stdlib.config.default_which_cache_invalid = La capacité de cache which par défaut doit être positive.
stdlib.config.workspace_root_absolute = Le chemin racine de l'espace de travail doit être absolu.
stdlib.config.fetch_response_limit_positive = La limite de réponse de fetch doit être positive.
stdlib.config.command_output_limit_positive = La limite de capture de sortie des commandes doit être positive.
stdlib.config.command_stream_limit_positive = La limite de flux des commandes doit être positive.
stdlib.config.which_cache_capacity_positive = La capacité du cache which doit être positive.
stdlib.config.skip_dir_empty = Les entrées de répertoires à ignorer ne doivent pas être vides.
stdlib.config.skip_dir_navigation = Les entrées de répertoires à ignorer ne doivent pas contenir « .. ».
stdlib.config.skip_dir_separator = Les entrées de répertoires à ignorer ne doivent pas contenir de séparateurs de chemin.
stdlib.config.fetch_cache_empty = Le chemin de cache fetch ne doit pas être vide.
stdlib.config.fetch_cache_not_relative = Le chemin de cache fetch doit être relatif, reçu { $path }.
stdlib.config.fetch_cache_escapes = Le chemin de cache fetch ne doit pas sortir de l'espace de travail : { $path }.
stdlib.config.open_workspace_root = Impossible d'ouvrir le répertoire courant comme racine de l'espace de travail stdlib.
stdlib.config.resolve_cwd = Impossible de résoudre le répertoire courant comme racine de l'espace de travail stdlib.
stdlib.config.cwd_non_utf8 = Le répertoire courant contient des composants non UTF-8 : { $path }.

# Diagnostics de l'assistant fetch.
stdlib.fetch.url_invalid = URL non valide « { $url } » : { $details }.
stdlib.fetch.disallowed = L'URL « { $url } » n'est pas autorisée : { $details }.
stdlib.fetch.failed = Impossible de récupérer « { $url } » : { $details }.
stdlib.fetch.cache_read_failed = Impossible de lire l'entrée de cache « { $name } » : { $details }.
stdlib.fetch.cache_open_failed = Impossible d'ouvrir l'entrée de cache « { $name } » : { $details }.
stdlib.fetch.response_read_failed = Impossible de lire la réponse de « { $url } » : { $details }.
stdlib.fetch.response_buffer_overflow = Débordement du tampon lors de la lecture de « { $url } ».
stdlib.fetch.cache_write_failed = Impossible d'écrire le cache pour « { $url } » : { $details }.
stdlib.fetch.response_limit_exceeded = La réponse de « { $url } » a dépassé la limite de { $limit } octets.
stdlib.fetch.cache_limit_exceeded = La réponse en cache « { $name } » a dépassé la limite de { $limit } octets.
stdlib.fetch.io_failed = { $action } a échoué pour { $path } : { $details }.
stdlib.fetch.action.sync_cache = synchroniser le cache fetch
stdlib.fetch.action.create_cache_dir = créer le répertoire de cache fetch
stdlib.fetch.action.open_cache_dir = ouvrir le répertoire de cache fetch
stdlib.fetch.action.stat_cache = interroger l'entrée de cache fetch
stdlib.fetch.action.open_cache_entry = ouvrir l'entrée de cache fetch

# Diagnostics de l'assistant de commandes.
stdlib.command.location = commande « { $command } » dans le gabarit « { $template } »
stdlib.command.spawn_failed = Impossible de lancer { $location } : { $details }.
stdlib.command.io_failed = { $location } a échoué : { $details }.
stdlib.command.closed_input_early = L'entrée s'est fermée avant la fin de l'écriture vers la commande.
stdlib.command.broken_pipe = Tube rompu lors de l'exécution de { $location } : { $details }.
stdlib.command.terminated_by_signal = { $location } a été arrêté par un signal.
stdlib.command.exited_with_status = { $location } s'est terminé avec le statut { $status }.
stdlib.command.output_limit_exceeded = { $location } a dépassé la limite { $mode } de { $limit } octets pour { $stream }.
stdlib.command.timeout = { $location } a dépassé le délai de { $seconds } secondes.
stdlib.command.exit_status_suffix = (statut de sortie { $status })
stdlib.command.signal_suffix = (arrêté par un signal)
stdlib.command.shell.empty = La commande shell ne doit pas être vide.
stdlib.command.grep.empty_pattern = Le motif grep ne doit pas être vide.
stdlib.command.grep.flags_not_string = Les options de grep doivent être des chaînes.
stdlib.command.quote.invalid = Impossible de protéger { $arg } par des guillemets : { $details }.
stdlib.command.quote.line_break = Les arguments contenant des retours chariot ou des sauts de ligne ne peuvent pas être protégés sans risque.
stdlib.command.input_undefined = La valeur d'entrée est indéfinie.
stdlib.command.tempfile.root_required = La racine de l'espace de travail est requise pour créer des fichiers temporaires de commande.
stdlib.command.tempfile.create_failed = Impossible de créer le fichier temporaire de commande : { $details }.
stdlib.command.options.invalid_utf8 = La clé d'une option de commande doit être de l'UTF-8 valide.
stdlib.command.option.mode_not_string = Le mode de sortie doit être une chaîne.
stdlib.command.options.invalid_type = Les options de commande doivent former un objet.
stdlib.command.output.mode_unsupported = Mode de sortie non pris en charge « { $mode } ».
stdlib.command.output.mode.capture = capture
stdlib.command.output.mode.streaming = flux
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnostics de l'assistant de chemins.
stdlib.path.io.failed = { $action } a échoué pour { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } a échoué pour { $path } : { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } a échoué pour { $path } ({ $label }) : { $detail }.
stdlib.path.io.not_found = introuvable
stdlib.path.io.permission_denied = permission refusée
stdlib.path.io.already_exists = existe déjà
stdlib.path.io.invalid_input = entrée non valide
stdlib.path.io.invalid_data = données non valides
stdlib.path.io.timed_out = délai dépassé
stdlib.path.io.interrupted = interrompu
stdlib.path.io.would_block = bloquerait
stdlib.path.io.write_zero = écriture nulle
stdlib.path.io.unexpected_eof = fin de fichier inattendue
stdlib.path.io.broken_pipe = tube rompu
stdlib.path.io.connection_refused = connexion refusée
stdlib.path.io.connection_reset = connexion réinitialisée
stdlib.path.io.connection_aborted = connexion abandonnée
stdlib.path.io.not_connected = non connecté
stdlib.path.io.addr_in_use = adresse déjà utilisée
stdlib.path.io.addr_not_available = adresse non disponible
stdlib.path.io.out_of_memory = mémoire insuffisante
stdlib.path.io.unsupported = non pris en charge
stdlib.path.io.file_too_large = fichier trop volumineux
stdlib.path.io.resource_busy = ressource occupée
stdlib.path.io.executable_busy = exécutable occupé
stdlib.path.io.deadlock = interblocage
stdlib.path.io.crosses_devices = franchit des périphériques
stdlib.path.io.too_many_links = trop de liens
stdlib.path.io.invalid_filename = nom de fichier non valide
stdlib.path.io.arg_list_too_long = liste d'arguments trop longue
stdlib.path.io.stale_handle = descripteur de fichier réseau périmé
stdlib.path.io.storage_full = stockage saturé
stdlib.path.io.not_seekable = positionnement impossible
stdlib.path.io.network_down = réseau hors service
stdlib.path.io.network_unreachable = réseau injoignable
stdlib.path.io.host_unreachable = hôte injoignable
stdlib.path.io.other = erreur d'E/S
stdlib.path.action.canonicalize = canonicaliser
stdlib.path.action.open_directory = ouvrir le répertoire
stdlib.path.action.stat = interroger
stdlib.path.action.read = lire
stdlib.path.action.open_file = ouvrir le fichier
stdlib.path.with_suffix.empty_separator = with_suffix exige un séparateur non vide.
stdlib.path.relative_to.mismatch = { $path } n'est pas relatif à { $root }.
stdlib.path.expanduser.unsupported = L'expansion de ~ propre à un utilisateur n'est pas prise en charge.
stdlib.path.expanduser.no_home = Impossible d'étendre ~ : aucune variable d'environnement de répertoire personnel n'est définie.
stdlib.path.contents.unsupported_encoding = Encodage non pris en charge « { $encoding } ».
stdlib.path.hash.unsupported_algorithm = Algorithme de hachage non pris en charge « { $algorithm } ».
stdlib.path.hash.unsupported_algorithm_legacy = Algorithme de hachage non pris en charge « { $algorithm } » (activez la fonctionnalité « { $feature } »).

# Diagnostics des assistants de collections.
stdlib.collections.flatten.expected_sequence = flatten attendait des éléments de séquence mais a trouvé { $kind }.
stdlib.collections.group_by.empty_attribute = group_by exige un attribut non vide.
stdlib.collections.group_by.unresolved = group_by n'a pas pu résoudre « { $attr } » sur un élément de type { $kind }.

# Diagnostics des assistants temporels.
stdlib.time.offset.invalid = Le décalage de now « { $offset } » est non valide : « +HH:MM[:SS] » ou « Z » était attendu.
stdlib.time.timedelta.overflow = Débordement de timedelta lors de l'ajout de { $component }.
stdlib.time.label.weeks = semaines
stdlib.time.label.days = jours
stdlib.time.label.hours = heures
stdlib.time.label.minutes = minutes
stdlib.time.label.seconds = secondes
stdlib.time.label.milliseconds = millisecondes
stdlib.time.label.microseconds = microsecondes
stdlib.time.label.nanoseconds = nanosecondes

# Diagnostics de l'assistant which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] commande « { $command } » introuvable après examen de { $count } entrées de PATH. Aperçu : { $preview }
stdlib.which.not_found.hint.cwd_auto = Les segments vides de PATH sont ignorés ; utilisez cwd_mode="auto" pour inclure le répertoire de travail.
stdlib.which.not_found.hint.cwd_always = Définissez cwd_mode="always" pour inclure le répertoire courant.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] la commande « { $command } » située dans « { $path } » est absente ou non exécutable.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <vide>
stdlib.which.path_entry.non_utf8 = L'entrée PATH nº { $index } contient des caractères non UTF-8 ; Netsuke exige des chemins UTF-8.
stdlib.which.command.empty = which exige une chaîne non vide.
stdlib.which.cwd_mode.invalid = cwd_mode doit valoir « auto », « always » ou « never », reçu « { $mode } ».
stdlib.which.cwd.resolve_failed = Impossible de résoudre le répertoire courant : { $details }.
stdlib.which.cwd.non_utf8 = Le répertoire courant contient des composants non UTF-8.
stdlib.which.canonicalize_failed = Impossible de canonicaliser « { $path } » : { $details }.
stdlib.which.is_executable = Impossible de déterminer si « { $path } » est exécutable : { $details }.
stdlib.which.canonicalize_non_utf8 = Le chemin canonique contient des composants non UTF-8.
stdlib.which.workspace_non_utf8 = Le chemin de l'espace de travail contient des composants non UTF-8 lors de la résolution de la commande « { $command } » : { $path }.
stdlib.which.walkdir_error = Erreur de parcours de l'espace de travail pendant la résolution de la commande : { $details }.

# Enregistrement de la bibliothèque standard.
stdlib.register.open_dir = Impossible d'ouvrir le répertoire courant pour l'enregistrement de la stdlib.
stdlib.register.resolve_dir = Impossible de résoudre le répertoire courant pour l'enregistrement de la stdlib.
stdlib.register.dir_non_utf8 = Le répertoire courant contient des composants non UTF-8 : { $path }.

# Compte rendu d'état pour le mode de sortie accessible.
status.state.pending = en attente
status.state.running = en cours
status.state.done = terminée
status.state.failed = échouée
status.stage.label = Étape { $current }/{ $total } : { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Tâche { $current }/{ $total }
status.task.progress_update = { $task } : { $description }
status.stage.manifest_ingestion = Lecture du fichier manifeste
status.stage.initial_yaml_parsing = Analyse du document YAML
status.stage.template_expansion = Expansion des directives de gabarit
status.stage.final_rendering = Désérialisation et rendu des valeurs du manifeste
status.stage.ir_generation_validation = Construction et validation du graphe de dépendances
status.stage.ninja_synthesis = Synthèse du plan de compilation Ninja
status.stage.ninja_synthesis_execute = Synthèse du plan Ninja et exécution de { $tool }
status.stage.graph_rendering = Rendu de l'artefact de graphe
status.stage.graph_rendering_with_tool = Rendu de { $tool }
status.complete = { $tool } : opération terminée.
status.timing.summary_header = Résumé des durées par étape :
status.timing.stage_line = - { $label } : { $duration }
status.timing.total_line = Durée totale du pipeline : { $duration }
status.tool.build = Compilation
status.tool.clean = Nettoyage
status.tool.graph = Graphe
status.tool.graph_html = Graphe (HTML)
status.tool.generate = Génération
status.tool.help_targets = Aide sur les cibles

# Chaînes du moteur de rendu HTML du graphe.
graph.html.title = Graphe de compilation Netsuke
graph.html.heading = Graphe de compilation Netsuke
graph.html.description = Graphe de compilation restitué par Netsuke
graph.html.outline.summary = Cibles et dépendances (plan textuel)
graph.html.outline.no_inputs = Aucune entrée
graph.html.noscript.notice = JavaScript est désactivé. Le plan textuel ci-dessus contient le graphe complet ; la source DOT suit.

# Préfixes sémantiques pour la sortie accessible.
semantic.prefix.error = Erreur :
semantic.prefix.warning = Avertissement :
semantic.prefix.success = Succès :
semantic.prefix.info = Info :
semantic.prefix.timing = Durée :
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Exemples de formes plurielles pour les traducteurs.
# Le français utilise les catégories CLDR `one` et `other`, mais `one` couvre
# aussi zéro : « 0 fichier traité » reste au singulier.
example.files_processed = { $count ->
    [one] { $count } fichier traité.
   *[other] { $count } fichiers traités.
}

example.errors_found = { $count ->
    [0] Aucune erreur trouvée.
    [one] { $count } erreur trouvée.
   *[other] { $count } erreurs trouvées.
}

