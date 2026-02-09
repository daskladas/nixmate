//! Pattern text translations (German).
//!
//! Provides German translations for pattern explanations, solutions, and deep_dive texts.
//! English is the default language stored in patterns.rs.

use std::collections::HashMap;
use once_cell::sync::Lazy;
use super::matcher::MatchResult;
use super::patterns::library_to_package;

/// Translation templates for a pattern (with $1, $2 placeholders)
struct PatternTranslation {
    title: &'static str,
    explanation: &'static str,
    solution: &'static str,
    deep_dive: &'static str,
    tip: Option<&'static str>,
}

/// German translations for patterns, keyed by pattern ID
static TRANSLATIONS_DE: Lazy<HashMap<&'static str, PatternTranslation>> = Lazy::new(|| {
    let mut m = HashMap::new();

    m.insert("linker-missing-lib", PatternTranslation {
        title: "Linker kann Bibliothek nicht finden: $1",
        explanation: "Der Linker braucht die '$1'-Bibliothek, aber sie ist nicht verfügbar.",
        solution: "\
buildInputs = [ $1 ];
nativeBuildInputs = [ pkg-config ];",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix-Builds laufen in isolierten Umgebungen (Sandboxes). Anders als bei 
traditionellem Linux, wo Bibliotheken in /usr/lib global verfügbar sind, 
musst du bei Nix jede Abhängigkeit explizit deklarieren.

DER BUILD-PROZESS:
1. Compiler erstellt .o Objektdateien aus deinem Quellcode
2. Linker (ld) kombiniert Objekte + Bibliotheken zur Executable
3. Linker sucht -l<n> in Pfaden aus buildInputs
4. Wenn nicht gefunden -> dieser Fehler

buildInputs vs nativeBuildInputs:
- buildInputs = Bibliotheken für das ZIEL-System (Laufzeit)
- nativeBuildInputs = Werkzeuge für das BUILD-System (Compiler)

DAS RICHTIGE PAKET FINDEN:
Bibliotheksnamen entsprechen nicht immer Paketnamen:
  -lssl     -> openssl
  -lz       -> zlib  
  -lcrypto  -> openssl

Nutze: nix search nixpkgs <n>",
        tip: Some("Häufig: ssl->openssl, z->zlib, ffi->libffi"),
    });

    m.insert("missing-header", PatternTranslation {
        title: "Fehlender Header: $1",
        explanation: "Der Compiler kann die Header-Datei '$1' nicht finden.",
        solution: "\
# Finde das Paket:
nix-locate -w '*/$1'

# Füge es hinzu:
buildInputs = [ <paket> ];",
        deep_dive: "\
WARUM PASSIERT DAS:
Header-Dateien (.h) enthalten Deklarationen die dem Compiler sagen, welche 
Funktionen existieren. Sie werden zur COMPILE-Zeit benötigt.

In Nix liegen Header in /nix/store/<hash>-<pkg>/include/. Der Compiler 
durchsucht nur Pfade von Paketen in buildInputs.

DAS RICHTIGE PAKET FINDEN:
1. Nutze nix-locate (von nix-index):
   nix-locate -w '*/openssl/ssl.h'
   
2. Durchsuche nixpkgs:
   nix search nixpkgs openssl

HÄUFIGE HEADER -> PAKET:
  openssl/*.h  -> openssl
  curl/*.h     -> curl
  zlib.h       -> zlib
  python*.h    -> python3",
        tip: Some("Installiere nix-index für nix-locate"),
    });

    m.insert("undefined-reference", PatternTranslation {
        title: "Undefinierte Referenz: $1",
        explanation: "Der Linker fand eine Deklaration aber keine Implementierung für '$1'.",
        solution: "buildInputs = [ <bibliothek-mit-$1> ];",
        deep_dive: "\
WARUM PASSIERT DAS:
Dies ist ein LINKER-Fehler, kein Compiler-Fehler. Der Unterschied:
- Compiler-Fehler: 'unbekannte Funktion' -> fehlender Header
- Linker-Fehler: 'undefined reference' -> fehlende Bibliothek

Der Code wurde kompiliert (Header gefunden), aber beim Linken 
konnte die Implementierung von '$1' nicht gefunden werden.

HÄUFIGE URSACHEN:
1. Fehlende Bibliothek in buildInputs
2. Falsche Bibliotheksversion (API geändert)
3. C++ Name-Mangling (extern \"C\" fehlt)

REIHENFOLGE WICHTIG:
Wenn libA libB braucht:
  FALSCH:  buildInputs = [ libB libA ];
  RICHTIG: buildInputs = [ libA libB ];",
        tip: None,
    });

    m.insert("builder-failed", PatternTranslation {
        title: "Build fehlgeschlagen",
        explanation: "Die Derivation konnte nicht gebaut werden. Der Fehler steht oben.",
        solution: "\
# Vollständiges Log anzeigen:
nix log $1

# Mit ausführlicher Ausgabe bauen:
nix build -L",
        deep_dive: "\
DIESEN FEHLER VERSTEHEN:
Diese Meldung erscheint am ENDE eines fehlgeschlagenen Builds. Der 
echte Fehler steht ÜBER dieser Zeile.

WIE NIX BUILDS FUNKTIONIEREN:
1. Nix wertet Ausdruck aus -> erstellt Derivation (.drv)
2. Derivation spezifiziert: Inputs, Build-Skript, Outputs
3. Nix führt Builder in Sandbox aus
4. Wenn Builder mit nicht-Null beendet -> dieser Fehler

DEN ECHTEN FEHLER FINDEN:
1. Im Terminal nach oben scrollen
2. nix log /nix/store/<hash>.drv
3. nix build -L (streamt Logs)

HÄUFIGE URSACHEN:
- Fehlende Abhängigkeit (buildInputs)
- Hardcodierte Pfade (/usr/bin/...)
- Netzwerkzugriff in Sandbox
- Fehlendes Build-Tool (nativeBuildInputs)",
        tip: Some("Der echte Fehler steht meist ÜBER dieser Zeile"),
    });

    m.insert("attribute-missing", PatternTranslation {
        title: "Attribut '$1' nicht gefunden",
        explanation: "Das Attribut '$1' existiert nicht in diesem Set.",
        solution: "\
# Im nix repl erkunden:
nix repl -f '<nixpkgs>'
nix-repl> pkgs.<TAB>

# Suchen: https://search.nixos.org/packages",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix Attribute-Sets sind wie Dictionaries. Wenn du pkgs.foo schreibst, 
greifst du auf 'foo' zu. Existiert es nicht -> dieser Fehler.

HÄUFIGE URSACHEN:

1. TIPPFEHLER:
   pkgs.python3Pkgs     # FALSCH
   pkgs.python3Packages # RICHTIG

2. PAKET UMBENANNT/ENTFERNT:
   Prüfe: https://search.nixos.org/packages

3. FEHLENDER INPUT (Flakes):
   outputs = { nixpkgs, ... }:  # muss in inputs sein

ATTRIBUTE-SETS ERKUNDEN:
  $ nix repl -f '<nixpkgs>'
  nix-repl> pkgs.python<TAB>
  nix-repl> builtins.attrNames pkgs.python3Packages

PRÜFEN OB ATTRIBUT EXISTIERT:
  pkgs.foo or null     # null wenn fehlend
  pkgs ? foo           # true/false",
        tip: Some("python3Packages nicht pythonPackages"),
    });

    m.insert("infinite-recursion", PatternTranslation {
        title: "Unendliche Rekursion",
        explanation: "Nix hat eine zirkuläre Abhängigkeit entdeckt.",
        solution: "\
# In Overlays - nutze 'prev' nicht 'final':
(final: prev: {
  pkg = prev.pkg.override { };  # prev!
})

# In Modulen - config nicht in options:
options.x = mkOption { default = 42; };",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix ist lazy, erkennt aber Zyklen. Wenn A B braucht und B A braucht 
-> dieser Fehler.

HÄUFIGE URSACHEN:

1. OVERLAYS - 'final' statt 'prev':
   # FALSCH:
   (final: prev: {
     myPkg = final.myPkg.override { };  # Rekursion!
   })
   
   # RICHTIG:
   (final: prev: {
     myPkg = prev.myPkg.override { };
   })

2. NIXOS MODULE - config in Defaults:
   # FALSCH:
   options.foo.port = mkOption {
     default = config.bar.port;  # config nicht bereit!
   };
   
   # RICHTIG - im config-Abschnitt:
   config.foo.port = mkDefault config.bar.port;

DEBUGGING:
  nix build --show-trace",
        tip: Some("Nutze --show-trace um die Quelle zu finden"),
    });

    m.insert("undefined-variable", PatternTranslation {
        title: "Undefinierte Variable: $1",
        explanation: "'$1' ist in diesem Scope nicht definiert.",
        solution: "\
# Zu Funktionsargumenten hinzufügen:
{ pkgs, lib, $1, ... }:

# Oder importieren:
let $1 = import ./file.nix; in ...",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix hat lexikalisches Scoping - Variablen müssen vor Verwendung 
definiert werden. Funktionen müssen Inputs explizit deklarieren.

HÄUFIGE URSACHEN:

1. FEHLENDES ARGUMENT:
   # FALSCH:
   { }: pkgs.hello
   
   # RICHTIG:
   { pkgs }: pkgs.hello

2. FALSCHER SCOPE:
   # FALSCH:
   let foo = 1; in bar
   
   # RICHTIG:
   let foo = 1; bar = 2; in bar

3. FLAKE OUTPUTS:
   # FALSCH:
   outputs = { self }: { ... nixpkgs ... }
   
   # RICHTIG:
   outputs = { self, nixpkgs }: { ... }

SPEZIELLE VARIABLEN:
- pkgs: Argument oder import <nixpkgs> {}
- lib: von pkgs.lib
- config: NixOS Modul-Argument
- builtins: immer global verfügbar",
        tip: Some("pkgs, lib, config müssen in Funktionsargs sein"),
    });

    m.insert("type-error", PatternTranslation {
        title: "Typfehler: erwartet $1, bekommen $2",
        explanation: "Nix erwartete '$1' aber erhielt '$2'.",
        solution: "\
# Häufige Fixes:
packages = [ pkgs.git ];  # Liste, nicht einzeln
enable = true;            # bool, nicht \"true\"
port = 8080;              # int, nicht \"8080\"",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix ist dynamisch typisiert aber erzwingt Typen zur Laufzeit.

HÄUFIGE TYP-MISMATCHES:

1. LISTE vs EINZELWERT:
   # FALSCH:
   environment.systemPackages = pkgs.git;
   # RICHTIG:
   environment.systemPackages = [ pkgs.git ];

2. BOOL vs STRING:
   # FALSCH:
   services.nginx.enable = \"true\";
   # RICHTIG:
   services.nginx.enable = true;

3. INT vs STRING:
   # FALSCH:
   port = \"8080\";
   # RICHTIG:
   port = 8080;

TYP-PRÜFUNG:
  builtins.typeOf x
  builtins.isList x
  builtins.isString x",
        tip: None,
    });

    m.insert("cannot-coerce", PatternTranslation {
        title: "Kann nicht zu String konvertieren",
        explanation: "Nix kann diesen Wert nicht automatisch zu String konvertieren.",
        solution: "\
# Für Derivations:
\"${pkgs.hello}/bin/hello\"

# Für Sets - greife auf Attribut zu:
mySet.name  # nicht mySet",
        deep_dive: "\
WARUM PASSIERT DAS:
String-Interpolation versucht Werte zu Strings zu konvertieren.
Nix kann manche Typen automatisch konvertieren, aber nicht Sets.

WAS INTERPOLIERT WERDEN KANN:
- Strings, Pfade, Derivations, Zahlen (mit toString)

WAS NICHT GEHT:
- Sets, Listen, Funktionen

LÖSUNGEN:
1. Spezifisches Attribut zugreifen:
   \"${pkgs.python3}/bin/python\"

2. toString nutzen:
   \"${toString 42}\"

3. toJSON nutzen:
   \"${builtins.toJSON mySet}\"",
        tip: None,
    });

    m.insert("syntax-error", PatternTranslation {
        title: "Syntaxfehler bei: $1",
        explanation: "Der Nix-Parser fand unerwartete Eingabe.",
        solution: "\
{ a = 1; b = 2; }  # Semikolons nach Attributen
[ a b c ]          # Keine Kommas in Listen  
{ a, b }: ...      # Kommas in Funktionsargs",
        deep_dive: "\
NIX SYNTAX-FALLEN:

1. SEMIKOLONS nach jedem Attribut:
   # FALSCH: { a = 1, b = 2 }
   # RICHTIG: { a = 1; b = 2; }

2. KEINE KOMMAS in Listen:
   # FALSCH: [ \"a\", \"b\" ]
   # RICHTIG: [ \"a\" \"b\" ]

3. KOMMAS in Funktionsargs:
   { a, b, c }: a + b + c

4. PFAD vs STRING:
   ./foo       # Pfad
   \"./foo\"     # String - KEIN Pfad!

FEHLERPOSITION KANN FALSCH SEIN:
Prüfe auch Zeilen VOR der gemeldeten Position.",
        tip: Some("Fehlerposition kann ungenau sein"),
    });

    m.insert("flake-no-output", PatternTranslation {
        title: "Flake hat keinen Output '$2'",
        explanation: "Das Flake '$1' hat nicht den angeforderten Output.",
        solution: "\
# Verfügbare Outputs auflisten:
nix flake show $1

# Häufige Pfade:
packages.x86_64-linux.default
devShells.x86_64-linux.default",
        deep_dive: "\
WARUM PASSIERT DAS:
Flakes haben ein strukturiertes Output-Schema. Du musst den exakten 
Pfad inkl. System-Architektur verwenden.

FLAKE OUTPUT-STRUKTUR:
  packages.<system>.<name>
  devShells.<system>.<name>
  nixosConfigurations.<name>

SYSTEM ist meist:
  x86_64-linux   (Linux)
  aarch64-linux  (ARM/RPi)
  x86_64-darwin  (Intel Mac)
  aarch64-darwin (Apple Silicon)

OUTPUTS AUFLISTEN:
  nix flake show",
        tip: Some("Vergiss nicht das System: x86_64-linux"),
    });

    m.insert("flake-input-missing", PatternTranslation {
        title: "Input '$1' nicht deklariert",
        explanation: "Der Input '$1' wird genutzt aber nicht in flake.nix deklariert.",
        solution: "\
# In flake.nix hinzufügen:
inputs.$1.url = \"github:owner/repo\";
outputs = { $1, ... }: { };",
        deep_dive: "\
WARUM PASSIERT DAS:
Flake-Inputs müssen in 'inputs' deklariert UND an 'outputs' übergeben werden.

FLAKE-STRUKTUR:
{
  inputs = {
    nixpkgs.url = \"github:NixOS/nixpkgs\";
  };
  outputs = { self, nixpkgs, ... }: { };
}

INPUTS UPDATEN:
  nix flake update
  nix flake lock --update-input nixpkgs",
        tip: None,
    });

    m.insert("hash-mismatch", PatternTranslation {
        title: "Hash-Mismatch",
        explanation: "Download entspricht nicht dem erwarteten Hash.",
        solution: "\
# Nutze den korrekten Hash:
hash = \"$1\";",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix erfordert dass Downloads ihren Hash vorab deklarieren.

DEN RICHTIGEN HASH BEKOMMEN:
1. nix-prefetch-url <url>
2. lib.fakeHash temporär nutzen
   (Build schlägt fehl mit korrektem Hash)

HASH-FORMATE:
  Alt: sha256 = \"0abc123...\";
  Neu: hash = \"sha256-ABC...\";  # bevorzugt",
        tip: Some("Nutze lib.fakeHash während Entwicklung"),
    });

    m.insert("download-failed", PatternTranslation {
        title: "Download fehlgeschlagen",
        explanation: "Konnte nicht von '$1' herunterladen.",
        solution: "\
# Prüfe ob URL funktioniert:
curl -I \"$1\"

# Versuche neuere Version oder Mirror",
        deep_dive: "\
HÄUFIGE URSACHEN:
1. URL existiert nicht mehr
2. Netzwerk/Firewall-Problem
3. Rate Limiting

LÖSUNGEN:
- Neue URL oder andere Version finden
- Mirror nutzen
- Warten bei Rate Limiting",
        tip: None,
    });

    m.insert("option-not-exist", PatternTranslation {
        title: "Option '$1' existiert nicht",
        explanation: "Diese NixOS-Option existiert nicht.",
        solution: "\
# Suchen: https://search.nixos.org/options

# Wenn aus Modul:
imports = [ module.nixosModules.default ];",
        deep_dive: "\
HÄUFIGE URSACHEN:
1. Tippfehler (enabel statt enable)
2. Falscher Pfad
3. Modul nicht importiert
4. Option umbenannt/entfernt

OPTIONEN FINDEN:
  https://search.nixos.org/options
  nixos-option services.nginx",
        tip: Some("Optionen sind case-sensitive"),
    });

    m.insert("assertion-failed", PatternTranslation {
        title: "Assertion fehlgeschlagen",
        explanation: "Eine NixOS-Modul-Prüfung ist fehlgeschlagen.",
        solution: "\
# Häufige Fixes:
hardware.enableRedistributableFirmware = true;
users.groups.mygroup = {};",
        deep_dive: "\
Module nutzen Assertions um Anforderungen durchzusetzen.
Die Nachricht nach 'Failed assertions:' erklärt was fehlt.

HÄUFIGE FIXES:
- Fehlender Benutzer/Gruppe anlegen
- Anderen Dienst aktivieren
- allowUnfree aktivieren
- Firmware aktivieren",
        tip: Some("Lies die Assertion-Nachricht genau"),
    });

    m.insert("collision", PatternTranslation {
        title: "Datei-Kollision",
        explanation: "Pakete '$1' und '$2' stellen dieselbe Datei bereit.",
        solution: "\
# Priorität setzen:
environment.systemPackages = [
  (lib.hiPrio pkgs.package1)  # gewinnt
  pkgs.package2
];",
        deep_dive: "\
Zwei Pakete installieren dieselbe Datei.

LÖSUNGEN:
1. lib.hiPrio nutzen (gewinnt)
2. lib.lowPrio nutzen (verliert)
3. Ein Paket entfernen

Niedrigere Prioritätszahl = gewinnt",
        tip: None,
    });

    m.insert("python-module-not-found", PatternTranslation {
        title: "Python-Modul nicht gefunden: $1",
        explanation: "Das Python-Modul '$1' ist nicht installiert.",
        solution: "\
# In shell.nix oder flake.nix:
python3.withPackages (ps: [ ps.$1 ])

# Oder:
propagatedBuildInputs = [ python3Packages.$1 ];",
        deep_dive: "\
In Nix müssen Python-Pakete explizit aufgelistet werden.

BEISPIEL:
  mkShell {
    packages = [
      (python3.withPackages (ps: [ ps.numpy ps.pandas ]))
    ];
  }

PAKETE FINDEN:
  nix search nixpkgs python3Packages.<n>",
        tip: Some("nix search nixpkgs python3Packages"),
    });

    m.insert("python-import-error", PatternTranslation {
        title: "Python Import-Fehler: $1 aus $2",
        explanation: "Kann '$1' nicht aus '$2' importieren. Versionskonflikt?",
        solution: "\
# Paketversion prüfen:
nix eval nixpkgs#python3Packages.$2.version",
        deep_dive: "\
Das Symbol '$1' existiert nicht im Modul '$2'.

URSACHEN:
1. Versionskonflikt
2. API geändert
3. Falscher Import-Pfad",
        tip: None,
    });

    m.insert("rust-crate-not-found", PatternTranslation {
        title: "Rust Crate nicht gefunden: $1",
        explanation: "Das Rust Crate '$1' kann nicht gefunden werden.",
        solution: "\
# In Cargo.toml:
[dependencies]
$1 = \"*\"

# Für System-Libs:
buildInputs = [ openssl ];",
        deep_dive: "\
Crates die C-Bibliotheken linken brauchen diese in buildInputs.

HÄUFIG:
  openssl-sys -> openssl, pkg-config
  rusqlite    -> sqlite",
        tip: Some("Prüfe ob Crate C-Bibliothek braucht"),
    });

    m.insert("rust-linker-native", PatternTranslation {
        title: "Rust fehlt native Lib: $1",
        explanation: "Rust kann die native Bibliothek '$1' nicht finden.",
        solution: "\
buildInputs = [ $1 ];
nativeBuildInputs = [ pkg-config ];",
        deep_dive: "\
Ein Rust Crate versucht gegen eine C-Bibliothek zu linken.

LÖSUNG:
  buildInputs = [ openssl ];
  nativeBuildInputs = [ pkg-config ];",
        tip: None,
    });

    m.insert("node-module-not-found", PatternTranslation {
        title: "Node-Modul nicht gefunden: $1",
        explanation: "Das Node.js Modul '$1' ist nicht installiert.",
        solution: "\
# In shell.nix:
nodePackages.$1

# Oder mit npm:
buildInputs = [ nodejs ];",
        deep_dive: "\
Node.js Module sind in Nix nicht global verfügbar.

OPTIONEN:
1. nodePackages (begrenzt)
2. buildNpmPackage
3. node2nix
4. dream2nix",
        tip: Some("Nutze node2nix für komplexe Projekte"),
    });

    m.insert("permission-denied-nix-store", PatternTranslation {
        title: "Zugriff verweigert im Nix Store",
        explanation: "Versuch in den schreibgeschützten Nix Store zu schreiben.",
        solution: "\
# Nutze $out für Outputs:
mkdir -p $out/bin

# Für temp Dateien:
export HOME=$TMPDIR",
        deep_dive: "\
Der Nix Store (/nix/store) ist SCHREIBGESCHÜTZT.

LÖSUNGEN:
1. Output nach $out
2. Temp-Verzeichnis ($TMPDIR)
3. wrapProgram für Runtime",
        tip: Some("Schreibe niemals in /nix/store"),
    });

    m.insert("path-not-in-store", PatternTranslation {
        title: "Pfad nicht im Nix Store: $1",
        explanation: "Der Pfad '$1' muss erst in den Nix Store kopiert werden.",
        solution: "\
# Nutze ./pfad für lokale Dateien:
src = ./my-source;

# Oder fetchen:
src = fetchurl { url = \"...\"; hash = \"...\"; };",
        deep_dive: "\
Nix kann nur nutzen:
1. Pfade in /nix/store
2. Lokale Pfade (./foo)
3. Gefetchte URLs

WICHTIG:
  ./foo     # PFAD (wird kopiert)
  \"./foo\"   # STRING (kein Pfad!)",
        tip: None,
    });

    // =========================================================================
    // NEW PATTERNS - DAEMON / STORE
    // =========================================================================
    m.insert("cannot-connect-daemon", PatternTranslation {
        title: "Kann nicht zum Nix-Daemon verbinden",
        explanation: "Der Nix-Daemon läuft nicht oder ist nicht erreichbar.",
        solution: "\
# Auf NixOS:
sudo systemctl start nix-daemon

# Status prüfen:
systemctl status nix-daemon",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix nutzt einen Daemon für Multi-User-Installationen. Der Daemon 
verwaltet /nix/store und führt Builds aus.

HÄUFIGE URSACHEN:
1. Daemon nicht gestartet (nach Neustart)
2. Socket-Berechtigungsproblem
3. Daemon abgestürzt

FIXEN:
  # NixOS:
  sudo systemctl restart nix-daemon
  
  # Anderes Linux:
  sudo systemctl enable nix-daemon
  sudo systemctl start nix-daemon",
        tip: Some("Auf NixOS startet dies automatisch"),
    });

    m.insert("experimental-features", PatternTranslation {
        title: "Experimentelles Feature deaktiviert: $1",
        explanation: "Das Feature '$1' erfordert explizite Aktivierung.",
        solution: "\
# Temporär:
nix --experimental-features '$1' <befehl>

# Permanent (~/.config/nix/nix.conf):
experimental-features = nix-command flakes",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix hat 'experimentelle' Features die nicht standardmäßig aktiv sind.
Die häufigsten: 'nix-command' (neue CLI) und 'flakes'.

PERMANENT AKTIVIEREN:

~/.config/nix/nix.conf:
  experimental-features = nix-command flakes

NixOS configuration.nix:
  nix.settings.experimental-features = [ \"nix-command\" \"flakes\" ];",
        tip: Some("Die meisten aktivieren 'nix-command flakes' permanent"),
    });

    m.insert("store-path-not-valid", PatternTranslation {
        title: "Ungültiger Store-Pfad",
        explanation: "Ein referenzierter Store-Pfad existiert nicht oder ist beschädigt.",
        solution: "\
# Store verifizieren und reparieren:
nix-store --verify --check-contents --repair

# Oder Pfad neu bauen:
nix-store --realise <pfad>",
        deep_dive: "\
WARUM PASSIERT DAS:
Store-Pfade können ungültig werden durch:
1. Garbage Collection entfernte benötigte Pfade
2. Unterbrochene Builds
3. Manuelle Löschung aus /nix/store

FIXEN:
  nix-store --verify --check-contents --repair",
        tip: Some("Versuche: nix-store --verify --repair"),
    });

    m.insert("cached-failure", PatternTranslation {
        title: "Gecachter Build-Fehler",
        explanation: "Ein vorheriger Build schlug fehl und der Fehler ist gecacht.",
        solution: "\
# Failure-Cache leeren und neu versuchen:
nix build --rebuild

# Oder alle Failures löschen:
nix-store --delete $(nix-store -q --failed)",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix cacht Build-Fehler um wiederholte fehlgeschlagene Builds zu 
vermeiden. Das kann störend sein wenn du das Problem behoben hast.

CACHE LEEREN:
  nix build --rebuild .#package
  # Oder:
  nix-store --delete $(nix-store -q --failed)",
        tip: Some("Nutze --rebuild um gecachten Fehler zu ignorieren"),
    });

    // =========================================================================
    // HOME-MANAGER
    // =========================================================================
    m.insert("home-manager-not-found", PatternTranslation {
        title: "Home-Manager nicht gefunden",
        explanation: "Home-Manager ist nicht verfügbar. Fehlender Input oder Import?",
        solution: "\
# In flake.nix inputs:
inputs.home-manager = {
  url = \"github:nix-community/home-manager\";
  inputs.nixpkgs.follows = \"nixpkgs\";
};

# In outputs:
outputs = { home-manager, ... }: { };",
        deep_dive: "\
WARUM PASSIERT DAS:
Home-Manager ist ein separates Projekt, nicht Teil von nixpkgs.
Du musst es explizit als Input hinzufügen.

FLAKE SETUP:
{
  inputs = {
    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";
    home-manager = {
      url = \"github:nix-community/home-manager\";
      inputs.nixpkgs.follows = \"nixpkgs\";
    };
  };
  outputs = { nixpkgs, home-manager, ... }: { };
}",
        tip: Some("Vergiss nicht 'inputs.nixpkgs.follows'"),
    });

    m.insert("home-option-not-exist", PatternTranslation {
        title: "Home-Manager Option '$1' existiert nicht",
        explanation: "Diese Home-Manager Option existiert nicht. Tippfehler oder fehlendes Modul?",
        solution: "\
# Optionen suchen:
# https://nix-community.github.io/home-manager/options.html

# Falls Programm-Modul, erst aktivieren:
programs.git.enable = true;",
        deep_dive: "\
HÄUFIGE FEHLER:

1. PROGRAMM NICHT AKTIVIERT:
   # FALSCH:
   programs.git.userName = \"ich\";
   
   # RICHTIG:
   programs.git.enable = true;
   programs.git.userName = \"ich\";

2. FALSCHER PFAD:
   home.programs.git  # FALSCH
   programs.git       # RICHTIG",
        tip: Some("Erst Programm aktivieren: programs.X.enable = true"),
    });

    m.insert("home-file-collision", PatternTranslation {
        title: "Home-Manager Datei-Kollision: $1",
        explanation: "Datei '$1' existiert bereits und HM überschreibt sie nicht.",
        solution: "\
# Option 1 - Backup und HM verwalten lassen:
mv ~/.config/file ~/.config/file.backup

# Option 2 - Überschreiben erzwingen:
home.file.\"pfad\".force = true;",
        deep_dive: "\
WARUM PASSIERT DAS:
Home-Manager weigert sich bestehende Dateien zu überschreiben die 
es nicht verwaltet. Das verhindert Datenverlust.

LÖSUNGEN:
1. Datei backuppen und entfernen
2. home.file.\"pfad\".force = true;
3. HM von Anfang an verwalten lassen",
        tip: Some("Erst Datei backuppen, dann HM verwalten lassen"),
    });

    // =========================================================================
    // FUNCTION / ARGUMENT ERRORS
    // =========================================================================
    m.insert("function-expects-argument", PatternTranslation {
        title: "Funktion fehlt Argument: $2",
        explanation: "Funktion '$1' erwartet Argument '$2', aber es wurde nicht übergeben.",
        solution: "\
# Fehlendes Argument hinzufügen:
myFunction {
  $2 = <wert>;
  # ... andere args
}",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix-Funktionen mit Attribut-Set-Parametern können Pflichtargumente haben.

FUNKTIONSTYPEN:

1. PFLICHT-ARGUMENTE:
   f = { a, b }: a + b;
   f { a = 1; }  # FEHLER: 'b' fehlt

2. OPTIONAL MIT DEFAULT:
   f = { a, b ? 0 }: a + b;
   f { a = 1; }  # OK: b ist 0

3. MIT ... (EXTRA ARGS ERLAUBT):
   f = { a, ... }: a;
   f { a = 1; c = 2; }  # OK: 'c' ignoriert",
        tip: None,
    });

    m.insert("unexpected-argument", PatternTranslation {
        title: "Unerwartetes Argument: $1",
        explanation: "Funktion akzeptiert Argument '$1' nicht.",
        solution: "\
# Argument entfernen oder Funktionssignatur prüfen:
# Die Funktion hat vielleicht kein '...'

# Falls du die Funktion kontrollierst:
myFunc = { known, args, ... }: ...",
        deep_dive: "\
WARUM PASSIERT DAS:
Die Funktionsparameter enthalten dieses Argument nicht und haben 
kein '...' um Extra-Argumente zu akzeptieren.

BEISPIEL:
  f = { a, b }: a + b;
  f { a = 1; b = 2; c = 3; }  # FEHLER: 'c' unerwartet
  
  f = { a, b, ... }: a + b;
  f { a = 1; b = 2; c = 3; }  # OK: 'c' ignoriert",
        tip: Some("Prüfe auf Tippfehler im Argument-Namen"),
    });

    m.insert("not-a-function", PatternTranslation {
        title: "Keine Funktion (ist $1)",
        explanation: "Versuch $1 aufzurufen als wäre es eine Funktion.",
        solution: "\
# Typ prüfen:
builtins.typeOf x

# Häufiger Fix - Attrset nicht aufrufen:
pkgs.hello      # Derivation (korrekt)
pkgs.hello { }  # FALSCH - hello ist keine Funktion",
        deep_dive: "\
HÄUFIGE FEHLER:

1. DERIVATION AUFRUFEN:
   # FALSCH:
   pkgs.hello { }
   # RICHTIG:
   pkgs.hello

2. FEHLENDER IMPORT:
   # FALSCH:
   ./file.nix { }
   # RICHTIG:
   (import ./file.nix) { }

3. FALSCHES OVERRIDE:
   # FALSCH:
   pkgs.hello { patches = []; }
   # RICHTIG:
   pkgs.hello.override { }
   pkgs.hello.overrideAttrs (old: { })",
        tip: Some("Nutze builtins.typeOf zum Prüfen"),
    });

    // =========================================================================
    // FLAKE ADVANCED
    // =========================================================================
    m.insert("flake-lock-outdated", PatternTranslation {
        title: "Flake Input veraltet: $1",
        explanation: "Die flake.lock ist nicht aktuell mit flake.nix.",
        solution: "\
# Alle Inputs updaten:
nix flake update

# Spezifischen Input updaten:
nix flake lock --update-input $1",
        deep_dive: "\
WARUM PASSIERT DAS:
flake.lock pinnt exakte Versionen der Inputs. Wenn du flake.nix 
änderst, muss die Lock-Datei aktualisiert werden.

BEFEHLE:
  nix flake update           # Alle updaten
  nix flake lock --update-input nixpkgs  # Einen updaten
  rm flake.lock && nix flake lock  # Neu erstellen",
        tip: Some("'nix flake update' nach Input-Änderungen"),
    });

    m.insert("flake-follows-not-found", PatternTranslation {
        title: "Follows nicht-existenten Input: $1",
        explanation: "Input versucht '$1' zu folgen, aber dieser Input existiert nicht.",
        solution: "\
# Stelle sicher dass der gefolgte Input existiert:
inputs.nixpkgs.url = \"...\";
inputs.home-manager.inputs.nixpkgs.follows = \"nixpkgs\";",
        deep_dive: "\
WARUM PASSIERT DAS:
'follows' sagt einem Input eine andere Version zu nutzen.
Das Ziel muss existieren.

BEISPIEL:
{
  inputs = {
    nixpkgs.url = \"...\";
    home-manager = {
      url = \"github:nix-community/home-manager\";
      inputs.nixpkgs.follows = \"nixpkgs\";  # UNSER nixpkgs
    };
  };
}

HÄUFIGE FEHLER:
1. Tippfehler im gefolgten Input-Namen
2. Vergessen den Input zu deklarieren",
        tip: Some("Schreibweise des gefolgten Inputs prüfen"),
    });

    // =========================================================================
    // BUILD PHASE ERRORS
    // =========================================================================
    m.insert("substitute-in-place-failed", PatternTranslation {
        title: "substituteInPlace Pattern nicht gefunden",
        explanation: "Das zu ersetzende Pattern wurde in der Datei nicht gefunden.",
        solution: "\
# Exakten Inhalt prüfen:
cat $src/pfad/zur/datei | grep 'pattern'

# Flexibleres Pattern oder --replace-warn:
substituteInPlace datei --replace-warn 'alt' 'neu'",
        deep_dive: "\
WARUM PASSIERT DAS:
substituteInPlace braucht exakten Pattern-Match.

HÄUFIGE URSACHEN:
1. Whitespace-Unterschiede
2. Pattern in neuer Version geändert
3. Datei existiert nicht

DEBUGGING:
  postPatch = ''
    cat pfad/zur/datei
  '';

  # Nicht fehlschlagen:
  substituteInPlace file --replace-warn 'alt' 'neu'",
        tip: Some("--replace-warn schlägt nicht fehl bei fehlendem Pattern"),
    });

    m.insert("patch-failed", PatternTranslation {
        title: "Patch konnte nicht angewandt werden",
        explanation: "Eine Patch-Datei konnte nicht auf den Quellcode angewandt werden.",
        solution: "\
# Patch für neue Version regenerieren:
diff -u original modified > fix.patch

# Oder patchFlags nutzen:
patchFlags = [ \"-p0\" ];",
        deep_dive: "\
WARUM PASSIERT DAS:
Patches haben Kontext-Zeilen die matchen müssen. Wenn sich 
der Quellcode ändert, passen Patches oft nicht mehr.

DEBUGGING:
1. Patch-Level prüfen (default -p1)
2. Patch neu generieren
3. Fuzz versuchen: patchFlags = [ \"-F3\" ];",
        tip: Some("Patches brechen oft bei Version-Updates"),
    });

    m.insert("patchshebangs-failed", PatternTranslation {
        title: "patchShebangs fehlgeschlagen",
        explanation: "Script-Interpreter konnte nicht gefunden oder gepatcht werden.",
        solution: "\
# Interpreter zu nativeBuildInputs hinzufügen:
nativeBuildInputs = [ bash python3 ];

# Oder Patching überspringen:
dontPatchShebangs = true;",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix patcht automatisch #!/usr/bin/env python zu Nix-Store-Pfaden.
Wenn der Interpreter nicht in der Build-Umgebung ist, schlägt dies fehl.

LÖSUNGEN:
1. Interpreter zu Build hinzufügen
2. dontPatchShebangs = true;
3. Manuell patchen in postFixup",
        tip: Some("Interpreter zu nativeBuildInputs hinzufügen"),
    });

    m.insert("ifd-disabled", PatternTranslation {
        title: "Import From Derivation (IFD) deaktiviert",
        explanation: "Versuch Nix-Code aus Build-Ergebnis zu importieren, aber IFD ist deaktiviert.",
        solution: "\
# IFD aktivieren (wenn du das System kontrollierst):
nix.settings.allow-import-from-derivation = true;

# Oder refaktorieren um IFD zu vermeiden",
        deep_dive: "\
WAS IST IFD:
Import From Derivation bedeutet .nix Dateien zu importieren die 
von einem Build produziert werden. Es ist manchmal deaktiviert 
weil es die Evaluation verlangsamt.

WARUM PROBLEMATISCH:
- Blockiert parallele Evaluation
- Muss bauen bevor Eval weitergehen kann
- Hydra/CI deaktiviert es oft

AKTIVIEREN:
  nix.settings.allow-import-from-derivation = true;",
        tip: Some("IFD verlangsamt Evaluation - wenn möglich vermeiden"),
    });

    // =========================================================================
    // SYSTEM / PLATFORM
    // =========================================================================
    m.insert("unsupported-system", PatternTranslation {
        title: "System nicht unterstützt: $1",
        explanation: "Dieses Paket unterstützt deine System-Architektur nicht.",
        solution: "\
# Unterstützte Plattformen prüfen:
nix eval nixpkgs#hello.meta.platforms

# Für Cross-Compilation:
nix build .#packages.x86_64-linux.hello",
        deep_dive: "\
WARUM PASSIERT DAS:
Nicht alle Pakete funktionieren auf allen Systemen:
1. Nur-Binär-Pakete (Steam, Spotify)
2. Plattform-spezifischer Code
3. Fehlende Cross-Compilation

SYSTEME IN NIX:
- x86_64-linux: Die meisten Linux-PCs
- aarch64-linux: ARM Linux (Raspberry Pi)
- x86_64-darwin: Intel Macs
- aarch64-darwin: Apple Silicon Macs",
        tip: Some("meta.platforms für unterstützte Systeme prüfen"),
    });

    m.insert("unfree-not-allowed", PatternTranslation {
        title: "Unfreies Paket nicht erlaubt",
        explanation: "Dieses Paket hat eine nicht-freie Lizenz und unfree ist nicht aktiviert.",
        solution: "\
# Alle unfree erlauben (NixOS):
nixpkgs.config.allowUnfree = true;

# Oder pro Paket:
nixpkgs.config.allowUnfreePredicate = pkg:
  builtins.elem (lib.getName pkg) [ \"steam\" ];",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix respektiert Software-Freiheit standardmäßig. Pakete mit 
proprietären Lizenzen müssen explizit erlaubt werden.

AKTIVIEREN:
  # NixOS:
  nixpkgs.config.allowUnfree = true;
  
  # Flakes:
  import nixpkgs { config.allowUnfree = true; };
  
  # Umgebungsvariable:
  NIXPKGS_ALLOW_UNFREE=1 nix build ...",
        tip: Some("allowUnfreePredicate für selektives Unfree nutzen"),
    });

    m.insert("broken-package", PatternTranslation {
        title: "Paket ist als broken markiert",
        explanation: "Dieses Paket ist als defekt in nixpkgs bekannt.",
        solution: "\
# Broken erlauben (nicht empfohlen):
nixpkgs.config.allowBroken = true;

# Besser: Prüfen warum es broken ist",
        deep_dive: "\
WARUM PASSIERT DAS:
Pakete werden als broken markiert wenn:
1. Build konsistent fehlschlägt
2. Kritische Bugs
3. Sicherheitsprobleme
4. Unmaintained

BESSERE OPTIONEN:
1. Alternatives Paket finden
2. Älteres nixpkgs nutzen
3. Fixen und zu nixpkgs beitragen",
        tip: Some("nixpkgs Issues für das Paket prüfen"),
    });

    m.insert("insecure-package", PatternTranslation {
        title: "Paket hat Sicherheitslücken",
        explanation: "Dieses Paket hat bekannte Sicherheitsprobleme.",
        solution: "\
# Schwachstellen prüfen:
nix eval nixpkgs#pkg.meta.knownVulnerabilities

# Falls nötig:
nixpkgs.config.permittedInsecurePackages = [
  \"openssl-1.1.1w\"
];",
        deep_dive: "\
WARUM PASSIERT DAS:
Nixpkgs trackt CVEs und markiert verwundbare Pakete.

ERLAUBEN (MIT VORSICHT):
  nixpkgs.config.permittedInsecurePackages = [
    \"electron-25.9.0\"
    \"openssl-1.1.1w\"
  ];

BESSERE OPTIONEN:
1. Auf gefixte Version updaten
2. Sichere Alternative finden
3. In Container/VM isolieren",
        tip: Some("Wenn möglich auf gepatchte Version updaten"),
    });

    // =========================================================================
    // GC / STORE MANAGEMENT
    // =========================================================================
    m.insert("gc-root-protected", PatternTranslation {
        title: "Kann nicht löschen: Pfad ist GC-Root",
        explanation: "Dieser Pfad ist durch einen Garbage-Collector-Root geschützt.",
        solution: "\
# GC Roots auflisten:
nix-store --gc --print-roots

# Root entfernen:
rm /nix/var/nix/gcroots/auto/<link>

# Dann Garbage Collection:
nix-collect-garbage",
        deep_dive: "\
WARUM PASSIERT DAS:
GC-Roots schützen Store-Pfade vor Garbage Collection.

TYPEN VON ROOTS:
1. User-Profile (~/.nix-profile)
2. System-Profil (/run/current-system)
3. Result-Symlinks (./result)
4. Auto-Roots

ROOTS AUFLISTEN:
  nix-store --gc --print-roots
  
ALTE GENERATIONEN ENTFERNEN:
  nix-collect-garbage -d",
        tip: Some("'nix-collect-garbage -d' entfernt alte Generationen"),
    });

    m.insert("disk-full", PatternTranslation {
        title: "Festplatte voll",
        explanation: "Nicht genug Speicherplatz für Build oder Store-Operationen.",
        solution: "\
# Speicher freigeben mit Garbage Collection:
nix-collect-garbage -d

# Nutzung prüfen:
du -sh /nix/store",
        deep_dive: "\
WARUM PASSIERT DAS:
/nix/store kann über Zeit groß werden:
- Mehrere Paketversionen
- Alte System-Generationen
- Build-Artefakte

SPEICHER FREIGEBEN:

1. SCHNELLE BEREINIGUNG:
   nix-collect-garbage

2. AGGRESSIV (entfernt alte Generationen):
   nix-collect-garbage -d
   # NixOS:
   sudo nix-collect-garbage -d

3. NUTZUNG PRÜFEN:
   df -h /nix
   du -sh /nix/store",
        tip: Some("'nix-collect-garbage -d' regelmäßig ausführen"),
    });

    // =========================================================================
    // PACKAGE RENAMED / REMOVED
    // =========================================================================
    m.insert("package-renamed", PatternTranslation {
        title: "Paket umbenannt: '$1' -> '$2'",
        explanation: "Das Paket '$1' wurde in '$2' umbenannt.",
        solution: "\
# Ersetze in deiner Konfiguration:
$1  ->  $2",
        deep_dive: "\
WARUM PASSIERT DAS:
Nixpkgs benennt Pakete manchmal um für:
- Konsistenz (python3Packages statt pythonPackages)
- Klarheit (libreoffice-still vs libreoffice)
- Upstream-Änderungen

WO ÄNDERN:
- configuration.nix
- home.nix
- flake.nix
- shell.nix

SUCHEN UND ERSETZEN:
  grep -r '$1' /etc/nixos/",
        tip: Some("Folge einfach der Umbenennung"),
    });

    m.insert("package-removed", PatternTranslation {
        title: "Paket entfernt: $1",
        explanation: "Das Paket '$1' wurde aus nixpkgs entfernt.",
        solution: "\
# Alternative suchen:
nix search nixpkgs <ähnlicher-name>

# Oder älteres nixpkgs nutzen:
inputs.nixpkgs-old.url = \"github:NixOS/nixpkgs/nixos-23.11\";",
        deep_dive: "\
WARUM PASSIERT DAS:
Pakete werden entfernt wegen:
- Unmaintained/abandoned upstream
- Sicherheitsprobleme
- Ersetzt durch bessere Alternative
- Lizenzprobleme

ALTERNATIVEN:

1. ÄHNLICHES PAKET FINDEN:
   nix search nixpkgs ...
   https://search.nixos.org

2. ÄLTERES NIXPKGS:
   Pinne auf Version wo es noch existierte.

3. SELBST PAKETIEREN:
   Wenn du es wirklich brauchst, erstelle eigene Derivation.",
        tip: Some("Prüfe ob es eine Alternative gibt"),
    });

    // =========================================================================
    // NETWORK ERRORS
    // =========================================================================
    m.insert("network-timeout", PatternTranslation {
        title: "Netzwerk-Timeout",
        explanation: "Verbindung zu '$1' ist fehlgeschlagen (Timeout).",
        solution: "\
# Erneut versuchen:
nix build --option connect-timeout 60

# Offline-Modus wenn möglich:
nix build --offline",
        deep_dive: "\
HÄUFIGE URSACHEN:
1. Langsame/instabile Internetverbindung
2. Server überlastet
3. Firewall blockiert
4. DNS-Probleme

LÖSUNGEN:

1. TIMEOUT ERHÖHEN:
   nix build --option connect-timeout 60

2. ALTERNATIVE CACHE:
   Nutze anderen Binary Cache oder baue lokal.

3. OFFLINE BAUEN:
   nix build --offline
   (Nur wenn alles im lokalen Store ist)

4. RETRY:
   Einfach nochmal versuchen.",
        tip: Some("Oft hilft einfach nochmal versuchen"),
    });

    m.insert("cannot-resolve-host", PatternTranslation {
        title: "Host konnte nicht aufgelöst werden",
        explanation: "DNS-Auflösung für Server fehlgeschlagen.",
        solution: "\
# DNS prüfen:
nslookup cache.nixos.org

# Temporär andere DNS nutzen:
echo 'nameserver 8.8.8.8' | sudo tee /etc/resolv.conf",
        deep_dive: "\
HÄUFIGE URSACHEN:
1. Kein Internet
2. DNS-Server nicht erreichbar
3. Firewall blockiert DNS (Port 53)
4. VPN-Probleme

DEBUGGING:
  ping cache.nixos.org
  nslookup cache.nixos.org
  dig cache.nixos.org

LÖSUNGEN:
1. Internetverbindung prüfen
2. DNS-Server in /etc/resolv.conf ändern
3. VPN deaktivieren/aktivieren
4. Router neustarten",
        tip: Some("Prüfe deine Internetverbindung"),
    });

    m.insert("ssl-certificate-error", PatternTranslation {
        title: "SSL-Zertifikatsfehler",
        explanation: "SSL/TLS-Zertifikat konnte nicht verifiziert werden.",
        solution: "\
# Systemzeit prüfen (häufige Ursache!):
date

# CA-Zertifikate aktualisieren:
sudo nixos-rebuild switch",
        deep_dive: "\
HÄUFIGE URSACHEN:
1. FALSCHE SYSTEMZEIT (sehr häufig!)
   Zertifikate haben Gültigkeitszeiträume.

2. FEHLENDE CA-ZERTIFIKATE
   Besonders in minimalen Systemen.

3. MITM/PROXY
   Firmen-Proxy mit eigenen Zertifikaten.

LÖSUNGEN:

1. ZEIT SYNCHRONISIEREN:
   sudo systemctl restart systemd-timesyncd
   # Oder:
   sudo ntpdate pool.ntp.org

2. CA-ZERTIFIKATE:
   security.pki.certificateFiles = [ ./corp-ca.crt ];

3. PROXY-ZERTIFIKATE:
   Füge Firmen-CA zu den vertrauenswürdigen hinzu.",
        tip: Some("Prüfe zuerst deine Systemzeit!"),
    });

    // =========================================================================
    // NIXOS-REBUILD SPECIFIC
    // =========================================================================
    m.insert("nixos-config-not-found", PatternTranslation {
        title: "NixOS Konfiguration '$1' nicht gefunden",
        explanation: "nixosConfigurations.$1 existiert nicht im Flake.",
        solution: "\
# Verfügbare Konfigurationen auflisten:
nix flake show

# Mit richtigem Hostname:
sudo nixos-rebuild switch --flake .#<hostname>",
        deep_dive: "\
WARUM PASSIERT DAS:
Der Hostname in --flake .#<n> muss mit einem Key in 
nixosConfigurations übereinstimmen.

PRÜFEN:
  hostname  # Aktueller Hostname
  nix flake show  # Verfügbare Configs

TYPISCHE STRUKTUR:
  nixosConfigurations = {
    mein-pc = nixpkgs.lib.nixosSystem { ... };
    laptop = nixpkgs.lib.nixosSystem { ... };
  };

HÄUFIGE FEHLER:
- Tippfehler im Hostname
- Hostname geändert ohne Flake zu updaten
- Groß-/Kleinschreibung",
        tip: Some("Hostname muss mit flake.nix übereinstimmen"),
    });

    m.insert("activation-script-failed", PatternTranslation {
        title: "Aktivierungsskript fehlgeschlagen",
        explanation: "Ein NixOS Aktivierungsskript ist fehlgeschlagen.",
        solution: "\
# Fehler im Journal prüfen:
journalctl -xe

# Manuell aktivieren mit Debug:
sudo /nix/var/nix/profiles/system/activate",
        deep_dive: "\
WARUM PASSIERT DAS:
Aktivierungsskripte laufen nach dem Build um das System zu 
konfigurieren. Fehler können auftreten bei:

1. BERECHTIGUNGSPROBLEME
2. DIENST KONNTE NICHT STARTEN
3. SYMLINK-KONFLIKT
4. FEHLENDE ABHÄNGIGKEIT ZUR RUNTIME

DEBUGGING:
  journalctl -xe
  systemctl status <service>
  
HÄUFIGE URSACHEN:
- Altes State kollidiert mit neuem
- Service braucht manuelle Migration
- Hardcodierte Pfade im Service",
        tip: Some("Prüfe journalctl -xe für Details"),
    });

    m.insert("boot-loader-failed", PatternTranslation {
        title: "Bootloader-Installation fehlgeschlagen",
        explanation: "Der Bootloader konnte nicht installiert werden.",
        solution: "\
# GRUB neu installieren:
sudo grub-install /dev/sda
sudo nixos-rebuild boot

# Oder EFI-Partition prüfen:
mount | grep boot",
        deep_dive: "\
HÄUFIGE URSACHEN:

1. EFI-PARTITION NICHT GEMOUNTED:
   mount /dev/sda1 /boot
   
2. VOLLE BOOT-PARTITION:
   df -h /boot
   # Alte Kernel entfernen

3. FALSCHE DEVICE:
   boot.loader.grub.device = \"/dev/sda\";
   # Nicht /dev/sda1!

4. UEFI vs BIOS MISMATCH:
   boot.loader.grub.efiSupport = true;
   boot.loader.efi.canTouchEfiVariables = true;

RECOVERY:
Boote von NixOS USB und:
  mount /dev/sda2 /mnt
  mount /dev/sda1 /mnt/boot
  nixos-enter
  nixos-rebuild boot",
        tip: Some("Bei EFI: Prüfe ob /boot gemounted ist"),
    });

    m.insert("systemd-service-failed", PatternTranslation {
        title: "Systemd-Dienst fehlgeschlagen: $1",
        explanation: "Der Dienst '$1' konnte nach dem Switch nicht starten.",
        solution: "\
# Status prüfen:
systemctl status $1
journalctl -u $1

# Neustarten:
sudo systemctl restart $1",
        deep_dive: "\
DEBUGGING:

1. STATUS UND LOGS:
   systemctl status <service>
   journalctl -u <service> -e

2. KONFIGURATION PRÜFEN:
   systemctl cat <service>

3. ABHÄNGIGKEITEN:
   systemctl list-dependencies <service>

HÄUFIGE URSACHEN:
- Port bereits belegt
- Fehlende Berechtigungen
- Konfigurationsfehler
- Fehlende Runtime-Deps

MANUELL TESTEN:
  sudo -u <user> /nix/store/.../bin/<program>",
        tip: Some("journalctl -u <service> zeigt die Logs"),
    });

    m.insert("switch-to-configuration-failed", PatternTranslation {
        title: "switch-to-configuration fehlgeschlagen",
        explanation: "Die neue Konfiguration konnte nicht aktiviert werden.",
        solution: "\
# Vorherige Generation nutzen:
sudo nixos-rebuild switch --rollback

# Oder bei Boot:
# Im GRUB-Menü ältere Generation wählen",
        deep_dive: "\
WARUM PASSIERT DAS:
Die Aktivierung der neuen Konfiguration ist fehlgeschlagen.
Das System läuft noch auf der alten Konfiguration.

ROLLBACK:
  sudo nixos-rebuild switch --rollback
  
  # Oder: Boot in alte Generation via GRUB/systemd-boot

DEBUGGING:
  journalctl -xe
  systemctl --failed

HÄUFIGE URSACHEN:
- Service-Startfehler
- Berechtigungsprobleme
- Ressourcenkonflikte (Ports, Files)
- Inkompatible State-Migration",
        tip: Some("Rollback ist sicher und schnell"),
    });

    m.insert("dependency-build-failed", PatternTranslation {
        title: "Abhängigkeit konnte nicht gebaut werden",
        explanation: "Eine Abhängigkeit für '$1' konnte nicht gebaut werden.",
        solution: "\
# Vollständiges Log prüfen:
nix log <failed-derivation>

# Oder mit mehr Output:
nix build -L",
        deep_dive: "\
WARUM PASSIERT DAS:
Ein Paket in deiner Abhängigkeitskette ist fehlgeschlagen.
Dein Paket selbst ist wahrscheinlich okay.

DEBUGGING:

1. WELCHE DEPENDENCY:
   Die Fehlermeldung nennt die fehlgeschlagene Derivation.

2. LOG ANSCHAUEN:
   nix log /nix/store/<hash>-<name>.drv

3. URSACHE FINDEN:
   Ist es ein nixpkgs Bug? Prüfe GitHub Issues.

LÖSUNGEN:
- nixpkgs updaten (Bug könnte gefixt sein)
- Zur letzten funktionierenden Version zurück
- Bug in nixpkgs melden",
        tip: Some("Prüfe nixpkgs GitHub Issues"),
    });

    // =========================================================================
    // LOCK / PERMISSION
    // =========================================================================
    m.insert("resource-locked", PatternTranslation {
        title: "Ressource gesperrt",
        explanation: "Eine Nix-Ressource ist von einem anderen Prozess gesperrt.",
        solution: "\
# Andere Nix-Prozesse prüfen:
ps aux | grep nix

# Lock-Datei manuell entfernen (Vorsicht!):
sudo rm /nix/var/nix/gc.lock",
        deep_dive: "\
WARUM PASSIERT DAS:
Nix nutzt Locks um gleichzeitige Zugriffe zu verhindern.

HÄUFIGE URSACHEN:
1. Anderer nix-build läuft noch
2. nix-collect-garbage läuft
3. Abgestürzter Nix-Prozess hinterließ Lock

LÖSUNGEN:

1. WARTEN:
   Wenn ein anderer Build läuft, warten.

2. PROZESS FINDEN:
   ps aux | grep nix
   # Kill wenn es ein Zombie ist

3. LOCK ENTFERNEN:
   sudo rm /nix/var/nix/gc.lock
   sudo rm /nix/var/nix/db/db.lock
   
   VORSICHT: Nur wenn sicher kein anderer Prozess läuft!",
        tip: Some("Warte bis andere Nix-Prozesse fertig sind"),
    });

    m.insert("not-authorized-daemon", PatternTranslation {
        title: "Nicht autorisiert für Nix-Daemon",
        explanation: "Du hast keine Berechtigung den Nix-Daemon zu nutzen.",
        solution: "\
# User zur nix-users Gruppe hinzufügen:
sudo usermod -aG nixbld $USER

# Neu einloggen oder:
newgrp nixbld",
        deep_dive: "\
WARUM PASSIERT DAS:
Multi-User Nix-Installationen erfordern Gruppenmitgliedschaft.

LÖSUNGEN:

1. GRUPPE HINZUFÜGEN:
   sudo usermod -aG nixbld $USER
   # Dann neu einloggen!

2. NIXOS:
   users.users.meinuser.extraGroups = [ \"wheel\" ];
   # wheel kann normalerweise sudo nix nutzen

3. TRUSTED USER:
   nix.settings.trusted-users = [ \"meinuser\" ];

PRÜFEN:
  groups  # Zeigt deine Gruppen
  ls -la /nix/var/nix/daemon-socket/",
        tip: Some("Neu einloggen nach Gruppenänderung"),
    });

    // =========================================================================
    // FLAKE SPECIFIC
    // =========================================================================
    m.insert("flake-not-found", PatternTranslation {
        title: "flake.nix nicht gefunden",
        explanation: "Im angegebenen Pfad/Repository wurde keine flake.nix gefunden.",
        solution: "\
# Prüfe ob flake.nix existiert:
ls -la flake.nix

# Bei Git-Repos - richtiger Branch?
git branch -a",
        deep_dive: "\
HÄUFIGE URSACHEN:

1. FALSCHES VERZEICHNIS:
   cd /pfad/zum/flake
   ls flake.nix

2. FALSCHER GIT-BRANCH:
   git checkout main
   
3. FLAKE.NIX NICHT COMMITTED:
   git add flake.nix
   git commit -m 'Add flake'

4. URL FALSCH:
   github:user/repo  # Nicht github:user/repo.git
   
5. PRIVATES REPO:
   git+ssh://git@github.com/user/private-repo",
        tip: Some("Ist flake.nix im Git committed?"),
    });

    m.insert("dirty-git-tree", PatternTranslation {
        title: "Git-Verzeichnis hat uncommittete Änderungen",
        explanation: "Das Git-Repository hat ungespeicherte Änderungen.",
        solution: "\
# Änderungen committen:
git add -A && git commit -m 'Update'

# Oder dirty erlauben (zum Testen):
nix build --impure",
        deep_dive: "\
WARUM PASSIERT DAS:
Flakes wollen einen sauberen Git-Zustand für Reproduzierbarkeit.
Uncommittete Dateien werden ignoriert!

WICHTIG:
Neue Dateien die nicht committed sind werden NICHT gesehen!

LÖSUNGEN:

1. COMMITTEN:
   git add -A
   git commit -m 'WIP'

2. DIRTY ERLAUBEN (nur zum Testen):
   nix build .#package --impure
   
3. LOKALER PFAD STATT GIT:
   nix build path:.#package",
        tip: Some("Uncommittete Dateien werden ignoriert!"),
    });

    m.insert("pure-eval-forbidden", PatternTranslation {
        title: "Absoluter Pfad in Pure-Eval verboten",
        explanation: "Zugriff auf absoluten Pfad ist im Pure-Eval-Modus nicht erlaubt.",
        solution: "\
# Relativen Pfad nutzen:
./config statt /home/user/config

# Oder in flake.nix:
src = ./.;",
        deep_dive: "\
WARUM PASSIERT DAS:
Flakes erzwingen 'pure evaluation' - keine Seiteneffekte, keine 
absoluten Pfade, volle Reproduzierbarkeit.

VERBOTEN:
  /home/user/file
  /etc/nixos/config.nix
  builtins.getEnv \"HOME\"

ERLAUBT:
  ./file              # Relativ zur flake.nix
  self                # Das Flake selbst
  inputs.nixpkgs      # Deklarierte Inputs

UMGEHEN (nicht empfohlen):
  nix build --impure
  
  # Oder in flake.nix:
  outputs = { self, ... }: {
    # Mit --impure kann man dann:
    # builtins.getEnv nutzen
  };",
        tip: Some("Nutze relative Pfade: ./foo statt /absolute/foo"),
    });

    // =========================================================================
    // COMMON TYPOS / MISTAKES
    // =========================================================================
    m.insert("not-in-nixpkgs", PatternTranslation {
        title: "Paket '$1' nicht in nixpkgs",
        explanation: "Das Paket '$1' konnte in nixpkgs nicht gefunden werden.",
        solution: "\
# Suchen mit richtigem Namen:
nix search nixpkgs $1

# Online suchen:
# https://search.nixos.org/packages",
        deep_dive: "\
MÖGLICHE URSACHEN:

1. TIPPFEHLER:
   pkgs.htoop  # -> pkgs.htop

2. ANDERER NAME:
   pkgs.openjdk  # Nicht pkgs.java
   pkgs.python3  # Nicht pkgs.python

3. IN UNTERGRUPPE:
   pkgs.python3Packages.numpy
   pkgs.nodePackages.typescript

4. EXISTIERT NICHT IN NIXPKGS:
   -> Selbst paketieren oder Alternative finden

SUCHEN:
  nix search nixpkgs <n>
  https://search.nixos.org/packages",
        tip: Some("Exakte Schreibweise auf search.nixos.org prüfen"),
    });

    m.insert("file-conflict-activation", PatternTranslation {
        title: "Datei-Konflikt bei Aktivierung",
        explanation: "Eine Datei existiert bereits und kann nicht ersetzt werden.",
        solution: "\
# Alte Datei backuppen:
sudo mv /konflikt/datei /konflikt/datei.bak

# Dann nochmal:
sudo nixos-rebuild switch",
        deep_dive: "\
WARUM PASSIERT DAS:
NixOS/Home-Manager wollen eine Datei verwalten die schon existiert.

HÄUFIG BEI:
- /etc/... Konfigurations-Dateien
- ~/.config/... User-Configs
- Symlinks die auf nichts zeigen

LÖSUNGEN:

1. BACKUP UND ENTFERNEN:
   sudo mv /etc/datei /etc/datei.bak
   
2. FORCE (Home-Manager):
   home.file.\"pfad\".force = true;

3. PRÜFEN WAS ES IST:
   ls -la /pfad/zur/datei
   file /pfad/zur/datei",
        tip: Some("Backup machen, dann Datei entfernen"),
    });

    m.insert("nar-hash-mismatch", PatternTranslation {
        title: "NAR Hash Mismatch",
        explanation: "Der heruntergeladene Inhalt hat einen unerwarteten Hash.",
        solution: "\
# Erneut versuchen (oft temporär):
nix build --rebuild

# Cache leeren:
nix-store --delete /nix/store/<hash>...",
        deep_dive: "\
WARUM PASSIERT DAS:
Das Binary-Cache lieferte Daten mit falschem Hash.

URSACHEN:
1. Korrupter Download
2. Cache-Server-Problem
3. Man-in-the-Middle (selten)
4. Disk-Korruption lokal

LÖSUNGEN:

1. NOCHMAL VERSUCHEN:
   nix build --rebuild

2. ANDEREN CACHE:
   nix build --option substituters ''
   # Baut lokal statt Download

3. PFAD LÖSCHEN UND NEU:
   nix-store --delete /nix/store/<hash>...
   nix build

4. STORE VERIFIZIEREN:
   nix-store --verify --check-contents",
        tip: Some("Meist hilft einfach nochmal versuchen"),
    });

    // =========================================================================
    // SUPER COMMON DAILY ERRORS
    // =========================================================================
    m.insert("need-root", PatternTranslation {
        title: "Root/sudo erforderlich",
        explanation: "Diese Operation braucht Root-Rechte.",
        solution: "\
# Mit sudo ausführen:
sudo nixos-rebuild switch
sudo nix-collect-garbage",
        deep_dive: "\
BRAUCHT ROOT:
- nixos-rebuild switch/boot/test
- nix-collect-garbage (systemweit)
- System-Profil Installation

BRAUCHT KEIN ROOT:
- nix build
- nix develop
- nix-shell
- User-Profil (nix-env als User)",
        tip: Some("Nutze sudo für System-Operationen"),
    });

    m.insert("git-not-found", PatternTranslation {
        title: "Git nicht gefunden",
        explanation: "Git wird benötigt aber ist nicht installiert.",
        solution: "\
# NixOS - zu configuration.nix:
environment.systemPackages = [ pkgs.git ];

# Oder temporär:
nix-shell -p git",
        deep_dive: "\
WARUM:
Flakes und viele Fetch-Operationen brauchen git.

LÖSUNGEN:
1. Permanent: git zu systemPackages
2. Temporär: nix-shell -p git",
        tip: Some("Git zu systemPackages hinzufügen"),
    });

    m.insert("channel-not-found", PatternTranslation {
        title: "Nixpkgs Channel nicht gefunden",
        explanation: "Kein nixpkgs Channel konfiguriert.",
        solution: "\
# Channel hinzufügen:
nix-channel --add https://nixos.org/channels/nixos-unstable nixpkgs
nix-channel --update

# Oder besser - Flakes nutzen",
        deep_dive: "\
FÜR TRADITIONELLES NIX:
  nix-channel --add https://nixos.org/channels/nixos-unstable nixpkgs
  nix-channel --update

FÜR FLAKES (EMPFOHLEN):
  inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";",
        tip: Some("Erwäge auf Flakes umzusteigen"),
    });

    m.insert("value-is-null", PatternTranslation {
        title: "Wert ist null",
        explanation: "Ein Wert ist null obwohl etwas anderes erwartet wurde.",
        solution: "\
# Default-Wert hinzufügen:
myValue = config.foo.bar or \"default\";

# Oder auf null prüfen:
if myValue != null then ... else ...",
        deep_dive: "\
LÖSUNGEN:
1. Default: config.foo.bar or \"default\"
2. Null-Check: if x != null then ...
3. lib.optionalAttrs nutzen",
        tip: Some("Nutze 'or' für Default-Werte"),
    });

    m.insert("attribute-already-defined", PatternTranslation {
        title: "Attribut '$1' bereits definiert",
        explanation: "Das Attribut '$1' ist mehrfach definiert.",
        solution: "\
# Zusammenführen statt überschreiben:
{ a = 1; } // { b = 2; }

# Oder lib.mkMerge:
lib.mkMerge [ config1 config2 ]",
        deep_dive: "\
LÖSUNGEN:
1. Verschiedene Namen verwenden
2. Sets mit // mergen
3. lib.mkMerge in Modulen
4. lib.recursiveUpdate für tiefes Merge",
        tip: Some("Nutze // oder lib.mkMerge"),
    });

    m.insert("out-of-memory", PatternTranslation {
        title: "Speicher voll (OOM)",
        explanation: "Der Build hat keinen Speicher mehr.",
        solution: "\
# Parallele Jobs limitieren:
nix build -j 1

# Oder Swap erhöhen:
sudo fallocate -l 8G /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile",
        deep_dive: "\
LÖSUNGEN:
1. Weniger parallel: nix build -j 1 --cores 2
2. Swap hinzufügen
3. Binary Cache nutzen
4. Andere Apps schließen",
        tip: Some("Versuche: nix build -j 1 --cores 2"),
    });

    m.insert("build-interrupted", PatternTranslation {
        title: "Build unterbrochen",
        explanation: "Build wurde unterbrochen (Ctrl+C).",
        solution: "\
# Einfach nochmal starten:
nix build

# Partielle Builds sind sicher",
        deep_dive: "\
DAS IST OK:
- Nix Builds sind atomar
- Partielle Builds korrumpieren nicht
- Einfach nochmal starten

Nix überspringt fertige Teile.",
        tip: Some("Einfach Befehl nochmal ausführen"),
    });

    m.insert("config-not-found", PatternTranslation {
        title: "configuration.nix nicht gefunden",
        explanation: "NixOS Konfigurationsdatei nicht gefunden.",
        solution: "\
# Pfad prüfen:
ls -la /etc/nixos/

# Erstellen wenn fehlend:
sudo nixos-generate-config

# Oder Pfad angeben:
sudo nixos-rebuild switch -I nixos-config=./configuration.nix",
        deep_dive: "\
URSACHEN:
1. Frische Installation ohne Config
2. Config verschoben/gelöscht
3. Bei Flakes: --flake vergessen

LÖSUNGEN:
1. sudo nixos-generate-config
2. Pfad mit -I angeben
3. --flake für Flake-Configs",
        tip: Some("--flake für Flake-basierte Configs"),
    });

    m.insert("flake-lock-not-committed", PatternTranslation {
        title: "flake.lock nicht committed",
        explanation: "Die flake.lock muss in Git getrackt werden.",
        solution: "\
# Hinzufügen und committen:
git add flake.lock
git commit -m 'Update flake.lock'",
        deep_dive: "\
WARUM LOCK COMMITTEN:
- Stellt sicher alle nutzen gleiche Versionen
- Reproduzierbare Builds
- CI/CD Konsistenz

WORKFLOW:
1. flake.nix ändern
2. nix flake update
3. git add flake.nix flake.lock
4. git commit",
        tip: Some("Immer flake.lock committen"),
    });

    m.insert("evaluation-timeout", PatternTranslation {
        title: "Evaluation Timeout/Overflow",
        explanation: "Nix-Evaluation zu lang oder Rekursionslimit erreicht.",
        solution: "\
# Meist unendliche Schleife - prüfe:
# - Rekursive Imports
# - Overlays die sich selbst referenzieren
# - Zirkuläre Modul-Imports",
        deep_dive: "\
URSACHEN:
1. Unendliche Rekursion (Overlay mit final statt prev)
2. Sehr große Evaluation
3. Stack Overflow

DEBUGGING:
  nix eval --show-trace
  
Suche nach sich wiederholenden Mustern.",
        tip: Some("Meist unendliche Schleife - Overlays prüfen"),
    });

    m.insert("binary-cache-miss", PatternTranslation {
        title: "Kein Binary-Cache Treffer",
        explanation: "Paket nicht im Cache - wird lokal gebaut.",
        solution: "\
# Informativ, kein Fehler
# Für mehr Cache: stabiles nixpkgs nutzen:
inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-24.05\";",
        deep_dive: "\
WARUM:
Hydra baut nur für stabile Branches.

LÖSUNGEN:
1. Stabilen Branch nutzen (nixos-24.05)
2. Warten (Hydra baut noch)
3. Lokalen Build akzeptieren
4. Cachix für Community-Caches",
        tip: Some("Stabiles nixpkgs für bessere Cache-Hits"),
    });

    m.insert("derivation-output-mismatch", PatternTranslation {
        title: "Derivation-Output stimmt nicht",
        explanation: "Build-Output entspricht nicht dem erwarteten Hash.",
        solution: "\
# Für fetchurl - Hash updaten:
hash = lib.fakeHash;  # Korrekten Hash aus Fehler nehmen",
        deep_dive: "\
URSACHEN:
1. Upstream hat Datei geändert
2. Falscher Hash im Paket
3. Mirror lieferte anderen Inhalt

LÖSUNG:
lib.fakeHash nutzen, bauen, korrekten Hash einsetzen.",
        tip: Some("lib.fakeHash für korrekten Hash nutzen"),
    });

    m.insert("read-only-store", PatternTranslation {
        title: "Nix Store ist schreibgeschützt",
        explanation: "Kann nicht in /nix/store schreiben.",
        solution: "\
# Mount prüfen:
mount | grep /nix

# Remounten wenn nötig:
sudo mount -o remount,rw /nix",
        deep_dive: "\
URSACHEN:
1. Dateisystem read-only gemounted
2. Disk-Fehler
3. Docker/Container-Einschränkungen
4. Nix Daemon läuft nicht",
        tip: Some("Prüfe: mount | grep nix"),
    });

    m.insert("generation-switch-failed", PatternTranslation {
        title: "Generation-Wechsel fehlgeschlagen",
        explanation: "Konnte nicht zur angegebenen Generation wechseln.",
        solution: "\
# Verfügbare Generationen:
sudo nix-env --list-generations -p /nix/var/nix/profiles/system

# Zu bestimmter wechseln:
sudo nix-env --switch-generation 42 -p /nix/var/nix/profiles/system",
        deep_dive: "\
URSACHEN:
1. Generation wurde garbage-collected
2. Profil korrupt
3. Falsche Generationsnummer

LÖSUNGEN:
1. Verfügbare Generationen auflisten
2. Zu existierender wechseln
3. Stattdessen neu bauen",
        tip: Some("Erst verfügbare Generationen prüfen"),
    });

    m.insert("module-import-failed", PatternTranslation {
        title: "Modul-Import fehlgeschlagen",
        explanation: "Konnte NixOS/Home-Manager Modul nicht importieren.",
        solution: "\
# Pfad prüfen:
ls -la ./module.nix

# Syntax prüfen:
nix-instantiate --parse ./module.nix",
        deep_dive: "\
URSACHEN:
1. Datei existiert nicht
2. Syntaxfehler im Modul
3. Falscher Pfad
4. Modul hat Evaluierungsfehler

DEBUGGING:
  nix eval --show-trace",
        tip: Some("Datei-Existenz und Syntax prüfen"),
    });

    m.insert("overlay-infinite-recursion", PatternTranslation {
        title: "Overlay verursacht unendliche Rekursion",
        explanation: "Overlay nutzt final statt prev für modifiziertes Paket.",
        solution: "\
# FALSCH:
(final: prev: {
  pkg = final.pkg.override { };  # FALSCH!
})

# RICHTIG:
(final: prev: {
  pkg = prev.pkg.override { };   # prev nutzen!
})",
        deep_dive: "\
REGEL:
- final = Ergebnis NACH allen Overlays
- prev = VOR diesem Overlay

Paket X modifizieren? -> prev.X nutzen
Anderes Paket Y nutzen? -> final.Y ist ok",
        tip: Some("prev für das modifizierte Paket nutzen"),
    });

    m.insert("nix-path-empty", PatternTranslation {
        title: "NIX_PATH nicht gesetzt",
        explanation: "<nixpkgs> Lookup schlug fehl weil NIX_PATH leer ist.",
        solution: "\
# NIX_PATH setzen:
export NIX_PATH=nixpkgs=channel:nixos-unstable

# Oder besser - Flakes:
nix build nixpkgs#hello",
        deep_dive: "\
LÖSUNGEN:
1. NIX_PATH exportieren
2. Channels nutzen
3. Flakes nutzen (empfohlen)
4. Expliziten Pfad mit -I angeben",
        tip: Some("Erwäge Flakes stattdessen"),
    });

    // =========================================================================
    // EXTREMELY COMMON BEGINNER/DAILY ERRORS
    // =========================================================================
    m.insert("nix-command-not-found", PatternTranslation {
        title: "Nix Befehl nicht gefunden",
        explanation: "Nix ist nicht installiert oder nicht im PATH.",
        solution: "\
# Nix installieren:
sh <(curl -L https://nixos.org/nix/install) --daemon

# Oder zum PATH hinzufügen:
source ~/.nix-profile/etc/profile.d/nix.sh",
        deep_dive: "\
URSACHEN:
1. Nix nicht installiert
2. Shell nicht konfiguriert (PATH fehlt)
3. Neues Terminal nach Installation

LÖSUNGEN:
1. Nix installieren
2. Profile sourcen
3. Shell neustarten",
        tip: Some("Neues Terminal? Führe aus: source ~/.nix-profile/etc/profile.d/nix.sh"),
    });

    m.insert("not-a-derivation", PatternTranslation {
        title: "Wert ist keine Derivation",
        explanation: "Erwartet wurde ein Paket/Derivation aber etwas anderes bekommen.",
        solution: "\
# Prüfe was du referenzierst:
nix repl
> :t pkgs.hello

# Häufiger Fix - vielleicht ist es eine Funktion:
pkgs.callPackage ./pkg.nix { }",
        deep_dive: "\
HÄUFIGE URSACHEN:

1. ES IST EINE FUNKTION:
   # FALSCH:
   environment.systemPackages = [ ./my-pkg.nix ];
   # RICHTIG:
   environment.systemPackages = [ (pkgs.callPackage ./my-pkg.nix {}) ];

2. ES IST EIN SET:
   # FALSCH:
   pkgs.python3Packages  # Das ist ein Set!
   # RICHTIG:
   pkgs.python3Packages.numpy",
        tip: Some("Nutze 'nix repl' zum Erkunden"),
    });

    m.insert("override-not-available", PatternTranslation {
        title: "Paket unterstützt .override nicht",
        explanation: "Dieses Paket hat kein .override oder .overrideAttrs.",
        solution: "\
# Nutze overrideAttrs stattdessen:
pkg.overrideAttrs (old: {
  patches = old.patches or [] ++ [ ./fix.patch ];
})",
        deep_dive: "\
OVERRIDE-METHODEN:

1. overrideAttrs (funktioniert fast immer):
   pkgs.hello.overrideAttrs (old: { version = \"2.0\"; })

2. override (nur bei callPackage):
   pkgs.hello.override { stdenv = pkgs.clangStdenv; }

WANN WAS:
- Abhängigkeiten ändern -> .override
- Build-Attribute ändern -> .overrideAttrs",
        tip: Some("overrideAttrs funktioniert fast überall"),
    });

    m.insert("git-not-a-repository", PatternTranslation {
        title: "Kein Git-Repository",
        explanation: "Flakes erfordern dass das Verzeichnis ein Git-Repo ist.",
        solution: "\
# Git initialisieren:
git init
git add flake.nix flake.lock
git commit -m 'Initial commit'",
        deep_dive: "\
WARUM:
Flakes tracken Dateien über Git. Ohne Git weiß Nix nicht 
welche Dateien zum Flake gehören.

WICHTIG:
- Neue Dateien müssen 'git add' werden!
- Uncommittete Änderungen können ignoriert werden
- .gitignore'd Dateien sind für Flakes unsichtbar",
        tip: Some("'git init' ausführen und Dateien committen"),
    });

    m.insert("git-ref-not-found", PatternTranslation {
        title: "Git-Referenz nicht gefunden",
        explanation: "Der angegebene Branch, Tag oder Commit existiert nicht.",
        solution: "\
# Verfügbare Refs prüfen:
git ls-remote <repo>

# Korrekten Ref in flake.nix nutzen:
inputs.foo.url = \"github:owner/repo/main\";  # Nicht master!",
        deep_dive: "\
HÄUFIGE FEHLER:
1. 'master' vs 'main' - viele Repos haben gewechselt!
2. Tippfehler im Branch-Namen
3. Tag existiert noch nicht
4. Privates Repo ohne Auth",
        tip: Some("'master' ist oft jetzt 'main'"),
    });

    m.insert("not-a-shell-derivation", PatternTranslation {
        title: "Keine Shell-Derivation",
        explanation: "'nix develop' auf Paket statt devShell ausgeführt.",
        solution: "\
# Für Pakete nix shell nutzen:
nix shell nixpkgs#hello

# Für Entwicklung devShell erstellen:
devShells.default = mkShell {
  packages = [ gcc cmake ];
};",
        deep_dive: "\
BEFEHLE:
- nix develop -> Braucht devShell (für Entwicklung)
- nix shell -> Paket in PATH (zum Ausführen)
- nix build -> Paket bauen

HÄUFIGER FEHLER:
  nix develop nixpkgs#hello  # FALSCH
  nix shell nixpkgs#hello    # RICHTIG",
        tip: Some("'nix shell' für Pakete, 'nix develop' für devShells"),
    });

    m.insert("sqlite-database-locked", PatternTranslation {
        title: "Nix-Datenbank gesperrt",
        explanation: "Ein anderer Nix-Prozess nutzt die Datenbank.",
        solution: "\
# Andere Nix-Prozesse finden:
ps aux | grep nix

# Warten bis fertig, oder killen wenn hängt:
sudo pkill -9 nix",
        deep_dive: "\
URSACHEN:
1. Anderer nix build läuft
2. nix-collect-garbage läuft
3. Abgestürzter Nix-Prozess
4. nix-daemon hängt

LÖSUNGEN:
1. Auf anderen Build warten
2. Hängenden Prozess killen
3. Daemon neustarten",
        tip: Some("Auf andere Nix-Prozesse warten"),
    });

    m.insert("mkderivation-missing-name", PatternTranslation {
        title: "mkDerivation braucht name/pname",
        explanation: "stdenv.mkDerivation braucht entweder 'name' oder 'pname' + 'version'.",
        solution: "\
# Option 1 - pname + version (empfohlen):
stdenv.mkDerivation {
  pname = \"my-package\";
  version = \"1.0.0\";
}

# Option 2 - name direkt:
stdenv.mkDerivation {
  name = \"my-package-1.0.0\";
}",
        deep_dive: "\
WARUM pname + version BESSER:
- Ermöglicht versionbasierte Overrides
- Saubere Trennung
- Nixpkgs-Konvention",
        tip: Some("pname + version nutzen, nicht nur name"),
    });

    m.insert("nvidia-driver-mismatch", PatternTranslation {
        title: "NVIDIA Treiber-Versionskonflikt",
        explanation: "Kernel-Modul-Version passt nicht zum Userspace-Treiber.",
        solution: "\
# Nach nixos-rebuild neustarten:
sudo nixos-rebuild switch
sudo reboot

# Oder Module neu laden:
sudo rmmod nvidia_uvm nvidia_drm nvidia_modeset nvidia
sudo modprobe nvidia",
        deep_dive: "\
WARUM:
Nach Update ist Kernel-Modul alt aber Userspace neu.
Die müssen exakt übereinstimmen.

LÖSUNG:
Einfach neustarten nach nixos-rebuild!

NIXOS CONFIG:
  hardware.nvidia.package = config.boot.kernelPackages.nvidiaPackages.stable;",
        tip: Some("Nach NVIDIA-Update neustarten"),
    });

    m.insert("overlays-wrong-format", PatternTranslation {
        title: "Overlays müssen Liste von Funktionen sein",
        explanation: "Overlays müssen eine Liste von (final: prev: {...}) Funktionen sein.",
        solution: "\
# FALSCH:
nixpkgs.overlays = (final: prev: { });

# RICHTIG:
nixpkgs.overlays = [
  (final: prev: { myPkg = ...; })
];",
        deep_dive: "\
KORREKTES FORMAT:
  nixpkgs.overlays = [
    (final: prev: { myPackage = prev.hello; })
    (import ./my-overlay.nix)
  ];

FALSCHE FORMATE:
- Nur Funktion (keine Liste)
- Set statt Funktion",
        tip: Some("Overlays = [ (final: prev: {...}) ]"),
    });

    m.insert("modules-wrong-format", PatternTranslation {
        title: "Module müssen eine Liste sein",
        explanation: "Die 'modules' oder 'imports' Option erwartet eine Liste.",
        solution: "\
# FALSCH:
modules = ./module.nix;

# RICHTIG:
modules = [ ./module.nix ];",
        deep_dive: "\
Module und imports müssen Listen sein, auch für einzelne Einträge.

RICHTIG:
  modules = [
    ./configuration.nix
    ./hardware-configuration.nix
  ];",
        tip: Some("Immer [ ] nutzen, auch für einzelnes Modul"),
    });

    m.insert("specialisation-not-found", PatternTranslation {
        title: "Spezialisierung nicht gefunden",
        explanation: "Die angegebene NixOS-Spezialisierung existiert nicht.",
        solution: "\
# Verfügbare Spezialisierungen auflisten:
ls /nix/var/nix/profiles/system/specialisation/

# In configuration.nix definieren:
specialisation.gaming.configuration = {
  services.xserver.enable = true;
};",
        deep_dive: "\
WAS SIND SPEZIALISIERUNGEN:
Alternative Systemkonfigurationen die Basis teilen.
Nützlich für: Gaming-Modus, Arbeit, Minimal.

WECHSELN:
- Beim Booten: Im Bootloader auswählen
- Zur Laufzeit: sudo .../specialisation/gaming/.../switch-to-configuration switch",
        tip: Some("Schreibweise prüfen und erst rebuilden"),
    });

    m.insert("fetchgit-requires-hash", PatternTranslation {
        title: "fetchGit braucht Hash im Pure-Modus",
        explanation: "Pure Evaluation erfordert Hashes für Fetches.",
        solution: "\
# Hash hinzufügen:
src = fetchGit {
  url = \"https://...\";
  rev = \"abc123\";
  hash = \"sha256-...\";
};

# Oder fetchFromGitHub nutzen:
src = fetchFromGitHub { ... };",
        deep_dive: "\
HASH BEKOMMEN:
  nix-prefetch-git https://github.com/owner/repo --rev abc123

Oder lib.fakeHash nutzen - Build zeigt korrekten Hash.",
        tip: Some("lib.fakeHash für korrekten Hash nutzen"),
    });

    m.insert("home-manager-version-mismatch", PatternTranslation {
        title: "Home-Manager/nixpkgs Versionskonflikt",
        explanation: "Home-Manager Version passt nicht zu nixpkgs Version.",
        solution: "\
# Passende Branches nutzen:
inputs = {
  nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";
  home-manager = {
    url = \"github:nix-community/home-manager\";
    inputs.nixpkgs.follows = \"nixpkgs\";  # Wichtig!
  };
};",
        deep_dive: "\
PASSENDE VERSIONEN:
  nixos-24.05   -> home-manager release-24.05
  nixos-unstable -> home-manager master

'follows' IST WICHTIG:
Ohne es nutzt home-manager eigenes nixpkgs -> Konflikte!",
        tip: Some("Immer 'inputs.nixpkgs.follows' nutzen"),
    });

    m.insert("boot-read-only-filesystem", PatternTranslation {
        title: "Kann nicht auf /boot schreiben",
        explanation: "/boot ist schreibgeschützt oder voll.",
        solution: "\
# Prüfe ob gemounted:
mount | grep boot

# Speicherplatz prüfen:
df -h /boot

# Alte Generationen entfernen:
sudo nix-collect-garbage -d
sudo nixos-rebuild boot",
        deep_dive: "\
URSACHEN:
1. /boot nicht gemounted
2. /boot ist voll (häufig bei kleiner EFI-Partition)
3. Read-only gemounted

BEI KLEINER EFI-PARTITION:
  boot.loader.systemd-boot.configurationLimit = 10;",
        tip: Some("Alte Generationen entfernen: nix-collect-garbage -d"),
    });

    m.insert("environment-variable-not-set", PatternTranslation {
        title: "Umgebungsvariable nicht gesetzt",
        explanation: "Eine benötigte Umgebungsvariable fehlt.",
        solution: "\
# In Shell setzen:
export MY_VAR=\"value\"

# In Nix Derivation:
MY_VAR = \"value\";

# Oder preBuild:
preBuild = ''export MY_VAR=value'';",
        deep_dive: "\
WARUM:
Nix Builds laufen in sauberen Umgebungen. Variablen aus 
deiner Shell sind nicht verfügbar ohne explizite Übergabe.

LÖSUNGEN:
1. In Derivation setzen
2. In shellHook setzen  
3. makeWrapper nutzen für Runtime",
        tip: Some("Nix Builds haben saubere Umgebungen"),
    });

    m.insert("lib-not-found-runtime", PatternTranslation {
        title: "Shared Library zur Laufzeit nicht gefunden",
        explanation: "Programm kann benötigte .so Bibliothek nicht finden.",
        solution: "\
# Mit Library-Pfad wrappen:
postInstall = ''
  wrapProgram $out/bin/app \\
    --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ libGL ]}
'';

# Oder autoPatchelfHook nutzen:
nativeBuildInputs = [ autoPatchelfHook ];
buildInputs = [ libGL ];",
        deep_dive: "\
WARUM:
Binary erwartet Libraries in /usr/lib, aber Nix hat sie in /nix/store.

LÖSUNGEN:
1. autoPatchelfHook (beste für Binaries)
2. makeWrapper mit LD_LIBRARY_PATH
3. patchelf direkt

HÄUFIGE BIBLIOTHEKEN:
- libGL.so -> libGL, libglvnd
- libvulkan.so -> vulkan-loader
- libstdc++.so -> stdenv.cc.cc.lib",
        tip: Some("autoPatchelfHook für fertige Binaries nutzen"),
    });

    m.insert("flake-private-repo", PatternTranslation {
        title: "Kann nicht auf privates Repository zugreifen",
        explanation: "Git-Authentifizierung für privates Repo fehlgeschlagen.",
        solution: "\
# SSH URL nutzen:
inputs.private.url = \"git+ssh://git@github.com/owner/repo\";

# SSH-Key laden:
ssh-add ~/.ssh/id_ed25519",
        deep_dive: "\
WARUM:
HTTPS URLs können nicht authentifizieren. SSH für private Repos nutzen.

LÖSUNGEN:
1. SSH URL nutzen: git+ssh://git@github.com/owner/repo
2. SSH-Key laden: ssh-add
3. Testen: ssh -T git@github.com",
        tip: Some("git+ssh:// für private Repos nutzen"),
    });

    // =========================================================================
    // ADDITIONAL COMMON ERRORS
    // =========================================================================
    m.insert("cannot-unpack-archive", PatternTranslation {
        title: "Kann Archiv nicht entpacken",
        explanation: "Entpacken des heruntergeladenen Archivs fehlgeschlagen.",
        solution: "\
# Prüfe ob Archiv korrupt:
nix-prefetch-url --unpack <url>

# Oder Entpack-Methode angeben:
src = fetchzip { ... };  # Für zip
src = fetchurl { ... };  # Für tar.gz",
        deep_dive: "\
URSACHEN:
1. Korrupter Download
2. Falsches Archiv-Format
3. URL zeigt nicht auf Archiv

LÖSUNGEN:
1. Neu herunterladen
2. Richtigen Fetcher nutzen (fetchzip vs fetchurl)
3. URL mit curl prüfen",
        tip: Some("fetchzip für .zip Dateien nutzen"),
    });

    m.insert("file-not-found-store", PatternTranslation {
        title: "Datei nicht im Nix Store gefunden: $1",
        explanation: "Die referenzierte Datei existiert nicht im Nix Store.",
        solution: "\
# Pfad neu bauen:
nix-store --realise /nix/store/<pfad>

# Oder Derivation neu bauen:
nix build --rebuild",
        deep_dive: "\
URSACHEN:
1. Pfad wurde garbage-collected
2. Build wurde unterbrochen
3. Store-Korruption

LÖSUNGEN:
1. nix build --rebuild
2. nix-store --realise <pfad>
3. nix-store --verify",
        tip: Some("Versuche: nix build --rebuild"),
    });

    m.insert("file-not-found-stat", PatternTranslation {
        title: "Datei nicht gefunden: $1",
        explanation: "Die Datei oder das Verzeichnis existiert nicht.",
        solution: "\
# Pfad prüfen:
ls -la <pfad>

# Für Nix-Dateien relativen Pfad nutzen:
./myfile.nix  # Nicht /absoluter/pfad",
        deep_dive: "\
URSACHEN:
1. Tippfehler im Pfad
2. Datei gelöscht/verschoben
3. Absoluter Pfad in Flake (verboten)
4. Nicht in Git committet

LÖSUNGEN:
1. Pfad prüfen
2. Relative Pfade nutzen
3. git add für Flakes",
        tip: Some("Relative Pfade in Flakes nutzen"),
    });

    m.insert("unrecognised-cli-option", PatternTranslation {
        title: "Unbekannte Kommandozeilen-Option",
        explanation: "Die Kommandozeilen-Option existiert nicht.",
        solution: "\
# Verfügbare Optionen prüfen:
nix build --help

# Alte vs neue CLI:
# Alt: nix-build -A hello
# Neu: nix build .#hello",
        deep_dive: "\
URSACHEN:
1. Tippfehler
2. Alte vs neue CLI-Syntax
3. Option in neuer Version entfernt

ALT vs NEU:
  nix-build -A package  # Alt
  nix build .#package   # Neu

NEUE CLI AKTIVIEREN:
  experimental-features = nix-command flakes",
        tip: Some("Prüfe: nix <befehl> --help"),
    });

    m.insert("option-wrong-type", PatternTranslation {
        title: "Options-Wert hat falschen Typ",
        explanation: "Der NixOS-Options-Wert entspricht nicht dem erwarteten Typ.",
        solution: "\
# Erwarteten Typ prüfen:
nixos-option <option>

# Häufige Fixes:
enable = true;           # bool, nicht \"true\"
port = 8080;             # int, nicht \"8080\"
packages = [ pkg ];      # Liste, nicht einzeln",
        deep_dive: "\
HÄUFIGE TYP-FEHLER:

1. STRING statt BOOL:
   enable = \"true\";  # FALSCH
   enable = true;     # RICHTIG

2. STRING statt INT:
   port = \"80\";  # FALSCH
   port = 80;     # RICHTIG

3. EINZELN statt LISTE:
   packages = pkg;    # FALSCH
   packages = [pkg];  # RICHTIG",
        tip: Some("Optionstyp auf search.nixos.org prüfen"),
    });

    m.insert("while-evaluating", PatternTranslation {
        title: "Fehler beim Evaluieren von '$1'",
        explanation: "Ein Fehler trat beim Evaluieren von '$1' auf. Prüfe den vollständigen Trace.",
        solution: "\
# Vollständigen Stack-Trace anzeigen:
nix build --show-trace

# Der eigentliche Fehler steht unter dieser Zeile",
        deep_dive: "\
DIESE MELDUNG VERSTEHEN:
Diese Zeile sagt WO der Fehler passierte, nicht WAS der Fehler ist.
Die eigentliche Fehlermeldung kommt danach.

DEBUGGING:
1. VOLLE Fehlerausgabe lesen
2. --show-trace für kompletten Stack
3. Von unten nach oben lesen",
        tip: Some("--show-trace nutzen, von unten lesen"),
    });

    m.insert("derivation-call-error", PatternTranslation {
        title: "Fehler beim Derivation-Aufruf",
        explanation: "Beim Erstellen der Derivation ging etwas schief.",
        solution: "\
# Erforderliche Attribute prüfen:
# mkDerivation braucht: name (oder pname+version), src

stdenv.mkDerivation {
  pname = \"mypackage\";
  version = \"1.0\";
  src = ./. ;
}",
        deep_dive: "\
ERFORDERLICH FÜR mkDerivation:
- name ODER (pname + version)
- src ODER custom phases

HÄUFIGE FEHLER:
1. name fehlt
2. src ist String statt Pfad
3. buildInputs ist keine Liste",
        tip: Some("Prüfe: name/pname, src, buildInputs Typen"),
    });

    m
});

/// Replaces $1, $2, etc. with captured values.
fn substitute_captures(template: &str, captures: &[String]) -> String {
    let mut result = template.to_string();
    for (i, cap) in captures.iter().enumerate() {
        result = result.replace(&format!("${}", i + 1), cap);
    }
    result
}

/// Translates a MatchResult to German if translation is available.
pub fn translate_to_german(result: &MatchResult) -> MatchResult {
    if let Some(trans) = TRANSLATIONS_DE.get(result.pattern_id.as_str()) {
        let mut translated = result.clone();
        
        // Substitute captures into German templates
        translated.title = substitute_captures(trans.title, &result.captures);
        translated.explanation = substitute_captures(trans.explanation, &result.captures);
        translated.solution = substitute_captures(trans.solution, &result.captures);
        translated.deep_dive = substitute_captures(trans.deep_dive, &result.captures);
        translated.tip = trans.tip.map(|t| substitute_captures(t, &result.captures));
        
        // Special handling for linker errors
        if result.pattern_id == "linker-missing-lib" {
            if let Some(lib_name) = result.captures.first() {
                if let Some(pkg_name) = library_to_package(lib_name) {
                    translated.solution = translated.solution
                        .replace(&format!("[ {} ]", lib_name), &format!("[ {} ]", pkg_name));
                }
            }
        }
        
        translated
    } else {
        // No translation available, return original
        result.clone()
    }
}

/// Translates a MatchResult based on language code.
pub fn translate(result: &MatchResult, lang: &str) -> MatchResult {
    match lang {
        "de" => translate_to_german(result),
        _ => result.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::errors::patterns::Category;

    fn make_test_result() -> MatchResult {
        MatchResult {
            pattern_id: "linker-missing-lib".to_string(),
            category: Category::Build,
            title: "Linker cannot find library: ssl".to_string(),
            explanation: "The linker needs the 'ssl' library.".to_string(),
            solution: "buildInputs = [ ssl ];".to_string(),
            deep_dive: "Why this happens...".to_string(),
            tip: Some("Common: ssl->openssl".to_string()),
            captures: vec!["ssl".to_string()],
        }
    }

    #[test]
    fn test_translate_to_german() {
        let result = make_test_result();
        let translated = translate_to_german(&result);
        
        assert!(translated.title.contains("ssl"));
        assert!(translated.title.contains("Linker"));
        assert!(translated.explanation.contains("Bibliothek"));
    }

    #[test]
    fn test_translate_preserves_captures() {
        let result = make_test_result();
        let translated = translate(&result, "de");
        
        // $1 should be replaced with "ssl"
        assert!(translated.title.contains("ssl"));
        assert!(!translated.title.contains("$1"));
    }

    #[test]
    fn test_translate_english_unchanged() {
        let result = make_test_result();
        let translated = translate(&result, "en");
        
        assert_eq!(result.title, translated.title);
    }
}
