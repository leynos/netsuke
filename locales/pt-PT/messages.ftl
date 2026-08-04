# Recursos de localização para a CLI do Netsuke (português europeu).

cli.about = O Netsuke compila manifestos YAML + Jinja em planos de compilação Ninja.
cli.long_about = O Netsuke transforma manifestos YAML + Jinja em grafos Ninja reprodutíveis e executa o Ninja com predefinições seguras.
cli.usage = { $usage }

# Texto de ajuda das opções globais.
cli.flag.file.help = Caminho para o ficheiro de manifesto do Netsuke a utilizar.
cli.flag.directory.help = Executar como se tivesse sido iniciado nesta pasta.
cli.flag.config.help = Caminho para um ficheiro de configuração, ignorando a deteção automática.
cli.flag.jobs.help = Definir o número de tarefas de compilação em paralelo.
cli.flag.verbose.help = Ativar registos de diagnóstico detalhados e resumos de tempos no final.
cli.flag.locale.help = Etiqueta de idioma para os textos da CLI (por exemplo: en-US, pt-PT).
cli.flag.fetch_allow_scheme.help = Esquemas de URL adicionais permitidos para o auxiliar fetch.
cli.flag.fetch_allow_host.help = Nomes de anfitrião permitidos quando a recusa predefinida está ativa.
cli.flag.fetch_block_host.help = Nomes de anfitrião sempre bloqueados, mesmo que permitidos noutro local.
cli.flag.fetch_default_deny.help = Recusar todos os anfitriões por predefinição; permitir apenas a lista declarada.
cli.flag.json.help = Produzir saída JSON legível por máquinas.
cli.flag.no_input.help = Nunca ler entrada interativa.
cli.flag.color.help = Política de cor na saída (auto, always, never).
cli.flag.emoji.help = Política de emojis (auto, always, never).
cli.flag.progress.help = Política de apresentação do progresso (auto, always, never).
cli.flag.accessibility.help = Política de saída acessível (auto, on, off).
cli.flag.default_targets.help = Alvos de compilação predefinidos quando nenhum é indicado.

# Descrições dos subcomandos.
cli.subcommand.build.about = Compilar os alvos definidos no manifesto (predefinição).
cli.subcommand.build.long_about = Compilar os alvos pedidos; se nenhum for indicado, usar os predefinidos do manifesto.
cli.subcommand.clean.about = Remover os artefactos de compilação através do Ninja.
cli.subcommand.clean.long_about = Gerar um ficheiro Ninja temporário e depois executar `ninja -t clean`.
cli.subcommand.graph.about = Emitir o grafo de dependências de compilação. O formato predefinido é DOT.
cli.subcommand.graph.long_about = Projetar o manifesto do Netsuke analisado num grafo de compilação canónico e escrevê-lo como Graphviz DOT, ou como página HTML autónoma com `--html`. Use `--output <FICHEIRO>` para escrever num ficheiro; `-` escreve no stdout.
cli.subcommand.generate.about = Gerar o manifesto Ninja sem executar o Ninja.
cli.subcommand.generate.long_about = Escrever o manifesto Ninja gerado no stdout ou num ficheiro escolhido com `--output`.

# Texto de ajuda das opções do subcomando build.
cli.subcommand.build.flag.targets.help = Alvos a compilar (se omitido, usa os predefinidos do manifesto).

# Texto de ajuda das opções do subcomando graph.
cli.subcommand.graph.flag.html.help = Representar o grafo como página HTML autónoma em vez de DOT.
cli.subcommand.graph.flag.output.help = Escrever o artefacto do grafo em FICHEIRO; use `-` para o stdout.

# Texto de ajuda das opções do subcomando generate.
cli.subcommand.generate.flag.output.help = Escrever o manifesto Ninja gerado em FICHEIRO em vez do stdout.

# Erros de validação da CLI.
cli.validation.jobs.invalid_number = { $value } não é um número válido.
cli.validation.jobs.out_of_range = O número de tarefas tem de estar entre { $min } e { $max }.
cli.validation.scheme.empty = O esquema não pode estar vazio.
cli.validation.scheme.invalid_start = O esquema «{ $scheme }» tem de começar por uma letra ASCII.
cli.validation.scheme.invalid = Esquema inválido «{ $scheme }».
cli.validation.locale.empty = A etiqueta de idioma não pode estar vazia.
cli.validation.locale.invalid = Etiqueta de idioma inválida «{ $locale }».
cli.validation.color.invalid = Política de cor inválida «{ $value }». Opções válidas: auto, always, never.
cli.validation.emoji.invalid = Política de emojis inválida «{ $value }». Opções válidas: auto, always, never.
cli.validation.progress.invalid = Política de progresso inválida «{ $value }». Opções válidas: auto, always, never.
cli.validation.accessibility.invalid = Política de acessibilidade inválida «{ $value }». Opções válidas: auto, on, off.
cli.validation.config.expected_object = Esperava-se que os valores da CLI fossem serializados como objeto, obteve-se { $value }.

# Mensagens de erro do Clap.
clap-error-missing-argument = Falta um argumento obrigatório: { $argument }
clap-error-missing-subcommand = Falta o subcomando. Opções disponíveis: { $valid_subcommands }
clap-error-unknown-argument = Argumento desconhecido: { $argument }
clap-error-invalid-value = Valor inválido para { $argument }: { $value }
clap-error-invalid-subcommand = Subcomando desconhecido: { $subcommand }
# Nota: value-validation usa uma formulação distinta de invalid-value para
# diferenciar falhas de validadores personalizados
# (ErrorKind::ValueValidation) de incompatibilidades de tipo
# (ErrorKind::InvalidValue).
clap-error-value-validation = A validação falhou para { $argument }: { $value }

# Erros e contextos do executor.
runner.manifest.not_found = Manifesto «{ $manifest_name }» não encontrado em { $directory }.
runner.manifest.not_found.help = Confirme que o manifesto existe ou indique `--file` com o caminho correto.
runner.manifest.path_missing_name = O caminho do manifesto «{ $path }» não tem nome de ficheiro.
runner.manifest.path_utf8 = O caminho do manifesto «{ $path }» não é UTF-8 válido.
runner.manifest.directory_utf8 = O caminho da pasta do manifesto «{ $path }» não é UTF-8 válido.
runner.manifest.directory_label = pasta `{ $directory }`
runner.manifest.current_directory_label = a pasta atual
runner.context.network_policy = Não foi possível construir a política de rede.
runner.context.load_manifest = Não foi possível carregar o manifesto em { $path }.
runner.context.serialise_manifest = Não foi possível serializar o manifesto.
runner.context.build_graph = Não foi possível construir o grafo a partir do manifesto.
runner.context.generate_ninja = Não foi possível gerar o manifesto Ninja.
runner.context.render_graph = Não foi possível representar o artefacto do grafo.

runner.io.create_temp_file = Não foi possível criar o ficheiro Ninja temporário.
runner.io.write_temp_ninja = Não foi possível escrever o ficheiro Ninja temporário.
runner.io.flush_temp_ninja = Não foi possível esvaziar o buffer do ficheiro Ninja temporário.
runner.io.sync_temp_ninja = Não foi possível sincronizar o ficheiro Ninja temporário.
runner.io.create_parent_dir = Não foi possível criar a pasta principal { $path }.
runner.io.create_ninja_file = Não foi possível criar o ficheiro Ninja em { $path }.
runner.io.write_ninja_file = Não foi possível escrever o ficheiro Ninja em { $path }.
runner.io.flush_ninja_file = Não foi possível esvaziar o buffer do ficheiro Ninja em { $path }.
runner.io.sync_ninja_file = Não foi possível sincronizar o ficheiro Ninja em { $path }.
runner.io.open_ambient_dir = Não foi possível abrir a pasta do ambiente.
runner.io.no_existing_ancestor = Não existe nenhuma pasta ascendente para { $path }.
runner.io.derive_relative_path = Não foi possível derivar o caminho Ninja relativo.
runner.io.non_utf8_path = Não são suportados caminhos que não sejam UTF-8 (caminho: { $path }).
runner.io.write_stdout = Não foi possível escrever o manifesto Ninja no stdout.
runner.io.flush_stdout = Não foi possível esvaziar o buffer do stdout.

# Diagnósticos do manifesto.
manifest.parse = A análise do manifesto falhou.
manifest.structure_error = Erro de estrutura do manifesto em { $name }: { $details }
manifest.yaml.parse = Erro de análise YAML na linha { $line }, coluna { $column }: { $details }
manifest.yaml.label = YAML inválido
manifest.yaml.hint.tabs = O YAML não permite tabulações; use espaços na indentação.
manifest.yaml.hint.list_item = Os itens de lista YAML têm de começar por «-» e estar corretamente indentados.
manifest.yaml.hint.expected_colon = Isto parece uma entrada de mapeamento; falta «:» depois da chave.
manifest.yaml.hint.mapping_values = Os mapeamentos YAML exigem um valor depois de «:» (ou um bloco aninhado).
manifest.yaml.hint.invalid_token = O símbolo YAML é inválido ou inesperado.
manifest.yaml.hint.escape = Faça o escape das barras invertidas ou remova as sequências de escape inválidas.
manifest.env.missing = A variável de ambiente obrigatória «{ $name }» não está definida.
manifest.env.invalid_utf8 = A variável de ambiente «{ $name }» contém UTF-8 inválido.
manifest.vars.not_object = `vars` do manifesto tem de ser um mapa ou objeto.
manifest.read_failed = Não foi possível ler o manifesto em { $path }.
manifest.resolve_workspace_root = Não foi possível resolver a raiz da área de trabalho.
manifest.workspace_non_utf8 = O caminho de raiz da área de trabalho «{ $path }» não é UTF-8 válido.
manifest.path_non_utf8 = O caminho do manifesto «{ $manifest }» não é UTF-8 válido: { $path }.
manifest.path_missing_name = O caminho do manifesto «{ $path }» não tem nome de ficheiro.
manifest.open_workspace_failed = Não foi possível abrir a área de trabalho { $workspace } para o manifesto { $manifest }.
manifest.foreach.not_iterable = A expressão `foreach` não é iterável.
manifest.foreach.serialise_item = Não foi possível serializar o item de `foreach`.
manifest.when.empty = A expressão `when` não pode estar vazia.
manifest.when.eval_error = Não foi possível avaliar a expressão `when` «{ $expr }».
manifest.when.template_error = Não foi possível representar o modelo `when` «{ $expr }».
manifest.target.vars_not_object = `vars` do alvo tem de ser um objeto, obteve-se { $value }.
manifest.vars.entry_not_object = Uma entrada `vars` do manifesto tem de ser um objeto.
manifest.field_not_string = O campo «{ $field }» tem de ser uma cadeia de carateres.
manifest.expression.parse_error = Não foi possível analisar a expressão { $name }.
manifest.expression.eval_error = Não foi possível avaliar a expressão { $name }.

# Diagnósticos das macros do manifesto.
manifest.macro.signature_missing_identifier = Falta um identificador na assinatura da macro.
manifest.macro.signature_missing_params = Faltam parâmetros na assinatura da macro.
manifest.macro.compile_failed = Não foi possível compilar a macro { $name }.
manifest.macro.sequence_invalid = As macros têm de ser definidas como um mapeamento de nomes para modelos.
manifest.macro.register_failed = Não foi possível registar as macros do manifesto.
manifest.macro.not_initialised = O ambiente de macros não está inicializado.
manifest.macro.caller_invalid = O invocador da macro tem de ser uma cadeia de carateres.
manifest.macro.template_load_failed = Não foi possível carregar o modelo da macro.
manifest.macro.init_failed = Não foi possível inicializar o ambiente de macros.
manifest.macro.missing = Falta a macro { $name }.

# Erros de glob do manifesto.
manifest.glob.unmatched_brace = Padrão glob inválido «{ $pattern }»: «{ $character }» sem correspondência na posição { $position }.
manifest.glob.invalid_pattern = Padrão glob inválido «{ $pattern }»: { $detail }.
manifest.glob.unknown_pattern_error = erro de padrão desconhecido.
manifest.glob.io_failed = O glob falhou para «{ $pattern }»: { $detail }.
manifest.glob.unknown_io_error = erro de E/S desconhecido.

# Erros da representação intermédia.
ir.rule_not_found = A regra «{ $rule }» referenciada pelo alvo «{ $target }» não foi encontrada.
ir.multiple_rules = O alvo «{ $target }» tem de referenciar uma única regra, obteve-se { $rules }.
ir.empty_rule = O alvo «{ $target }» tem de referenciar uma regra.
ir.duplicate_outputs = Foram detetadas saídas duplicadas: { $outputs }.
ir.circular_dependency = Foi detetada uma dependência circular: { $cycle }.
ir.action_serialisation = Não foi possível serializar a ação: { $details }.
ir.invalid_command = Interpolação de comando inválida: { $snippet }.

# Erros de geração do Ninja.
ninja_gen.missing_action = Falta a ação «{ $id }» referenciada por uma aresta de compilação.
ninja_gen.format = Não foi possível formatar a saída do manifesto Ninja.

# Validação de padrões de anfitrião.
host_pattern.empty = O padrão de anfitrião não pode estar vazio.
host_pattern.contains_scheme = O padrão de anfitrião «{ $pattern }» não pode incluir um esquema de URL.
host_pattern.contains_slash = O padrão de anfitrião «{ $pattern }» não pode incluir «/».
host_pattern.missing_suffix = O padrão de anfitrião «{ $pattern }» tem de incluir um sufixo depois de «*.».
host_pattern.empty_label = O padrão de anfitrião «{ $pattern }» contém uma etiqueta vazia.
host_pattern.invalid_chars = O padrão de anfitrião «{ $pattern }» contém carateres inválidos.
host_pattern.invalid_label_edge = As etiquetas do padrão de anfitrião «{ $pattern }» não podem começar nem terminar por «-».
host_pattern.label_too_long = O padrão de anfitrião «{ $pattern }» contém uma etiqueta com mais de 63 carateres.
host_pattern.too_long = O padrão de anfitrião «{ $pattern }» excede o limite de 255 carateres.

# Política de rede.
network_policy.scheme.empty = O esquema não pode estar vazio.
network_policy.scheme.invalid = O esquema «{ $scheme }» contém carateres inválidos.
network_policy.allowlist.empty = A lista de anfitriões permitidos não pode estar vazia.
network_policy.scheme.not_allowed = O esquema «{ $scheme }» não é permitido.
network_policy.missing_host = Falta o anfitrião no URL.
network_policy.host.blocked = O anfitrião «{ $host }» está bloqueado pela política.
network_policy.host.not_allowlisted = O anfitrião «{ $host }» não consta da lista de permitidos.

# Configuração da biblioteca padrão.
stdlib.config.default_fetch_cache_invalid = O caminho predefinido da cache do fetch tem de ser relativo.
stdlib.config.default_which_cache_invalid = A capacidade predefinida da cache do which tem de ser positiva.
stdlib.config.workspace_root_absolute = O caminho de raiz da área de trabalho tem de ser absoluto.
stdlib.config.fetch_response_limit_positive = O limite de resposta do fetch tem de ser positivo.
stdlib.config.command_output_limit_positive = O limite de captura da saída dos comandos tem de ser positivo.
stdlib.config.command_stream_limit_positive = O limite de fluxo dos comandos tem de ser positivo.
stdlib.config.which_cache_capacity_positive = A capacidade da cache do which tem de ser positiva.
stdlib.config.skip_dir_empty = As entradas de pastas a ignorar não podem estar vazias.
stdlib.config.skip_dir_navigation = As entradas de pastas a ignorar não podem conter «..».
stdlib.config.skip_dir_separator = As entradas de pastas a ignorar não podem conter separadores de caminho.
stdlib.config.fetch_cache_empty = O caminho da cache do fetch não pode estar vazio.
stdlib.config.fetch_cache_not_relative = O caminho da cache do fetch tem de ser relativo, obteve-se { $path }.
stdlib.config.fetch_cache_escapes = O caminho da cache do fetch não pode sair da área de trabalho: { $path }.
stdlib.config.open_workspace_root = Não foi possível abrir a pasta atual como raiz da área de trabalho da stdlib.
stdlib.config.resolve_cwd = Não foi possível resolver a pasta atual como raiz da área de trabalho da stdlib.
stdlib.config.cwd_non_utf8 = A pasta atual contém componentes que não são UTF-8: { $path }.

# Diagnósticos do auxiliar fetch.
stdlib.fetch.url_invalid = URL inválido «{ $url }»: { $details }.
stdlib.fetch.disallowed = O URL «{ $url }» não é permitido: { $details }.
stdlib.fetch.failed = Não foi possível obter «{ $url }»: { $details }.
stdlib.fetch.cache_read_failed = Não foi possível ler a entrada de cache «{ $name }»: { $details }.
stdlib.fetch.cache_open_failed = Não foi possível abrir a entrada de cache «{ $name }»: { $details }.
stdlib.fetch.response_read_failed = Não foi possível ler a resposta de «{ $url }»: { $details }.
stdlib.fetch.response_buffer_overflow = Sobrecarga do buffer ao ler «{ $url }».
stdlib.fetch.cache_write_failed = Não foi possível escrever a cache para «{ $url }»: { $details }.
stdlib.fetch.response_limit_exceeded = A resposta de «{ $url }» excedeu o limite de { $limit } bytes.
stdlib.fetch.cache_limit_exceeded = A resposta em cache «{ $name }» excedeu o limite de { $limit } bytes.
stdlib.fetch.io_failed = { $action } falhou para { $path }: { $details }.
stdlib.fetch.action.sync_cache = sincronizar a cache do fetch
stdlib.fetch.action.create_cache_dir = criar a pasta de cache do fetch
stdlib.fetch.action.open_cache_dir = abrir a pasta de cache do fetch
stdlib.fetch.action.stat_cache = consultar a entrada de cache do fetch
stdlib.fetch.action.open_cache_entry = abrir a entrada de cache do fetch

# Diagnósticos do auxiliar de comandos.
stdlib.command.location = comando «{ $command }» no modelo «{ $template }»
stdlib.command.spawn_failed = Não foi possível iniciar { $location }: { $details }.
stdlib.command.io_failed = { $location } falhou: { $details }.
stdlib.command.closed_input_early = A entrada fechou antes de concluir a escrita para o comando.
stdlib.command.broken_pipe = Canal quebrado ao executar { $location }: { $details }.
stdlib.command.terminated_by_signal = { $location } foi terminado por um sinal.
stdlib.command.exited_with_status = { $location } terminou com o estado { $status }.
stdlib.command.output_limit_exceeded = { $location } excedeu o limite de { $mode } de { $limit } bytes para { $stream }.
stdlib.command.timeout = { $location } excedeu o tempo-limite de { $seconds } segundos.
stdlib.command.exit_status_suffix = (estado de saída { $status })
stdlib.command.signal_suffix = (terminado por um sinal)
stdlib.command.shell.empty = O comando de shell não pode estar vazio.
stdlib.command.grep.empty_pattern = O padrão do grep não pode estar vazio.
stdlib.command.grep.flags_not_string = As opções do grep têm de ser cadeias de carateres.
stdlib.command.quote.invalid = Não foi possível colocar { $arg } entre aspas: { $details }.
stdlib.command.quote.line_break = Os argumentos com retornos de carro ou mudanças de linha não podem ser protegidos com segurança.
stdlib.command.input_undefined = O valor de entrada não está definido.
stdlib.command.tempfile.root_required = É necessária a raiz da área de trabalho para criar ficheiros temporários de comandos.
stdlib.command.tempfile.create_failed = Não foi possível criar o ficheiro temporário do comando: { $details }.
stdlib.command.options.invalid_utf8 = A chave de uma opção do comando tem de ser UTF-8 válido.
stdlib.command.option.mode_not_string = O modo de saída tem de ser uma cadeia de carateres.
stdlib.command.options.invalid_type = As opções do comando têm de ser um objeto.
stdlib.command.output.mode_unsupported = Modo de saída não suportado «{ $mode }».
stdlib.command.output.mode.capture = captura
stdlib.command.output.mode.streaming = fluxo contínuo
stdlib.command.output.stream.stdout = stdout
stdlib.command.output.stream.stderr = stderr

# Diagnósticos do auxiliar de caminhos.
stdlib.path.io.failed = { $action } falhou para { $path } ({ $label }).
stdlib.path.io.failed_with_detail = { $action } falhou para { $path }: { $detail }.
stdlib.path.io.failed_with_label_and_detail = { $action } falhou para { $path } ({ $label }): { $detail }.
stdlib.path.io.not_found = não encontrado
stdlib.path.io.permission_denied = permissão negada
stdlib.path.io.already_exists = já existe
stdlib.path.io.invalid_input = entrada inválida
stdlib.path.io.invalid_data = dados inválidos
stdlib.path.io.timed_out = tempo esgotado
stdlib.path.io.interrupted = interrompido
stdlib.path.io.would_block = bloquearia
stdlib.path.io.write_zero = não foi escrito nenhum byte
stdlib.path.io.unexpected_eof = fim de ficheiro inesperado
stdlib.path.io.broken_pipe = canal quebrado
stdlib.path.io.connection_refused = ligação recusada
stdlib.path.io.connection_reset = ligação reposta
stdlib.path.io.connection_aborted = ligação abortada
stdlib.path.io.not_connected = sem ligação
stdlib.path.io.addr_in_use = endereço em utilização
stdlib.path.io.addr_not_available = endereço indisponível
stdlib.path.io.out_of_memory = sem memória
stdlib.path.io.unsupported = não suportado
stdlib.path.io.file_too_large = ficheiro demasiado grande
stdlib.path.io.resource_busy = recurso ocupado
stdlib.path.io.executable_busy = executável ocupado
stdlib.path.io.deadlock = impasse
stdlib.path.io.crosses_devices = atravessa dispositivos
stdlib.path.io.too_many_links = demasiadas ligações
stdlib.path.io.invalid_filename = nome de ficheiro inválido
stdlib.path.io.arg_list_too_long = lista de argumentos demasiado longa
stdlib.path.io.stale_handle = identificador de ficheiro de rede obsoleto
stdlib.path.io.storage_full = armazenamento cheio
stdlib.path.io.not_seekable = não permite posicionamento
stdlib.path.io.network_down = rede em baixo
stdlib.path.io.network_unreachable = rede inacessível
stdlib.path.io.host_unreachable = anfitrião inacessível
stdlib.path.io.other = erro de E/S
stdlib.path.action.canonicalize = canonizar
stdlib.path.action.open_directory = abrir a pasta
stdlib.path.action.stat = consultar
stdlib.path.action.read = ler
stdlib.path.action.open_file = abrir o ficheiro
stdlib.path.with_suffix.empty_separator = with_suffix exige um separador não vazio.
stdlib.path.relative_to.mismatch = { $path } não é relativo a { $root }.
stdlib.path.expanduser.unsupported = A expansão de ~ para um utilizador específico não é suportada.
stdlib.path.expanduser.no_home = Não é possível expandir ~: não há variáveis de ambiente da pasta pessoal definidas.
stdlib.path.contents.unsupported_encoding = Codificação não suportada «{ $encoding }».
stdlib.path.hash.unsupported_algorithm = Algoritmo de hash não suportado «{ $algorithm }».
stdlib.path.hash.unsupported_algorithm_legacy = Algoritmo de hash não suportado «{ $algorithm }» (ative a funcionalidade «{ $feature }»).

# Diagnósticos dos auxiliares de coleções.
stdlib.collections.flatten.expected_sequence = O flatten esperava itens de uma sequência, mas encontrou { $kind }.
stdlib.collections.group_by.empty_attribute = O group_by exige um atributo não vazio.
stdlib.collections.group_by.unresolved = O group_by não conseguiu resolver «{ $attr }» num item do tipo { $kind }.

# Diagnósticos dos auxiliares de tempo.
stdlib.time.offset.invalid = O desvio de now «{ $offset }» é inválido: esperava-se «+HH:MM[:SS]» ou «Z».
stdlib.time.timedelta.overflow = Sobrecarga de timedelta ao adicionar { $component }.
stdlib.time.label.weeks = semanas
stdlib.time.label.days = dias
stdlib.time.label.hours = horas
stdlib.time.label.minutes = minutos
stdlib.time.label.seconds = segundos
stdlib.time.label.milliseconds = milissegundos
stdlib.time.label.microseconds = microssegundos
stdlib.time.label.nanoseconds = nanossegundos

# Diagnósticos do auxiliar which.
stdlib.which.not_found = [netsuke::jinja::which::not_found] comando «{ $command }» não encontrado após verificar { $count } entradas do PATH. Pré-visualização: { $preview }
stdlib.which.not_found.hint.cwd_auto = Os segmentos vazios do PATH são ignorados; use cwd_mode="auto" para incluir a pasta de trabalho.
stdlib.which.not_found.hint.cwd_always = Defina cwd_mode="always" para incluir a pasta atual.
stdlib.which.direct_not_found = [netsuke::jinja::which::not_found] o comando «{ $command }» em «{ $path }» não existe ou não é executável.
stdlib.which.args_error = [netsuke::jinja::which::args] { $details }
stdlib.which.path_preview.empty = <vazio>
stdlib.which.path_entry.non_utf8 = A entrada n.º { $index } do PATH contém carateres que não são UTF-8; o Netsuke exige caminhos UTF-8.
stdlib.which.command.empty = O which exige uma cadeia de carateres não vazia.
stdlib.which.cwd_mode.invalid = cwd_mode tem de ser «auto», «always» ou «never», obteve-se «{ $mode }».
stdlib.which.cwd.resolve_failed = Não foi possível resolver a pasta atual: { $details }.
stdlib.which.cwd.non_utf8 = A pasta atual contém componentes que não são UTF-8.
stdlib.which.canonicalize_failed = Não foi possível canonizar «{ $path }»: { $details }.
stdlib.which.is_executable = Não foi possível verificar se «{ $path }» é executável: { $details }.
stdlib.which.canonicalize_non_utf8 = O caminho canónico contém componentes que não são UTF-8.
stdlib.which.workspace_non_utf8 = O caminho da área de trabalho contém componentes que não são UTF-8 ao resolver o comando «{ $command }»: { $path }.
stdlib.which.walkdir_error = Erro ao percorrer a área de trabalho durante a resolução do comando: { $details }.

# Registo da biblioteca padrão.
stdlib.register.open_dir = Não foi possível abrir a pasta atual para o registo da stdlib.
stdlib.register.resolve_dir = Não foi possível resolver a pasta atual para o registo da stdlib.
stdlib.register.dir_non_utf8 = A pasta atual contém componentes que não são UTF-8: { $path }.

# Relatório de estado para o modo de saída acessível.
status.state.pending = pendente
status.state.running = em curso
status.state.done = concluída
status.state.failed = falhou
status.stage.label = Fase { $current }/{ $total }: { $description }
status.stage.summary = [{ $state }] { $label }
status.stage.summary_with_task = [{ $state }] { $label } ({ $task_progress })
status.task.progress_label = Tarefa { $current }/{ $total }
status.task.progress_update = { $task }: { $description }
status.stage.manifest_ingestion = A ler o ficheiro de manifesto
status.stage.initial_yaml_parsing = A analisar o documento YAML
status.stage.template_expansion = A expandir as diretivas dos modelos
status.stage.final_rendering = A desserializar e representar os valores do manifesto
status.stage.ir_generation_validation = A construir e validar o grafo de dependências
status.stage.ninja_synthesis = A sintetizar o plano de compilação Ninja
status.stage.ninja_synthesis_execute = A sintetizar o plano Ninja e a executar { $tool }
status.stage.graph_rendering = A representar o artefacto do grafo
status.stage.graph_rendering_with_tool = A representar { $tool }
status.complete = { $tool }: operação concluída.
status.timing.summary_header = Resumo de tempos por fase:
status.timing.stage_line = - { $label }: { $duration }
status.timing.total_line = Tempo total do pipeline: { $duration }
status.tool.build = Compilação
status.tool.clean = Limpeza
status.tool.graph = Grafo
status.tool.graph_html = Grafo (HTML)
status.tool.generate = Geração

# Cadeias do representador HTML do grafo.
graph.html.title = Grafo de compilação do Netsuke
graph.html.heading = Grafo de compilação do Netsuke
graph.html.description = Grafo de compilação representado pelo Netsuke
graph.html.outline.summary = Alvos e dependências (esquema textual)
graph.html.outline.no_inputs = Sem entradas
graph.html.noscript.notice = O JavaScript está desativado. O esquema textual acima contém o grafo completo; segue-se o código DOT.

# Prefixos semânticos para a saída acessível.
semantic.prefix.error = Erro:
semantic.prefix.warning = Aviso:
semantic.prefix.success = Sucesso:
semantic.prefix.info = Info:
semantic.prefix.timing = Tempos:
semantic.prefix.rendered = {"{"}symbol{"}"} {"{"}label{"}"}

# Exemplos de formas plurais para tradutores.
# O português usa as categorias CLDR `one` e `other`, tal como o idioma
# de origem.
example.files_processed = { $count ->
    [one] Foi processado { $count } ficheiro.
   *[other] Foram processados { $count } ficheiros.
}

example.errors_found = { $count ->
    [0] Não foram encontrados erros.
    [one] Foi encontrado { $count } erro.
   *[other] Foram encontrados { $count } erros.
}
