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

*Un compilador de sistemas de construcción amigable: YAML y Jinja como entrada,
Ninja como salida.*

Netsuke convierte un `Netsukefile` legible en un grafo de compilación Ninja
validado y estático. Mantiene el trabajo dinámico en un manifiesto de nivel
superior y deja la ejecución rápida e incremental a
[Ninja](https://ninja-build.org/).

Sitio web: <https://df12.studio/netsuke>

______________________________________________________________________

## ¿Por qué Netsuke?

- **Manifiestos legibles**: describa reglas, objetivos, dependencias y
  valores predeterminados en YAML en lugar de un lenguaje sensible a las
  tabulaciones.
- **Planificación dinámica**: use variables, macros, `foreach`, `when` y
  patrones glob de Jinja antes de que Netsuke cree el grafo de compilación.
- **Ejecución estática**: inspeccione el archivo Ninja generado o represente
  el grafo antes de ejecutar cualquier comando de compilación.
- **Diagnósticos útiles**: obtenga errores con reconocimiento del origen,
  salida localizada, informes de progreso y una salida canónica `--json` de
  comandos legible por máquina.
- **Sin una cadena de herramientas privilegiada**: use el mismo modelo de
  manifiesto para Rust, C, Python, proyectos web o cualquier otra cosa que un
  comando pueda generar.

______________________________________________________________________

## Primeros pasos

### Requisitos previos

Netsuke actualmente requiere lo siguiente:

- [Ninja](https://ninja-build.org/) en `PATH`;
- al instalar desde el código fuente, la cadena de herramientas nightly de
  Rust con fecha fija en [`rust-toolchain.toml`](rust-toolchain.toml) (`rustup`
  la instala automáticamente en un checkout). Netsuke se compila con el
  verificador de préstamos Polonius, que nightly habilita de forma
  predeterminada y que permanece exclusivo de nightly hasta que se estabilice;
  véase [ADR-006](docs/adr-006-adopt-polonius-nightly-toolchain.md).

### Instalación

La última versión prelanzamiento publicada es Netsuke v0.1.0-beta3 (precedida
por v0.1.0-beta2), disponible en crates.io. Cuando
[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) esté
disponible, prefiéralo: obtiene un binario ya compilado del lanzamiento y evita
el requisito de cadena de herramientas indicado a continuación.

```sh
cargo binstall netsuke-build
```

En cambio, instalar desde el registro se ejecuta fuera de un checkout del
repositorio, por lo que la cadena de herramientas fijada no se detecta
automáticamente; selecciónela explícitamente:

```sh
rustup toolchain install nightly-2026-08-23
cargo +nightly-2026-08-23 install netsuke-build
```

Hay instaladores prediseñados disponibles en el
[lanzamiento de GitHub v0.1.0-beta3](https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta3):

| Plataforma | Arquitecturas                      | Paquetes                       |
| ---------- | ---------------------------------- | ------------------------------ |
| Linux      | x86-64 (`amd64`) y Arm64 (`arm64`) | Debian (`.deb`) y RPM (`.rpm`) |
| macOS      | Intel x86-64 y Apple silicon Arm64 | Paquete instalador (`.pkg`)    |
| Windows    | x64 y Arm64                        | Instalador de Windows (`.msi`) |

Los paquetes de Linux instalan la página de manual de `netsuke` y declaran
`ninja-build` como dependencia. Ninja debe instalarse por separado al usar el
instalador de macOS o de Windows. El instalador MSI de Windows se instala en
`C:\Program Files\netsuke` y no actualiza `PATH`. Los archivos de suma de
comprobación SHA-256 acompañan a los binarios independientes y a los archivos
de ayuda y licencia preparados. Los paquetes instaladores no tienen archivos de
suma de comprobación adjuntos en v0.1.0-beta3. Consulte la
[guía del usuario](docs/users-guide.md#install-netsuke) para conocer los
comandos específicos de cada plataforma y la configuración en Windows.

Para instalar el checkout de código fuente actual con Cargo:

```sh
git clone https://github.com/leynos/netsuke.git
cd netsuke
cargo install --path .
```

### Su primera compilación

Cree un nuevo directorio y agregue un archivo llamado `Netsukefile`:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

Ejecute Netsuke y luego inspeccione el resultado:

```sh
netsuke
cat hello.txt
```

El segundo comando imprime `Hello from Netsuke!`. Consulte la
[guía de inicio rápido](docs/quickstart.md) para conocer las variables, las
plantillas y `foreach`; luego use la
[guía de la biblioteca estándar de plantillas](docs/stdlib-yaml-and-jinja-guide.md)
para todos los ayudantes de rutas, colecciones, sistema de archivos, tiempo,
comandos, entorno, glob y red.

______________________________________________________________________

## Qué funciona hoy

El compilador de sistemas de construcción principal de Netsuke v0.1.0-beta3
ofrece lo siguiente:

- análisis de manifiestos YAML 1.2 con validación de claves duplicadas y de
  esquema;
- variables, macros, `foreach`, `when`, patrones glob, ayudantes de entorno,
  detección de ejecutables y ayudantes de red opcionales de Jinja;
- reglas, objetivos, acciones y valores predeterminados reutilizables,
  además de dependencias explícitas, implícitas y de solo orden;
- detección de objetivos y acciones mediante `netsuke help targets`,
  incluidas las entradas condicionales sin generación de la receta;
- un grafo de compilación intermedio determinista con comprobaciones de
  salidas duplicadas, reglas faltantes y ciclos;
- ejecución de recetas heredadas de Windows mediante Windows PowerShell de
  forma predeterminada, con una ruta de compatibilidad explícita con Git Bash o
  MSYS2;
- generación y ejecución de Ninja, además de `clean` y generación de
  manifiestos independientes;
- grafos de dependencias reproducibles como DOT de Graphviz o HTML
  accesible y autónomo;
- configuración por capas, salida localizada, preferencias de
  accesibilidad, informes de progreso, tiempos de las etapas, y resultados o
  diagnósticos JSON versionados;
- cobertura de pruebas unitarias, de comportamiento, de integración, de
  propiedades, de instantáneas y de verificación inicial con Kani.

El lanzamiento beta3 también admite agregados de acciones y objetivos basados
únicamente en dependencias: los nodos con una lista `deps` no vacía pueden
omitir una receta.

______________________________________________________________________

## Estado del lanzamiento y del desarrollo

El lanzamiento v0.1.0-beta3 es una vista previa útil para quienes lo adoptan
tempranamente, no una declaración de que Netsuke esté terminado o de que todas
sus interfaces sean estables. El pipeline del compilador y el flujo de trabajo
habitual de compilación local son sustanciales; la interfaz de línea de
comandos, el vocabulario de configuración y el modelo avanzado de recetas
siguen siendo previos a la estabilidad.

Fije la versión de Netsuke en la automatización y espere que algunos nombres de
comandos, indicadores, esquemas de diagnóstico y detalles del manifiesto
cambien antes de la versión 1.0.

Las siguientes limitaciones se aplican a beta3.

Las limitaciones conocidas incluyen:

- las recetas siguen siendo cadenas de shell: los scripts de Unix usan
  `/bin/sh -e`, las recetas heredadas de Windows usan Windows PowerShell de
  forma predeterminada, y la ruta de compatibilidad con Bash de Windows es una
  opción explícita; los argumentos de ejecutables estructurados y las
  asignaciones de entorno de las recetas aún no están implementados;
- las importaciones de dependencias generadas por el compilador, como los
  depfiles de GCC, están planificadas pero aún no forman parte del modelo de
  manifiesto;
- `--json` emite exactamente un documento de resultado o diagnóstico
  versionado por cada comando, pero el esquema aún puede cambiar antes de la
  versión 1.0;
- la representación en color no está implementada;
- la accesibilidad todavía necesita verificación con tecnología de
  asistencia.

El lanzamiento beta3 corrige la limitación del signo de dólar de shell de beta2
mediante un escape consciente de Ninja, de modo que las expresiones de shell
habituales puedan escribirse con normalidad. Los manifiestos de beta2 que usan
expresiones literales con el signo de dólar de shell requieren migración; véase
el
[límite de seguridad de la guía del usuario](docs/users-guide.md#review-the-safety-boundary).

Un `Netsukefile` puede ejecutar comandos y usar ayudantes de plantilla impuros.
Trátelo con el mismo cuidado que un `Makefile`: revise los manifiestos que no
sean de confianza antes de ejecutarlos. Netsuke entrecomilla las sustituciones
de ruta admitidas, pero no es un entorno aislado.

______________________________________________________________________

## El camino por delante

El trabajo posterior al primer lanzamiento se organiza en torno a tres
prioridades:

1. **Estabilizar el contrato de la línea de comandos**: reforzar los
   nombres canónicos de comandos e indicadores, las salvaguardas no
   interactivas, los códigos de salida estables, la salida acotada y los
   documentos `--json` versionados.
2. **Hacer que las recetas sean más seguras y claras**: agregar argumentos
   de ejecutables estructurados, asignaciones de entorno, importaciones de
   dependencias del compilador y mejores comentarios sobre las acciones
   condicionales.
3. **Reforzar la confianza**: ampliar la cobertura de Kani y de pruebas de
   propiedades, verificar la accesibilidad con tecnología de asistencia y
   agregar cobertura de regresión para la representación en la terminal.

El trabajo a más largo plazo explora contexto legible por máquina, perfiles,
historial de ejecuciones, entrega de artefactos y retroalimentación local-first
para flujos de trabajo humanos y de agentes. La [hoja de ruta](docs/roadmap.md)
hace seguimiento de la secuencia detallada y del progreso actual.

______________________________________________________________________

## Más información

- [Guía de inicio rápido](docs/quickstart.md) — genere algo en cinco
  minutos.
- [Guía del usuario](docs/users-guide.md) — referencia de manifiestos y
  comandos.
- [Documento de diseño](docs/netsuke-design.md) — arquitectura y
  justificación del diseño.
- [Guía del desarrollador](docs/developers-guide.md) — flujo de trabajo de
  desarrollo y controles de calidad.
- [Hoja de ruta](docs/roadmap.md) — bases completadas y trabajo planificado.

______________________________________________________________________

## Licencia

ISC; consulte [LICENSE](LICENSE) para más detalles.

______________________________________________________________________

## Contribuciones

Las contribuciones son bienvenidas. Comience con la
[guía del desarrollador](docs/developers-guide.md); los colaboradores
automatizados también deben seguir [AGENTS.md](AGENTS.md). </content> </invoke>
