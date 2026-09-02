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

*Ein freundlicher Build-System-Compiler: YAML und Jinja hinein, Ninja hinaus.*

Netsuke verwandelt ein lesbares `Netsukefile` in einen validierten, statischen
Ninja-Build-Graph. Es hält die dynamische Arbeit in einem höherstufigen
Manifest und überlässt [Ninja](https://ninja-build.org/) die schnelle,
inkrementelle Ausführung.

Website: <https://df12.studio/netsuke>

______________________________________________________________________

## Warum Netsuke?

- **Lesbare Manifeste**: Beschreiben Sie Regeln, Ziele, Abhängigkeiten und
  Vorgaben in YAML statt in einer einrückungsempfindlichen Sprache.
- **Dynamische Planung**: Verwenden Sie Jinja-Variablen, Makros, `foreach`,
  `when` und Glob-Muster, bevor Netsuke den Build-Graph erstellt.
- **Statische Ausführung**: Prüfen Sie die erzeugte Ninja-Datei oder rendern
  Sie den Graphen, bevor Sie einen Build-Befehl ausführen.
- **Nützliche Diagnosen**: Erhalten Sie quellenbezogene Fehlermeldungen,
  lokalisierte Ausgaben, Fortschrittsanzeigen und maschinenlesbare, kanonische
  `--json`-Befehlsausgaben.
- **Keine bevorzugte Toolchain**: Verwenden Sie dasselbe Manifestmodell für
  Rust, C, Python, Webprojekte oder alles andere, das ein Befehl erzeugen kann.

______________________________________________________________________

## Schnellstart

### Voraussetzungen

Netsuke erfordert derzeit:

- [Ninja](https://ninja-build.org/) im `PATH`;
- bei der Installation aus dem Quellcode die datierte Rust-Nightly-Toolchain,
  die in [`rust-toolchain.toml`](rust-toolchain.toml) festgelegt ist (`rustup`
  installiert sie in einem Checkout automatisch). Netsuke wird mit dem
  Polonius-Borrow-Checker gebaut, den Nightly standardmäßig aktiviert und der
  bis zur Stabilisierung nightly-exklusiv bleibt; siehe
  [ADR-006](docs/adr-006-adopt-polonius-nightly-toolchain.md).

### Installation

Die zuletzt veröffentlichte Vorabversion ist Netsuke v0.1.0-beta3, verfügbar
über crates.io. Wo
[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) verfügbar ist,
sollte es bevorzugt werden: Es lädt eine vorgefertigte Release-Binärdatei
herunter und vermeidet die unten genannte Toolchain-Anforderung.

```sh
cargo binstall netsuke-build
```

Die Installation über die Registry erfolgt außerhalb eines
Repository-Checkouts, sodass die festgelegte Toolchain nicht automatisch
übernommen wird; wählen Sie sie explizit aus:

```sh
rustup toolchain install nightly-2026-08-23
cargo +nightly-2026-08-23 install netsuke-build
```

Vorgefertigte Installationsprogramme sind über das
[v0.1.0-beta3-GitHub-Release](https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta3)
verfügbar:

| Plattform | Architekturen                        | Pakete                           |
| --------- | ------------------------------------ | -------------------------------- |
| Linux     | x86-64 (`amd64`) und Arm64 (`arm64`) | Debian (`.deb`) und RPM (`.rpm`) |
| macOS     | Intel x86-64 und Apple Silicon Arm64 | Installationspaket (`.pkg`)      |
| Windows   | x64 und Arm64                        | Windows Installer (`.msi`)       |

Die Linux-Pakete installieren die `netsuke`-Handbuchseite und deklarieren
`ninja-build` als Abhängigkeit. Ninja muss bei Verwendung des macOS- oder
Windows-Installationsprogramms separat installiert werden. Die Windows-MSI
installiert nach `C:\Program Files\netsuke` und aktualisiert `PATH` nicht.
SHA-256-Prüfsummendateien begleiten eigenständige Binärdateien sowie
bereitgestellte Hilfe- und Lizenzdateien. Installationspakete verfügen in
v0.1.0-beta3 nicht über begleitende Prüfsummen. Siehe das
[Benutzerhandbuch](docs/users-guide.md#install-netsuke) für
plattformspezifische Befehle und die Windows-Einrichtung.

So installieren Sie den aktuellen Quell-Checkout mit Cargo:

```sh
git clone https://github.com/leynos/netsuke.git
cd netsuke
cargo install --path .
```

### Ihr erster Build

Erstellen Sie ein neues Verzeichnis und fügen Sie eine Datei namens
`Netsukefile` hinzu:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

Führen Sie Netsuke aus und prüfen Sie anschließend das Ergebnis:

```sh
netsuke
cat hello.txt
```

Der zweite Befehl gibt `Hello from Netsuke!` aus. Siehe das
[Schnellstart-Handbuch](docs/quickstart.md) für Variablen, Vorlagen und
`foreach`, und verwenden Sie danach den
[Leitfaden zur Vorlagen-Standardbibliothek](docs/stdlib-yaml-and-jinja-guide.md)
für jeden Pfad-, Sammlungs-, Dateisystem-, Zeit-, Befehls-, Umgebungs-, Glob-
und Netzwerk-Helfer.

______________________________________________________________________

## Was heute funktioniert

Der zentrale Build-System-Compiler von Netsuke v0.1.0-beta3 bietet:

- YAML-1.2-Manifest-Parsing mit Prüfung auf doppelte Schlüssel und
  Schemavalidierung;
- Jinja-Variablen, Makros, `foreach`, `when`, Glob-Muster,
  Umgebungs-Helfer, Erkennung ausführbarer Dateien und optionale
  Netzwerk-Helfer;
- wiederverwendbare Regeln, Ziele, Aktionen, Vorgaben sowie explizite,
  implizite und Ordnungsabhängigkeiten;
- Ziel- und Aktionserkennung über `netsuke help targets`, einschließlich
  bedingter Einträge ohne Rezept-Rendering;
- einen deterministischen Zwischen-Build-Graph mit Prüfungen auf doppelte
  Ausgaben, fehlende Regeln und Zyklen;
- Ausführung veralteter Windows-Rezepte standardmäßig über Windows
  PowerShell, mit einem expliziten Git-Bash- oder MSYS2-Kompatibilitätspfad;
- Ninja-Erzeugung und -Ausführung sowie `clean` und eigenständige
  Manifesterzeugung;
- reproduzierbare Abhängigkeitsgraphen als Graphviz-DOT oder
  eigenständiges, barrierefreies HTML;
- geschichtete Konfiguration, lokalisierte Ausgaben,
  Barrierefreiheitseinstellungen, Fortschrittsanzeigen, Phasenzeiten sowie
  versionierte JSON-Ergebnisse oder -Diagnosen;
- Unit-, Verhaltens-, Integrations-, Property-, Snapshot- und erste
  Kani-Verifikationsabdeckung.

Das beta3-Release unterstützt außerdem reine Abhängigkeits-Aggregate für
Aktionen und Ziele: Knoten mit einer nicht leeren `deps`-Liste können ein
Rezept auslassen.

______________________________________________________________________

## Release- und Entwicklungsstatus

Das Release v0.1.0-beta3 ist eine nützliche Vorschau für Früheinsteiger, keine
Erklärung, dass Netsuke fertig ist oder jede Schnittstelle stabil ist. Die
Compiler-Pipeline und der gewöhnliche lokale Build-Workflow sind umfangreich;
die Kommandozeilenschnittstelle, das Konfigurationsvokabular und das erweiterte
Rezeptmodell bleiben vorläufig.

Legen Sie die Netsuke-Version in der Automatisierung fest und rechnen Sie
damit, dass sich einige Befehlsnamen, Flags, Diagnoseschemata und
Manifestdetails vor 1.0 ändern.

Für beta3 gelten die folgenden Einschränkungen.

Zu den bekannten Einschränkungen zählen:

- Rezepte bleiben Shell-Zeichenketten: Unix-Skripte verwenden
  `/bin/sh -e`, veraltete Windows-Rezepte verwenden standardmäßig Windows
  PowerShell, und der Windows-Bash-Kompatibilitätspfad ist ein ausdrückliches
  Opt-in; strukturierte ausführbare Argumente und Umgebungszuordnungen für
  Rezepte sind noch nicht implementiert;
- vom Compiler erzeugte Abhängigkeitsimporte wie GCC-Depfiles sind geplant,
  aber noch nicht Teil des Manifestmodells;
- `--json` gibt für jeden Befehl genau ein versioniertes Ergebnis- oder
  Diagnosedokument aus, aber das Schema kann sich vor 1.0 noch ändern;
- Farbdarstellung ist nicht implementiert;
- die Barrierefreiheit erfordert noch eine Überprüfung mit assistiven
  Technologien.

Das beta3-Release behebt die Shell-Dollar-Einschränkung von beta2 durch
Ninja-bewusstes Escaping, sodass gewöhnliche Shell-Ausdrücke normal geschrieben
werden können. Beta2-Manifeste, die wörtliche Shell-Dollar-Ausdrücke verwenden,
müssen migriert werden; siehe die
[Sicherheitsgrenze im Benutzerhandbuch](docs/users-guide.md#review-the-safety-boundary).

Ein `Netsukefile` kann Befehle ausführen und unreine Vorlagen-Helfer verwenden.
Es sollte mit derselben Sorgfalt behandelt werden wie ein `Makefile`: Prüfen
Sie nicht vertrauenswürdige Manifeste, bevor Sie sie ausführen. Netsuke
maskiert unterstützte Pfadersetzungen, ist jedoch keine Sandbox.

______________________________________________________________________

## Der weitere Weg

Die Arbeit nach dem ersten Release ist um drei Prioritäten herum organisiert:

1. **Den Kommandozeilenvertrag stabilisieren**: die kanonischen Befehls-
   und Flag-Namen, nicht interaktive Sicherungen, stabile Exit-Codes, begrenzte
   Ausgaben und versionierte `--json`-Dokumente festigen.
2. **Rezepte sicherer und klarer machen**: strukturierte ausführbare
   Argumente, Umgebungszuordnungen, Compiler-Abhängigkeitsimporte und bessere
   Rückmeldungen zu bedingten Aktionen hinzufügen.
3. **Vertrauen stärken**: die Kani- und Property-Test-Abdeckung
   erweitern, die Barrierefreiheit mit assistiven Technologien überprüfen und
   Regressionsabdeckung für das Terminal-Rendering hinzufügen.

Langfristige Arbeiten untersuchen maschinenlesbaren Kontext, Profile,
Verlaufsdaten, Artefaktauslieferung und lokal-first-Rückmeldungen für
menschliche und agentenbasierte Workflows. Die [Roadmap](docs/roadmap.md)
verfolgt die detaillierte Reihenfolge und den aktuellen Fortschritt.

______________________________________________________________________

## Mehr erfahren

- [Schnellstart-Handbuch](docs/quickstart.md) — bauen Sie in fünf Minuten
  etwas.
- [Benutzerhandbuch](docs/users-guide.md) — Manifest- und
  Befehlsreferenz.
- [Design-Dokument](docs/netsuke-design.md) — Architektur und
  Design-Begründung.
- [Entwicklerhandbuch](docs/developers-guide.md) — Entwicklungsworkflow
  und Qualitätsprüfungen.
- [Roadmap](docs/roadmap.md) — abgeschlossene Grundlagen und geplante
  Arbeiten.

______________________________________________________________________

## Lizenz

ISC — siehe [LICENSE](LICENSE) für Einzelheiten.

______________________________________________________________________

## Mitwirken

Beiträge sind willkommen. Beginnen Sie mit dem
[Entwicklerhandbuch](docs/developers-guide.md); automatisierte Mitwirkende
sollten außerdem [AGENTS.md](AGENTS.md) befolgen.
