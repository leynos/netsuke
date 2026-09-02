<!-- markdownlint-disable MD013 MD033 MD041 -->

<div align="center">

[English](README.md) | [Deutsch](README.de.md) | [Español](README.es.md) |
[Français](README.fr.md) | [日本語](README.ja.md) |
[Português do Brasil](README.pt-BR.md) | [简体中文](README.zh-CN.md)

</div>

<!-- markdownlint-enable MD013 MD033 MD041 -->

# 🧵 Netsuke

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](
https://deepwiki.com/leynos/netsuke)

*Um compilador de sistema de build amigável: YAML e Jinja na entrada, Ninja na
saída.*

O Netsuke transforma um `Netsukefile` legível em um grafo de build Ninja
estático e validado. Ele mantém o trabalho dinâmico em um manifesto de nível
mais alto e deixa a execução rápida e incremental a cargo do
[Ninja](https://ninja-build.org/).

Site: <https://df12.studio/netsuke>

______________________________________________________________________

## Por que o Netsuke?

- **Manifestos legíveis**: descreva regras, alvos, dependências e padrões em
  YAML em vez de uma linguagem sensível a tabulações.
- **Planejamento dinâmico**: use variáveis, macros, `foreach`, `when` e
  globbing do Jinja antes que o Netsuke crie o grafo de build.
- **Execução estática**: inspecione o arquivo Ninja gerado ou renderize o
  grafo antes de executar qualquer comando de build.
- **Diagnósticos úteis**: obtenha erros com reconhecimento de origem, saída
  localizada, relatório de progresso e a saída canônica de comandos, legível
  por máquina, em `--json`.
- **Nenhuma toolchain privilegiada**: use o mesmo modelo de manifesto para
  Rust, C, Python, projetos web ou qualquer outra coisa que um comando consiga
  construir.

______________________________________________________________________

## Primeiros passos

### Pré-requisitos

Atualmente, o Netsuke requer:

- [Ninja](https://ninja-build.org/) no `PATH`;
- ao instalar a partir do código-fonte, a toolchain nightly do Rust com data
  fixada em [`rust-toolchain.toml`](rust-toolchain.toml) (o `rustup` a instala
  automaticamente em um checkout). O Netsuke é compilado com o verificador de
  empréstimos (borrow checker) Polonius, que a nightly habilita por padrão e
  que permanece exclusivo da nightly até ser estabilizado; veja a
  [ADR-006](docs/adr-006-adopt-polonius-nightly-toolchain.md).

### Instalação

A versão prévia mais recente publicada é o Netsuke v0.1.0-beta3 (precedida pela
v0.1.0-beta2), disponível no crates.io. Onde o
[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) estiver
disponível, prefira-o: ele busca um binário de release pré-compilado e evita o
requisito de toolchain descrito abaixo.

```sh
cargo binstall netsuke-build
```

Compilar a partir do registro, em vez disso, ocorre fora de um checkout do
repositório, então a toolchain fixada não é detectada automaticamente;
selecione-a explicitamente:

```sh
rustup toolchain install nightly-2026-08-23
cargo +nightly-2026-08-23 install netsuke-build
```

Instaladores pré-compilados estão disponíveis na
[release v0.1.0-beta3 do GitHub](https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta3):

| Plataforma | Arquiteturas                       | Pacotes                        |
| ---------- | ---------------------------------- | ------------------------------ |
| Linux      | x86-64 (`amd64`) e Arm64 (`arm64`) | Debian (`.deb`) e RPM (`.rpm`) |
| macOS      | Intel x86-64 e Apple silicon Arm64 | Pacote instalador (`.pkg`)     |
| Windows    | x64 e Arm64                        | Instalador do Windows (`.msi`) |

Os pacotes Linux instalam a página de manual do `netsuke` e declaram
`ninja-build` como dependência. O Ninja deve ser instalado separadamente ao
usar o instalador do macOS ou do Windows. O MSI do Windows instala em
`C:\Program Files\netsuke` e não atualiza o `PATH`. Arquivos de checksum
SHA-256 acompanham os binários independentes e os arquivos de ajuda e licença
preparados. Os pacotes instaladores não têm arquivos de checksum associados na
v0.1.0-beta3. Veja o [guia do usuário](docs/users-guide.md#install-netsuke)
para comandos específicos de cada plataforma e para a configuração no Windows.

Para instalar o checkout atual do código-fonte com o Cargo:

```sh
git clone https://github.com/leynos/netsuke.git
cd netsuke
cargo install --path .
```

### Seu primeiro build

Crie um novo diretório e adicione um arquivo chamado `Netsukefile`:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

Execute o Netsuke e, em seguida, inspecione o resultado:

```sh
netsuke
cat hello.txt
```

O segundo comando imprime `Hello from Netsuke!`. Veja o
[guia de início rápido](docs/quickstart.md) para variáveis, modelos e
`foreach`, e depois use o
[guia da biblioteca padrão de modelos](docs/stdlib-yaml-and-jinja-guide.md)
para cada auxiliar de caminho, coleção, sistema de arquivos, tempo, comando,
ambiente, glob e rede.

______________________________________________________________________

## O que funciona hoje

O compilador de sistema de build principal do Netsuke v0.1.0-beta3 oferece:

- análise de manifestos YAML 1.2 com validação de chaves duplicadas e de
  esquema;
- variáveis, macros, `foreach`, `when`, globbing, auxiliares de ambiente,
  descoberta de executáveis e auxiliares de rede opcionais do Jinja;
- regras, alvos, ações e padrões reutilizáveis, além de dependências
  explícitas, implícitas e apenas de ordem;
- descoberta de alvos e ações por meio de `netsuke help targets`, incluindo
  entradas condicionais sem renderização de receita;
- um grafo de build intermediário determinístico com verificações de saída
  duplicada, regra ausente e ciclos;
- execução de receitas legadas do Windows por meio do Windows PowerShell por
  padrão, com uma rota de compatibilidade explícita via Git Bash ou MSYS2;
- geração e execução do Ninja, além do `clean` e da geração autônoma de
  manifesto;
- grafos de dependências reproduzíveis em Graphviz DOT ou em HTML acessível
  e autocontido;
- configuração em camadas, saída localizada, preferências de acessibilidade,
  relatório de progresso, tempos de estágio e resultados ou diagnósticos JSON
  versionados;
- cobertura de testes unitários, comportamentais, de integração, de
  propriedade, de snapshot e verificação inicial com Kani.

A release beta3 também oferece suporte a agregados de ações e alvos
apenas-de-dependência: nós com uma lista `deps` não vazia podem omitir uma
receita.

______________________________________________________________________

## Status da release e do desenvolvimento

A release v0.1.0-beta3 é uma prévia útil para adotantes iniciais, não uma
declaração de que o Netsuke está concluído ou de que todas as interfaces são
estáveis. O pipeline do compilador e o fluxo de trabalho comum de build local
já são substanciais; a interface de linha de comando, o vocabulário de
configuração e o modelo avançado de receitas permanecem pré-estáveis.

Fixe a versão do Netsuke em automações e espere que alguns nomes de comando,
flags, esquemas de diagnóstico e detalhes de manifesto mudem antes da 1.0.

As limitações a seguir se aplicam à beta3.

Limitações conhecidas incluem:

- as receitas continuam sendo strings de shell: scripts Unix usam
  `/bin/sh -e`, receitas legadas do Windows usam o Windows PowerShell por
  padrão, e a rota de compatibilidade com o Bash no Windows é opcional e
  explícita; argumentos de executável estruturados e mapeamentos de ambiente de
  receita ainda não estão implementados;
- importações de dependências geradas pelo compilador, como depfiles do
  GCC, estão planejadas, mas ainda não fazem parte do modelo de manifesto;
- `--json` emite exatamente um documento versionado de resultado ou
  diagnóstico para cada comando, mas o esquema ainda pode mudar antes da 1.0;
- a renderização de cores não está implementada;
- a acessibilidade ainda precisa de verificação com tecnologia assistiva.

A release beta3 corrige a limitação do cifrão de shell da beta2 com escape
sensível ao Ninja, de modo que expressões de shell comuns podem ser escritas
normalmente. Manifestos da beta2 que usam expressões literais de cifrão de
shell exigem migração; veja a
[fronteira de segurança do guia do usuário](docs/users-guide.md#review-the-safety-boundary).

Um `Netsukefile` pode executar comandos e usar auxiliares de modelo impuros.
Trate-o com o mesmo cuidado que um `Makefile`: revise manifestos não confiáveis
antes de executá-los. O Netsuke coloca entre aspas as substituições de caminho
suportadas, mas não é um sandbox.

______________________________________________________________________

## O caminho adiante

O trabalho após a primeira release está organizado em torno de três prioridades:

1. **Estabilizar o contrato de linha de comando**: consolidar os nomes
   canônicos de comandos e flags, salvaguardas não interativas, códigos de
   saída estáveis, saída limitada e documentos `--json` versionados.
2. **Tornar as receitas mais seguras e claras**: adicionar argumentos de
   executável estruturados, mapeamentos de ambiente, importações de
   dependências do compilador e melhor retorno sobre ações condicionais.
3. **Fortalecer a confiança**: expandir a cobertura de Kani e de testes de
   propriedade, verificar a acessibilidade com tecnologia assistiva e adicionar
   cobertura de regressão para a renderização no terminal.

O trabalho de longo prazo explora contexto legível por máquina, perfis,
histórico de execuções, entrega de artefatos e retorno local-first para fluxos
de trabalho humanos e de agentes. O [roteiro](docs/roadmap.md) acompanha a
sequência detalhada e o progresso atual.

______________________________________________________________________

## Saiba mais

- [Guia de início rápido](docs/quickstart.md) — construa algo em cinco
  minutos.
- [Guia do usuário](docs/users-guide.md) — referência de manifesto e
  comandos.
- [Documento de design](docs/netsuke-design.md) — arquitetura e
  justificativa de design.
- [Guia do desenvolvedor](docs/developers-guide.md) — fluxo de trabalho de
  desenvolvimento e portões de qualidade.
- [Roteiro](docs/roadmap.md) — fundações concluídas e trabalho planejado.

______________________________________________________________________

## Licença

ISC — veja [LICENSE](LICENSE) para detalhes.

______________________________________________________________________

## Contribuindo

Contribuições são bem-vindas. Comece pelo
[guia do desenvolvedor](docs/developers-guide.md); contribuidores automatizados
também devem seguir o [AGENTS.md](AGENTS.md).
