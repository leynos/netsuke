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

*Un compilateur de système de build convivial : YAML et Jinja en entrée, Ninja
en sortie.*

Netsuke transforme un `Netsukefile` lisible en un graphe de compilation Ninja
statique et validé. Il conserve le travail dynamique dans un manifeste de plus
haut niveau et laisse l'exécution rapide et incrémentale à
[Ninja](https://ninja-build.org/).

Site web : <https://df12.studio/netsuke>

______________________________________________________________________

## Pourquoi Netsuke ?

- **Manifestes lisibles** : décrivez les règles, les cibles, les
  dépendances et les valeurs par défaut en YAML plutôt que dans un langage
  sensible aux tabulations.
- **Planification dynamique** : utilisez les variables Jinja, les macros,
  `foreach`, `when` et le glob avant que Netsuke ne crée le graphe de
  compilation.
- **Exécution statique** : inspectez le fichier Ninja généré ou affichez le
  graphe avant d'exécuter la moindre commande de build.
- **Diagnostics utiles** : bénéficiez d'erreurs contextualisées à la source,
  d'une sortie localisée, d'un suivi de progression et d'une sortie de commande
  canonique `--json` lisible par une machine.
- **Aucune chaîne d'outils imposée** : utilisez le même modèle de manifeste
  pour Rust, C, Python, les projets web, ou tout ce qu'une commande peut
  construire.

______________________________________________________________________

## Démarrage rapide

### Prérequis

Netsuke nécessite actuellement :

- [Ninja](https://ninja-build.org/) accessible dans le `PATH`;
- lors d'une installation depuis les sources, la chaîne d'outils Rust
  nightly datée épinglée dans [`rust-toolchain.toml`](rust-toolchain.toml)
  (`rustup` l'installe automatiquement dans un clone du dépôt). Netsuke se
  compile avec le vérificateur d'emprunts Polonius, que nightly active par
  défaut et qui reste réservé à nightly jusqu'à sa stabilisation ; voir
  [ADR-006](docs/adr-006-adopt-polonius-nightly-toolchain.md).

### Installation

La dernière préversion publiée est Netsuke v0.1.0-beta3, disponible sur
crates.io. Lorsque
[`cargo binstall`](https://github.com/cargo-bins/cargo-binstall) est
disponible, préférez-le : il récupère un binaire de version prêt à l'emploi et
évite l'exigence de chaîne d'outils ci-dessous.

```sh
cargo binstall netsuke-build
```

Construire depuis le registre s'exécute en dehors d'un clone du dépôt, la
chaîne d'outils épinglée n'est donc pas détectée automatiquement ;
sélectionnez-la explicitement :

```sh
rustup toolchain install nightly-2026-08-23
cargo +nightly-2026-08-23 install netsuke-build
```

Des installateurs prêts à l'emploi sont disponibles depuis la version GitHub
[v0.1.0-beta3](https://github.com/leynos/netsuke/releases/tag/v0.1.0-beta3) :

| Plateforme | Architectures                       | Paquets                         |
| ---------- | ----------------------------------- | ------------------------------- |
| Linux      | x86-64 (`amd64`) et Arm64 (`arm64`) | Debian (`.deb`) et RPM (`.rpm`) |
| macOS      | Intel x86-64 et Apple silicon Arm64 | Paquet d'installation (`.pkg`)  |
| Windows    | x64 et Arm64                        | Installateur Windows (`.msi`)   |

Les paquets Linux installent la page de manuel `netsuke` et déclarent
`ninja-build` comme dépendance. Ninja doit être installé séparément lors de
l'utilisation de l'installateur macOS ou Windows. Le MSI Windows s'installe dans
`C:\Program Files\netsuke` et ne met pas à jour `PATH`. Des fichiers de somme
de contrôle SHA-256 accompagnent les binaires autonomes ainsi que les fichiers
d'aide et de licence intégrés. Les paquets d'installation n'ont pas de fichiers
de somme de contrôle associés dans la v0.1.0-beta3. Consultez le
[guide de l'utilisateur](docs/users-guide.md#install-netsuke) pour les
commandes spécifiques à chaque plateforme et la configuration sous Windows.

Pour installer le clone des sources actuel avec Cargo :

```sh
git clone https://github.com/leynos/netsuke.git
cd netsuke
cargo install --path .
```

### Votre premier build

Créez un nouveau répertoire et ajoutez un fichier nommé `Netsukefile`:

```yaml
netsuke_version: "1.0.0"

targets:
  - name: hello.txt
    command: "echo 'Hello from Netsuke!' > hello.txt"

defaults:
  - hello.txt
```

Exécutez Netsuke, puis inspectez le résultat :

```sh
netsuke
cat hello.txt
```

La seconde commande affiche `Hello from Netsuke!`. Consultez le
[guide de démarrage rapide](docs/quickstart.md) pour les variables, les modèles
et `foreach`, puis utilisez le
[guide de la bibliothèque standard de modèles](docs/stdlib-yaml-and-jinja-guide.md)
pour chaque assistant de chemin, de collection, de système de fichiers, de
temps, de commande, d'environnement, de glob et de réseau.

______________________________________________________________________

## Ce qui fonctionne aujourd'hui

Le compilateur de système de build principal de Netsuke v0.1.0-beta3 fournit :

- l'analyse de manifestes YAML 1.2 avec validation des clés en double et du
  schéma ;
- les variables Jinja, les macros, `foreach`, `when`, le glob, les
  assistants d'environnement, la découverte d'exécutables et des assistants
  réseau à activation explicite ;
- des règles, cibles, actions et valeurs par défaut réutilisables, ainsi que
  des dépendances explicites, implicites et sans ordre ;
- la découverte des cibles et des actions via `netsuke help targets`, y
  compris les entrées conditionnelles sans rendu de recette ;
- un graphe de compilation intermédiaire déterministe avec des
  vérifications de sorties en double, de règles manquantes et de cycles ;
- l'exécution des recettes historiques Windows via Windows PowerShell par
  défaut, avec une voie de compatibilité explicite via Git Bash ou MSYS2 ;
- la génération et l'exécution de Ninja, ainsi que `clean` et la génération
  autonome de manifestes ;
- des graphes de dépendances reproductibles au format Graphviz DOT ou en
  HTML autonome et accessible ;
- une configuration en couches, une sortie localisée, des préférences
  d'accessibilité, un suivi de progression, la mesure du temps par étape, et
  des résultats ou diagnostics JSON versionnés ;
- une couverture de tests unitaires, comportementaux, d'intégration, par
  propriétés, par snapshot, et une couverture initiale de vérification Kani.

La version beta3 prend également en charge les agrégats d'actions et de cibles
ne comportant que des dépendances : les nœuds dotés d'une liste `deps` non vide
peuvent omettre une recette.

______________________________________________________________________

## État de la version et du développement

La version v0.1.0-beta3 constitue un aperçu utile pour les premiers
utilisateurs, et non une déclaration selon laquelle Netsuke serait achevé ou
que chaque interface serait stable. Le pipeline du compilateur et le flux de
build local ordinaire sont substantiels ; l'interface en ligne de commande, le
vocabulaire de configuration et le modèle de recette avancé restent pré-stables.

Épinglez la version de Netsuke dans vos automatisations et attendez-vous à ce
que certains noms de commandes, options, schémas de diagnostic et détails de
manifeste changent avant la version 1.0.

Les limitations suivantes s'appliquent à la beta3.

Les limitations connues comprennent :

- les recettes restent des chaînes shell : les scripts Unix utilisent
  `/bin/sh -e`, les recettes historiques Windows utilisent Windows PowerShell
  par défaut, et la voie de compatibilité Bash sous Windows nécessite une
  activation explicite ; les arguments d'exécutable structurés et les mappages
  d'environnement de recette ne sont pas encore implémentés ;
- les imports de dépendances générés par le compilateur, comme les
  depfiles GCC, sont prévus mais ne font pas encore partie du modèle de
  manifeste ;
- `--json` émet exactement un document de résultat ou de diagnostic
  versionné par commande, mais le schéma peut encore changer avant la version
  1.0 ;
- le rendu en couleur n'est pas implémenté ;
- l'accessibilité nécessite encore une vérification avec des technologies
  d'assistance.

La version beta3 corrige la limitation du dollar shell de la beta2 grâce à un
échappement compatible avec Ninja, de sorte que les expressions shell
ordinaires peuvent être écrites normalement. Les manifestes beta2 utilisant des
expressions littérales de dollar shell nécessitent une migration ; voir la
[limite de sécurité du guide de l'utilisateur](docs/users-guide.md#review-the-safety-boundary).

Un `Netsukefile` peut exécuter des commandes et utiliser des assistants de
modèle impurs. Traitez-le avec la même prudence qu'un `Makefile`: examinez les
manifestes non fiables avant de les exécuter. Netsuke met entre guillemets les
substitutions de chemin prises en charge, mais ce n'est pas un bac à sable.

______________________________________________________________________

## La suite

Le travail postérieur à la première version s'organise autour de trois
priorités :

1. **Stabiliser le contrat de la ligne de commande** : renforcer les noms
   canoniques de commandes et d'options, les garde-fous non interactifs, des
   codes de sortie stables, une sortie bornée et des documents `--json`
   versionnés.
2. **Rendre les recettes plus sûres et plus claires** : ajouter des
   arguments d'exécutable structurés, des mappages d'environnement, des imports
   de dépendances du compilateur et un meilleur retour sur les actions
   conditionnelles.
3. **Renforcer la confiance** : étendre la couverture Kani et des tests par
   propriétés, vérifier l'accessibilité avec des technologies d'assistance et
   ajouter une couverture de non-régression pour le rendu du terminal.

Le travail à plus long terme explore le contexte lisible par machine, les
profils, l'historique des exécutions, la livraison d'artefacts et un retour
local-first pour les flux de travail humains et agentiques. La
[feuille de route](docs/roadmap.md) suit la séquence détaillée et la
progression actuelle.

______________________________________________________________________

## En savoir plus

- [Guide de démarrage rapide](docs/quickstart.md) — construisez quelque
  chose en cinq minutes.
- [Guide de l'utilisateur](docs/users-guide.md) — référence des manifestes
  et des commandes.
- [Document de conception](docs/netsuke-design.md) — architecture et
  justification des choix de conception.
- [Guide du développeur](docs/developers-guide.md) — flux de travail de
  développement et portes qualité.
- [Feuille de route](docs/roadmap.md) — fondations achevées et travaux
  prévus.

______________________________________________________________________

## Licence

ISC — voir [LICENSE](LICENSE) pour plus de détails.

______________________________________________________________________

## Contribuer

Les contributions sont les bienvenues. Commencez par le
[guide du développeur](docs/developers-guide.md) ; les contributeurs
automatisés doivent également suivre [AGENTS.md](AGENTS.md).
