//! Pattern definitions for Nix error messages.
//!
//! Each pattern contains a regex to match errors and provides
//! human-readable explanations with solutions and deep-dive learning.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Mutex;

/// Error category for grouping and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Build,
    Eval,
    Flake,
    Fetch,
    NixOS,
}

impl Category {
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Build => "🔨",
            Self::Eval => "📜",
            Self::Flake => "❄️",
            Self::Fetch => "📥",
            Self::NixOS => "🐧",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Build => "BUILD",
            Self::Eval => "EVAL",
            Self::Flake => "FLAKE",
            Self::Fetch => "FETCH",
            Self::NixOS => "NIXOS",
        }
    }
}

/// A single error pattern with its explanation and solution.
pub struct Pattern {
    pub id: &'static str,
    pub category: Category,
    pub regex_str: &'static str,
    pub title: &'static str,
    pub explanation: &'static str,
    pub solution: &'static str,
    pub deep_dive: &'static str,
    pub tip: Option<&'static str>,
}

/// Cache for compiled regexes.
static REGEX_CACHE: Lazy<Mutex<HashMap<&'static str, Regex>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

impl Pattern {
    /// Returns the compiled regex, caching it for future use.
    pub fn regex(&self) -> Regex {
        let mut cache = REGEX_CACHE.lock().expect("regex cache lock poisoned");
        if let Some(re) = cache.get(self.regex_str) {
            return re.clone();
        }
        let re = Regex::new(self.regex_str).expect("Invalid regex pattern");
        cache.insert(self.regex_str, re.clone());
        re
    }
}

/// Maps common library names to their Nix package names.
pub fn library_to_package(lib: &str) -> Option<&'static str> {
    match lib {
        "ssl" | "crypto" => Some("openssl"),
        "z" => Some("zlib"),
        "ffi" => Some("libffi"),
        "curl" => Some("curl"),
        "xml2" => Some("libxml2"),
        "png" => Some("libpng"),
        "jpeg" | "jpg" => Some("libjpeg"),
        "sqlite3" => Some("sqlite"),
        "ncurses" | "ncursesw" | "tinfo" => Some("ncurses"),
        "readline" => Some("readline"),
        "bz2" => Some("bzip2"),
        "lzma" => Some("xz"),
        "uuid" => Some("libuuid"),
        "dbus-1" => Some("dbus"),
        "X11" => Some("xorg.libX11"),
        "GL" | "EGL" => Some("libGL"),
        "stdc++" => Some("stdenv.cc.cc.lib"),
        _ => None,
    }
}

// =============================================================================
// PATTERN DEFINITIONS
// =============================================================================

pub static PATTERNS: &[Pattern] = &[
    // =========================================================================
    // BUILD ERRORS - Linking & Compilation
    // =========================================================================
    Pattern {
        id: "linker-missing-lib",
        category: Category::Build,
        regex_str: r"(?:ld|collect2).*cannot find -l(\w+)",
        title: "Linker cannot find library: $1",
        explanation: "The linker needs the '$1' library but it's not available.",
        solution: "\
buildInputs = [ $1 ];
nativeBuildInputs = [ pkg-config ];",
        deep_dive: "\
WHY THIS HAPPENS:
Nix builds happen in isolated environments (sandboxes). Unlike traditional Linux 
where libraries in /usr/lib are globally available, Nix requires you to explicitly 
declare every dependency. This is a feature, not a bug - it ensures reproducibility.

THE BUILD PROCESS:
1. Compiler creates .o object files from your source
2. Linker (ld) combines objects + libraries into executable
3. Linker searches for -l<name> in paths from buildInputs
4. If not found -> this error

buildInputs vs nativeBuildInputs:
- buildInputs = libraries for the TARGET system (runtime deps)
- nativeBuildInputs = tools for the BUILD system (compilers, pkg-config)

pkg-config helps the compiler find library paths and flags automatically.

FINDING THE RIGHT PACKAGE:
Library names don't always match package names:
  -lssl        -> openssl
  -lz          -> zlib  
  -lcrypto     -> openssl
  -lpthread    -> (included in glibc, usually automatic)

Use: nix search nixpkgs <library-name>",
        tip: Some("Common: ssl->openssl, z->zlib, ffi->libffi"),
    },

    Pattern {
        id: "missing-header",
        category: Category::Build,
        regex_str: r"fatal error: (\S+\.h).*[Nn]o such file",
        title: "Missing header: $1",
        explanation: "The compiler can't find the header file '$1'.",
        solution: "\
# Find the package:
nix-locate -w '*/$1'

# Add it:
buildInputs = [ <package> ];",
        deep_dive: "\
WHY THIS HAPPENS:
Header files (.h) contain declarations that tell the compiler what functions 
and types exist in a library. They're needed at COMPILE time, not runtime.

In Nix, header files live in /nix/store/<hash>-<pkg>/include/. The compiler 
only searches paths from packages in buildInputs.

DEVELOPMENT vs RUNTIME PACKAGES:
Some distros split packages into 'foo' and 'foo-dev'. In Nix, the main package 
usually includes headers. But some packages have separate '.dev' outputs:

  buildInputs = [ openssl.dev ];  # Just headers
  buildInputs = [ openssl ];      # Usually works too

FINDING THE RIGHT PACKAGE:
1. Use nix-locate (from nix-index):
   nix-locate -w '*/openssl/ssl.h'
   
2. Search nixpkgs:
   nix search nixpkgs openssl

3. Check the Nix store:
   ls /nix/store/*openssl*/include/

COMMON HEADER -> PACKAGE MAPPINGS:
  openssl/*.h  -> openssl
  curl/*.h     -> curl
  zlib.h       -> zlib
  python*.h    -> python3",
        tip: Some("Install nix-index for nix-locate"),
    },

    Pattern {
        id: "undefined-reference",
        category: Category::Build,
        regex_str: r#"undefined reference to [`'"]([^'`"]+)[`'"]"#,
        title: "Undefined reference: $1",
        explanation: "The linker found a declaration but no implementation for '$1'.",
        solution: "buildInputs = [ <library-with-$1> ];",
        deep_dive: "\
WHY THIS HAPPENS:
This is a LINKER error, not a compiler error. The difference:
- Compiler error: 'unknown function foo()' -> missing header
- Linker error: 'undefined reference to foo' -> missing library

The code compiled successfully (headers were found), but when linking, the 
actual implementation of '$1' couldn't be found in any provided library.

COMMON CAUSES:
1. Missing library in buildInputs
2. Wrong library version (API changed)
3. Library exists but wasn't linked (missing -l flag)
4. C++ name mangling issues (extern \"C\" missing)

DEBUGGING STEPS:
1. Find which library provides the symbol:
   nm -D /nix/store/*-somelib*/lib/*.so | grep '$1'

2. Check if library is being linked:
   Look for -l<libname> in the build output

3. For C++ code called from C:
   Ensure the header uses: extern \"C\" { ... }

ORDER MATTERS:
Linker processes libraries left-to-right. If libA needs libB:
  WRONG:  buildInputs = [ libB libA ];
  RIGHT:  buildInputs = [ libA libB ];",
        tip: None,
    },

    Pattern {
        id: "builder-failed",
        category: Category::Build,
        regex_str: r#"builder for [`'"]([^'`"]+)[`'"] failed|build of [`'"]([^'`"]+)[`'"] failed"#,
        title: "Build failed",
        explanation: "The derivation failed to build. The actual error is above.",
        solution: "\
# View full log:
nix log $1

# Build with verbose output:
nix build -L",
        deep_dive: "\
UNDERSTANDING THIS ERROR:
This message appears at the END of a failed build. It's not the actual error - 
it's just telling you which derivation failed. The real error is ABOVE this line.

HOW NIX BUILDS WORK:
1. Nix evaluates your expression -> creates a derivation (.drv)
2. The derivation specifies: inputs, build script, outputs
3. Nix runs the builder in a sandbox
4. If builder exits non-zero -> this error

FINDING THE REAL ERROR:
1. Scroll up in terminal output
2. Use: nix log /nix/store/<hash>-<name>.drv
3. Build with -L flag: nix build -L (streams logs)
4. Keep failed build: nix build --keep-failed

THE BUILD SANDBOX:
Builds run isolated with:
- No network access (usually)
- No access to /home, /usr, etc.
- Only declared inputs available
- Temp directory as working dir

COMMON ROOT CAUSES:
- Missing dependency (buildInputs)
- Hardcoded paths (/usr/bin/...)
- Network access attempt
- Missing build tool (nativeBuildInputs)
- Incompatible compiler flags",
        tip: Some("The actual error is usually ABOVE this line"),
    },

    // =========================================================================
    // EVAL ERRORS - Nix Language
    // =========================================================================
    Pattern {
        id: "attribute-missing",
        category: Category::Eval,
        regex_str: r#"attribute [`'"](\w+)[`'"].*missing"#,
        title: "Attribute '$1' not found",
        explanation: "The attribute '$1' doesn't exist in this attribute set.",
        solution: "\
# Explore in nix repl:
nix repl -f '<nixpkgs>'
nix-repl> pkgs.<TAB>

# Search: https://search.nixos.org/packages",
        deep_dive: "\
WHY THIS HAPPENS:
Nix attribute sets are like dictionaries/maps. When you write pkgs.foo, 
you're accessing the 'foo' attribute. If it doesn't exist -> this error.

COMMON CAUSES:

1. TYPOS:
   pkgs.python3Pkgs     # WRONG
   pkgs.python3Packages # RIGHT
   
2. WRONG PATH:
   pkgs.nodePackages.typescript  # Might be correct
   pkgs.typescript               # Or this, depends on package

3. PACKAGE RENAMED/REMOVED:
   Packages change between nixpkgs versions. Check:
   https://search.nixos.org/packages

4. MISSING INPUT:
   In flakes, you must pass inputs:
   outputs = { nixpkgs, ... }:  # 'nixpkgs' must be in inputs

EXPLORING ATTRIBUTE SETS:
Use nix repl to explore:
  $ nix repl -f '<nixpkgs>'
  nix-repl> pkgs.python<TAB>  # Shows all python* attrs
  nix-repl> pkgs.python3Packages.n<TAB>  # Shows numpy, etc.
  nix-repl> builtins.attrNames pkgs.python3Packages

CHECKING IF ATTRIBUTE EXISTS:
  pkgs.foo or null              # Returns null if missing
  pkgs ? foo                    # Returns true/false
  lib.attrByPath [\"foo\"] null pkgs  # Safe nested access",
        tip: Some("python3Packages not pythonPackages"),
    },

    Pattern {
        id: "infinite-recursion",
        category: Category::Eval,
        regex_str: r"infinite recursion encountered",
        title: "Infinite recursion",
        explanation: "Nix detected a circular dependency in your expressions.",
        solution: "\
# In overlays - use 'prev' not 'final':
(final: prev: {
  pkg = prev.pkg.override { };  # ✓ prev
})

# In modules - don't reference config in options:
options.x = mkOption { default = 42; };  # ✓",
        deep_dive: "\
WHY THIS HAPPENS:
Nix is lazily evaluated, but it still detects cycles. When A needs B and B needs A, 
Nix throws this error to prevent hanging forever.

COMMON CAUSES:

1. OVERLAYS - Using 'final' instead of 'prev':
   # WRONG - causes infinite recursion:
   (final: prev: {
     myPkg = final.myPkg.override { ... };  # final.myPkg calls this overlay!
   })
   
   # RIGHT - use prev for the original:
   (final: prev: {
     myPkg = prev.myPkg.override { ... };   # prev.myPkg is the un-overlaid version
   })

2. NIXOS MODULES - config in option defaults:
   # WRONG:
   options.services.foo.port = mkOption {
     default = config.services.bar.port;  # config isn't ready yet!
   };
   
   # RIGHT - use mkDefault in config section:
   config.services.foo.port = mkDefault config.services.bar.port;

3. SELF-REFERENCING SETS:
   # WRONG:
   let x = { a = x.b; b = x.a; }; in x
   
   # RIGHT - use rec or let:
   let x = rec { a = 1; b = a; }; in x

DEBUGGING:
Use --show-trace to see the evaluation stack:
  nix build --show-trace

The trace shows the chain of calls leading to the recursion.",
        tip: Some("Use --show-trace to find the source"),
    },

    Pattern {
        id: "undefined-variable",
        category: Category::Eval,
        regex_str: r#"undefined variable [`'"](\w+)[`'"]"#,
        title: "Undefined variable: $1",
        explanation: "'$1' is not defined in this scope.",
        solution: "\
# Add to function arguments:
{ pkgs, lib, $1, ... }:

# Or import it:
let $1 = import ./file.nix; in ...",
        deep_dive: "\
WHY THIS HAPPENS:
Nix has lexical scoping - variables must be defined before use. Unlike global 
variables in other languages, Nix functions must explicitly declare their inputs.

COMMON CAUSES:

1. MISSING FUNCTION ARGUMENT:
   # WRONG - pkgs not in arguments:
   { }: pkgs.hello
   
   # RIGHT:
   { pkgs }: pkgs.hello

2. MISSING 'inherit' OR 'with':
   # WRONG:
   { inherit hello; }  # 'hello' must exist in scope
   
   # RIGHT:
   { inherit (pkgs) hello; }  # Get 'hello' from pkgs

3. WRONG SCOPE:
   # WRONG - 'bar' not in scope:
   let foo = 1; in bar
   
   # RIGHT:
   let foo = 1; bar = 2; in bar

4. FLAKE OUTPUTS:
   # WRONG:
   outputs = { self }: { packages = nixpkgs.legacyPackages; }
   
   # RIGHT - nixpkgs must be in inputs:
   outputs = { self, nixpkgs }: { ... }

SPECIAL VARIABLES:
- pkgs - usually passed as argument or via import <nixpkgs> {}
- lib - from pkgs.lib or nixpkgs.lib  
- config - NixOS module argument
- builtins - always available globally",
        tip: Some("pkgs, lib, config must be in function args"),
    },

    Pattern {
        id: "type-error",
        category: Category::Eval,
        regex_str: r"expected a (\w+) but found a (\w+)|value is a (\w+) while a (\w+) was expected",
        title: "Type error: expected $1, got $2",
        explanation: "Nix expected a '$1' but received a '$2' instead.",
        solution: r#"# Common fixes:
packages = [ pkgs.git ];  # list, not single
enable = true;            # bool, not "true"
port = 8080;              # int, not "8080""#,
        deep_dive: "\
WHY THIS HAPPENS:
Nix is dynamically typed but still enforces types at runtime. When a function 
expects a list but gets a string, you see this error.

COMMON TYPE MISMATCHES:

1. LIST vs SINGLE VALUE:
   # WRONG:
   environment.systemPackages = pkgs.git;
   
   # RIGHT - must be a list:
   environment.systemPackages = [ pkgs.git ];

2. BOOL vs STRING:
   # WRONG:
   services.nginx.enable = \"true\";
   
   # RIGHT:
   services.nginx.enable = true;

3. INT vs STRING:
   # WRONG:
   services.nginx.port = \"8080\";
   
   # RIGHT:
   services.nginx.port = 8080;

4. SET vs OTHER:
   # WRONG - mkOption expects set:
   options.foo = true;
   
   # RIGHT:
   options.foo = mkOption { type = types.bool; default = true; };

5. PATH vs STRING:
   # These are different types:
   ./foo.nix        # path (starts with . or /)
   \"./foo.nix\"      # string

TYPE CHECKING IN NIX:
  builtins.typeOf x       # Returns type as string
  builtins.isList x       # Check specific type
  builtins.isString x
  builtins.isInt x
  builtins.isAttrs x",
        tip: None,
    },

    Pattern {
        id: "cannot-coerce",
        category: Category::Eval,
        regex_str: r"cannot coerce .+ to a string",
        title: "Cannot convert to string",
        explanation: "Nix can't automatically convert this value to a string.",
        solution: r#"# For derivations:
"${pkgs.hello}/bin/hello"

# For sets - access the attribute:
mySet.name  # not mySet"#,
        deep_dive: "\
WHY THIS HAPPENS:
String interpolation (${...}) tries to convert values to strings. Nix can 
convert some types automatically (paths, derivations) but not arbitrary sets.

WHAT CAN BE INTERPOLATED:
✓ Strings:     \"${\"hello\"}\"      -> \"hello\"
✓ Paths:       \"${./foo}\"         -> \"/nix/store/...-foo\"
✓ Derivations: \"${pkgs.hello}\"    -> \"/nix/store/...-hello\"
✓ Numbers:     \"${toString 42}\"   -> \"42\"

✗ Sets:        \"${mySet}\"         -> ERROR
✗ Lists:       \"${myList}\"        -> ERROR
✗ Functions:   \"${myFunc}\"        -> ERROR

SOLUTIONS:

1. ACCESS SPECIFIC ATTRIBUTE:
   # WRONG:
   \"${pkgs.python3}\"  # This is a set with many attrs
   
   # RIGHT - access what you need:
   \"${pkgs.python3}/bin/python\"
   \"${pkgs.python3.name}\"

2. USE toString:
   \"value is ${toString 42}\"
   \"list: ${toString [1 2 3]}\"  # Works for simple lists

3. USE builtins.toJSON:
   \"json: ${builtins.toJSON mySet}\"

4. USE lib.concatStringsSep:
   lib.concatStringsSep \", \" [\"a\" \"b\" \"c\"]  # \"a, b, c\"",
        tip: None,
    },

    Pattern {
        id: "syntax-error",
        category: Category::Eval,
        regex_str: r"syntax error.*unexpected (\S+)",
        title: "Syntax error near: $1",
        explanation: "Nix parser encountered unexpected input.",
        solution: "\
{ a = 1; b = 2; }  # semicolons after attrs
[ a b c ]          # no commas in lists  
{ a, b }: ...      # commas in function args",
        deep_dive: r#"NIX SYNTAX GOTCHAS:

1. SEMICOLONS - after each attribute:
   # WRONG:
   { a = 1, b = 2 }
   { a = 1 b = 2 }
   
   # RIGHT:
   { a = 1; b = 2; }

2. NO COMMAS IN LISTS:
   # WRONG:
   [ "a", "b", "c" ]
   
   # RIGHT:
   [ "a" "b" "c" ]

3. COMMAS IN FUNCTION ARGS:
   # RIGHT:
   { a, b, c }: a + b + c
   
4. STRING ESCAPING:
   # Interpolation uses ${}:
   "hello ${name}"
   
   # Escape with double $:
   "literal $${not interpolated}"
   
   # Or use two single quotes for multi-line:
   ''
     no escaping needed here
     ${but this interpolates}
   ''

5. PATH vs STRING:
   ./foo       # path - relative to current file
   "./foo"     # string - NOT a path
   /foo        # absolute path (rare in Nix)

6. ATTRSET ACCESS:
   foo.bar     # access attr
   foo."bar"   # also valid (for weird names)
   foo.${x}    # dynamic attr access

ERROR LOCATION MAY BE WRONG:
The parser reports where it got confused, which may be AFTER the actual mistake.
Check lines BEFORE the reported location too."#,
        tip: Some("Error location may not be exact"),
    },

    // =========================================================================
    // FLAKE ERRORS
    // =========================================================================
    Pattern {
        id: "flake-no-output",
        category: Category::Flake,
        regex_str: r#"flake [`'"]([^'`"]+)[`'"] does not provide.*[`'"]([^'`"]+)[`'"]"#,
        title: "Flake has no output '$2'",
        explanation: "The flake '$1' doesn't have the requested output.",
        solution: "\
# List available outputs:
nix flake show $1

# Common paths:
packages.x86_64-linux.default
devShells.x86_64-linux.default",
        deep_dive: "\
WHY THIS HAPPENS:
Flakes have a structured output schema. You must use the exact path to the 
output you want, including the system architecture.

FLAKE OUTPUT STRUCTURE:
outputs = {
  packages.<system>.<name>     # nix build .#<name>
  packages.<system>.default    # nix build (no fragment)
  
  devShells.<system>.<name>    # nix develop .#<name>
  devShells.<system>.default   # nix develop (no fragment)
  
  apps.<system>.<name>         # nix run .#<name>
  
  nixosConfigurations.<name>   # nixos-rebuild --flake .#<name>
  homeConfigurations.<name>    # home-manager --flake .#<name>
}

SYSTEM MATTERS:
The <system> is usually:
  x86_64-linux   (most Linux)
  aarch64-linux  (ARM Linux, Raspberry Pi)
  x86_64-darwin  (Intel Mac)
  aarch64-darwin (Apple Silicon Mac)

COMMON MISTAKES:
  nix build .#hello          # Need: packages.x86_64-linux.hello
  nix build .#packages.hello # Need: packages.x86_64-linux.hello

LISTING OUTPUTS:
  nix flake show              # Shows all outputs
  nix flake show github:owner/repo

USING DEFAULT OUTPUTS:
If 'default' is defined:
  nix build                   # Uses packages.<system>.default
  nix develop                 # Uses devShells.<system>.default",
        tip: Some("Don't forget the system: x86_64-linux"),
    },

    Pattern {
        id: "flake-input-missing",
        category: Category::Flake,
        regex_str: r#"input [`'"](\w+)[`'"] not found"#,
        title: "Input '$1' not declared",
        explanation: "The input '$1' is used but not declared in flake.nix inputs.",
        solution: r#"# Add to flake.nix:
inputs.$1.url = "github:owner/repo";
outputs = { $1, ... }: { };"#,
        deep_dive: "\
WHY THIS HAPPENS:
Flake inputs must be declared in the 'inputs' section AND passed to 'outputs'.
Unlike non-flake Nix, you can't just import arbitrary paths or fetch URLs inline.

FLAKE STRUCTURE:
{
  inputs = {
    # 1. Declare inputs here:
    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";
    home-manager.url = \"github:nix-community/home-manager\";
  };

  outputs = { self, nixpkgs, home-manager, ... }:
    # 2. Use them here ↑ (must match input names)
    { };
}

INPUT URL FORMATS:
  github:owner/repo           # GitHub repo (default branch)
  github:owner/repo/branch    # Specific branch
  github:owner/repo?ref=v1.0  # Tag or ref
  
  git+https://example.com/repo.git
  git+ssh://git@github.com/owner/repo.git
  
  path:/absolute/path         # Local path
  path:./relative/path        # Relative path

FOLLOWING OTHER INPUTS:
Make inputs use the same nixpkgs:
{
  inputs.nixpkgs.url = \"...\";
  inputs.home-manager.url = \"...\";
  inputs.home-manager.inputs.nixpkgs.follows = \"nixpkgs\";
}

UPDATING INPUTS:
  nix flake update            # Update all inputs
  nix flake lock --update-input nixpkgs  # Update one",
        tip: None,
    },

    // =========================================================================
    // FETCH ERRORS
    // =========================================================================
    Pattern {
        id: "hash-mismatch",
        category: Category::Fetch,
        regex_str: r"(?s)hash mismatch.*got:\s*(\S+)",
        title: "Hash mismatch",
        explanation: "Downloaded content doesn't match the expected hash.",
        solution: r#"# Use the correct hash:
hash = "$1";"#,
        deep_dive: "\
WHY THIS HAPPENS:
Nix requires fixed-output derivations (downloads) to declare their hash upfront.
This ensures reproducibility - you always get the exact same bytes.

WHEN HASH CHANGES:
1. Upstream file changed (new release, re-uploaded)
2. You updated the URL but not the hash
3. Mirror returned different content
4. Archive was regenerated (timestamps changed)

GETTING THE RIGHT HASH:

Method 1 - Use nix-prefetch:
  nix-prefetch-url <url>
  nix-prefetch-url --unpack <url>  # For archives

Method 2 - Use lib.fakeHash temporarily:
  src = fetchurl {
    url = \"...\";
    hash = lib.fakeHash;  # Build will fail with correct hash
  };

Method 3 - Use nix hash:
  nix hash to-sri sha256:<hex-hash>

HASH FORMATS:
  Old: sha256 = \"0abc123...\";           # 64 hex chars
  New: hash = \"sha256-ABC123...\";       # SRI format (preferred)
  
  Convert: nix hash to-sri sha256:0abc...

FOR GITHUB:
  fetchFromGitHub {
    owner = \"user\";
    repo = \"repo\";
    rev = \"v1.0.0\";
    hash = \"sha256-...\";
  }
  
GITHUB ARCHIVES ARE UNSTABLE:
GitHub-generated archives can change. Use:
  nix-prefetch-url --unpack https://github.com/.../archive/v1.0.0.tar.gz",
        tip: Some("Use lib.fakeHash during development"),
    },

    Pattern {
        id: "download-failed",
        category: Category::Fetch,
        regex_str: r#"unable to download [`'"]([^'`"]+)[`'"]"#,
        title: "Download failed",
        explanation: "Could not download from '$1'.",
        solution: r#"# Check if URL works:
curl -I "$1"

# Try a newer version or mirror"#,
        deep_dive: "\
WHY THIS HAPPENS:
The URL is unreachable. Could be network, server, or URL issue.

COMMON CAUSES:

1. URL NO LONGER EXISTS:
   Software moves, domains expire, files get renamed.
   -> Find a new URL or use a different version

2. NETWORK/FIREWALL:
   Your network might block the download.
   -> Try: curl -I <url>

3. RATE LIMITING:
   GitHub and others limit anonymous requests.
   -> Wait, or use authenticated requests

4. SANDBOX RESTRICTIONS:
   Nix sandbox blocks network during build.
   -> Fetches must be fixed-output derivations

USING MIRRORS:
For source archives, check:
  - GitHub releases
  - Package homepage
  - Source mirrors (gnu.org, kernel.org, etc.)

VENDORING:
For unreliable sources, vendor the file:
  src = ./vendor/package-1.0.tar.gz;

GITHUB RELEASES vs ARCHIVES:
Releases (tagged assets) are stable:
  https://github.com/owner/repo/releases/download/v1.0/file.tar.gz

Archive downloads can change:
  https://github.com/owner/repo/archive/v1.0.tar.gz",
        tip: None,
    },

    // =========================================================================
    // NIXOS ERRORS
    // =========================================================================
    Pattern {
        id: "option-not-exist",
        category: Category::NixOS,
        regex_str: r#"option [`'"]([^'`"]+)[`'"] does not exist"#,
        title: "Option '$1' doesn't exist",
        explanation: "This NixOS option doesn't exist. Typo or missing module?",
        solution: "\
# Search: https://search.nixos.org/options

# If from a module, import it:
imports = [ module.nixosModules.default ];",
        deep_dive: "\
WHY THIS HAPPENS:
NixOS options must be declared by a module before they can be used. The option 
you're trying to set either doesn't exist or comes from an unimported module.

COMMON CAUSES:

1. TYPO:
   services.nginx.enabel = true;  # Should be 'enable'

2. WRONG PATH:
   services.nginx.settings.port    # Might be services.nginx.port
   
3. MODULE NOT IMPORTED:
   Flake modules need explicit import:
   imports = [ inputs.foo.nixosModules.default ];

4. REMOVED/RENAMED:
   Options change between NixOS versions.
   Check release notes!

FINDING OPTIONS:
1. Search online:
   https://search.nixos.org/options
   
2. Query locally:
   nixos-option services.nginx
   
3. Read module source:
   View in nixpkgs/nixos/modules/services/web-servers/nginx/

OPTION STRUCTURE:
Options have a specific path format:
  services.<name>.*
  programs.<name>.*
  networking.*
  hardware.*
  boot.*
  
Each level must exist. You can't invent new paths.

CUSTOM OPTIONS:
Define your own in a module:
  options.my.option = lib.mkOption {
    type = lib.types.bool;
    default = false;
  };",
        tip: Some("Options are case-sensitive"),
    },

    Pattern {
        id: "assertion-failed",
        category: Category::NixOS,
        regex_str: r"Failed assertions:|assertion.*failed|failed.*assertion",
        title: "Assertion failed",
        explanation: "A NixOS module check failed. Read the message carefully.",
        solution: "\
# Common fixes:
hardware.enableRedistributableFirmware = true;
users.groups.mygroup = {};",
        deep_dive: "\
WHY THIS HAPPENS:
NixOS modules use assertions to enforce requirements. When you enable a service 
that needs something else, an assertion tells you what's missing.

ASSERTIONS ARE HELPFUL:
Unlike cryptic errors, assertions explain exactly what's wrong:
  'The user 'nginx' must exist'
  'services.postgresql must be enabled'
  'hardware.cpu.intel.updateMicrocode requires unfree'

COMMON ASSERTION FIXES:

1. MISSING USER/GROUP:
   users.users.myuser = { isSystemUser = true; group = \"mygroup\"; };
   users.groups.mygroup = {};

2. REQUIRES ANOTHER SERVICE:
   services.postgresql.enable = true;

3. UNFREE PACKAGES:
   nixpkgs.config.allowUnfree = true;
   # Or per-package:
   nixpkgs.config.allowUnfreePredicate = pkg: builtins.elem (lib.getName pkg) [
     \"nvidia-x11\"
   ];

4. FIRMWARE:
   hardware.enableRedistributableFirmware = true;

5. DEPRECATED OPTIONS:
   Follow the message - it usually tells you the new option name.

READING THE MESSAGE:
The assertion message appears after 'Failed assertions:'. 
Read ALL of it - it often contains the solution.",
        tip: Some("Assertions are helpful - read the message carefully"),
    },

    Pattern {
        id: "collision",
        category: Category::NixOS,
        regex_str: r#"collision between.*[`'"]([^'`"]+)[`'"].*[`'"]([^'`"]+)[`'"]"#,
        title: "File collision",
        explanation: "Packages '$1' and '$2' both provide the same file.",
        solution: "\
# Set priority (lower number = higher priority):
environment.systemPackages = [
  (lib.hiPrio pkgs.package1)  # This one wins
  pkgs.package2
];",
        deep_dive: "\
WHY THIS HAPPENS:
Two packages in your environment try to install the same file path. Nix can't 
decide which one to use, so it fails.

COMMON COLLISION EXAMPLES:
- Different vim packages (vim vs neovim vs vim-full)
- Shell tools (coreutils vs busybox)
- Alternative implementations (openssl vs libressl)

SOLUTIONS:

1. SET PRIORITY (one package wins):
   environment.systemPackages = [
     (lib.hiPrio pkgs.coreutils)  # Use this /bin/ls
     pkgs.busybox                  # This /bin/ls ignored
   ];

2. USE lowPrio (one package loses):
   environment.systemPackages = [
     (lib.lowPrio pkgs.busybox)   # This /bin/ls ignored
     pkgs.coreutils               # Use this /bin/ls
   ];

3. REMOVE ONE PACKAGE:
   If you don't need both, remove one.

4. USE DIFFERENT OUTPUTS:
   Some packages have multiple outputs:
   pkgs.vim          # Full vim with runtime
   pkgs.vim.xxd      # Just xxd tool

5. CREATE WRAPPER:
   Override one package to install to different path:
   (pkgs.vim.overrideAttrs (old: {
     postInstall = (old.postInstall or \"\") + ''
       mv $out/bin/vim $out/bin/vim-original
     '';
   }))

PRIORITY VALUES:
lib.hiPrio  adds meta.priority = -10
lib.lowPrio adds meta.priority = 10
Default is usually 5. Lower number = wins collision.",
        tip: None,
    },

    // =========================================================================
    // PYTHON BUILD ERRORS
    // =========================================================================
    Pattern {
        id: "python-module-not-found",
        category: Category::Build,
        regex_str: r#"ModuleNotFoundError: No module named ['"]([\w.]+)['"]"#,
        title: "Python module not found: $1",
        explanation: "The Python module '$1' is not installed.",
        solution: "\
# In shell.nix or flake.nix:
python3.withPackages (ps: [ ps.$1 ])

# Or in buildPythonPackage:
propagatedBuildInputs = [ python3Packages.$1 ];",
        deep_dive: "\
WHY THIS HAPPENS:
In Nix, Python packages must be explicitly listed. There's no global site-packages 
directory like in traditional Python setups with pip.

PYTHON ENVIRONMENTS IN NIX:

1. DEVELOPMENT SHELL:
   mkShell {
     packages = [
       (python3.withPackages (ps: with ps; [
         numpy
         pandas
         requests
       ]))
     ];
   }

2. BUILDING A PYTHON PACKAGE:
   buildPythonPackage {
     propagatedBuildInputs = [
       python3Packages.numpy
     ];
   }

3. SYSTEM-WIDE (NixOS):
   environment.systemPackages = [
     (python3.withPackages (ps: [ ps.numpy ]))
   ];

FINDING PYTHON PACKAGES:
- Search: nix search nixpkgs python3Packages.<name>
- Browse: https://search.nixos.org/packages?query=python3Packages

PACKAGE NAME MAPPING:
PyPI name -> Nix name may differ:
  Pillow        -> pillow
  scikit-learn  -> scikitlearn
  PyYAML        -> pyyaml
  
Use nix search or check:
  nix eval nixpkgs#python3Packages.<name>

COMMON ISSUES:
- Package exists in PyPI but not nixpkgs -> need to package it
- Package name has different capitalization
- Package is in python3Packages not python2Packages",
        tip: Some("Search: nix search nixpkgs python3Packages"),
    },

    Pattern {
        id: "python-import-error",
        category: Category::Build,
        regex_str: r#"ImportError: cannot import name ['"](\w+)['"] from ['"]([\w.]+)['"]"#,
        title: "Python import error: $1 from $2",
        explanation: "Cannot import '$1' from '$2'. Version mismatch or changed API?",
        solution: "\
# Check package version:
nix eval nixpkgs#python3Packages.$2.version

# Try pinning a specific version or check docs",
        deep_dive: "\
WHY THIS HAPPENS:
The symbol '$1' doesn't exist in module '$2'. This usually means:

1. VERSION MISMATCH:
   Your code expects a different version than nixpkgs provides.
   
   Check what's available:
     nix eval nixpkgs#python3Packages.$2.version
   
   Pin nixpkgs to get a specific version, or override:
     python3Packages.$2.overridePythonAttrs (old: {
       version = \"x.y.z\";
       src = fetchPypi { ... };
     })

2. API CHANGED:
   The library's API changed between versions.
   Check their changelog/docs for migration guide.

3. WRONG MODULE PATH:
   Import path may have changed:
     Old: from foo.bar import baz
     New: from foo import baz

4. OPTIONAL DEPENDENCY:
   Some features need optional deps:
     python3Packages.$2.override {
       withSomeFeature = true;
     }

CHECKING AVAILABLE SYMBOLS:
In Python:
  import $2
  dir($2)
  
In Nix:
  nix repl
  > p = (import <nixpkgs> {}).python3Packages.$2
  > p.meta",
        tip: None,
    },

    // =========================================================================
    // RUST BUILD ERRORS  
    // =========================================================================
    Pattern {
        id: "rust-crate-not-found",
        category: Category::Build,
        regex_str: r"error\[E0463\]: can't find crate for `(\w+)`",
        title: "Rust crate not found: $1",
        explanation: "The Rust crate '$1' cannot be found.",
        solution: "\
# Add to Cargo.toml:
[dependencies]
$1 = \"*\"

# For system libs, add to buildInputs:
nativeBuildInputs = [ pkg-config ];
buildInputs = [ openssl ];",
        deep_dive: r#"WHY THIS HAPPENS:
Cargo dependencies are handled separately from Nix. But crates that link to 
C libraries (openssl, sqlite, etc.) need those libraries in buildInputs.

TYPES OF RUST DEPENDENCIES:

1. PURE RUST CRATES:
   Just add to Cargo.toml - Cargo/Nix handles it.

2. CRATES WITH C DEPENDENCIES:
   Need both Cargo.toml AND Nix buildInputs:
   
   buildRustPackage {
     nativeBuildInputs = [ pkg-config ];
     buildInputs = [ openssl sqlite ];
   }

3. CRATES THAT BUILD C CODE:
   Need C compiler:
   nativeBuildInputs = [ pkg-config cmake ];

COMMON CRATE -> NIX MAPPINGS:
  openssl-sys    -> openssl, pkg-config
  rusqlite       -> sqlite
  curl-sys       -> curl
  zlib-sys       -> zlib
  
ENVIRONMENT VARIABLES:
Some crates need hints:
  OPENSSL_DIR = "${openssl.dev}";
  SQLITE3_LIB_DIR = "${sqlite.out}/lib";

USING buildRustPackage:
  rustPlatform.buildRustPackage {
    cargoLock.lockFile = ./Cargo.lock;
    # or:
    cargoHash = "sha256-...";
  }

DEBUGGING:
Build with verbose cargo:
  CARGO_LOG=debug nix build -L"#,
        tip: Some("Check if crate needs a C library"),
    },

    Pattern {
        id: "rust-linker-native",
        category: Category::Build,
        regex_str: r"could not find native static library `(\w+)`",
        title: "Rust missing native lib: $1",
        explanation: "Rust can't find the native library '$1'.",
        solution: "\
buildInputs = [ $1 ];
nativeBuildInputs = [ pkg-config ];",
        deep_dive: "\
WHY THIS HAPPENS:
A Rust crate is trying to link against a C library that isn't available 
in the Nix build environment.

SOLUTION:
Add the library to buildInputs AND add pkg-config to find it:

  rustPlatform.buildRustPackage {
    nativeBuildInputs = [ pkg-config ];
    buildInputs = [ openssl ];
  }

LIBRARY NAME MAPPING:
The error shows the library name used in code, which may differ from package:
  ssl       -> openssl
  crypto    -> openssl
  z         -> zlib
  sqlite3   -> sqlite

FOR VENDORED BUILDS:
Some crates can vendor (bundle) C code:
  CARGO_FEATURE_VENDORED=1

OR use the 'vendored' feature:
  -sys crate with 'vendored' feature in Cargo.toml",
        tip: None,
    },

    // =========================================================================
    // NODE.JS BUILD ERRORS
    // =========================================================================
    Pattern {
        id: "node-module-not-found",
        category: Category::Build,
        regex_str: r#"Error: Cannot find module ['"]([\w./@-]+)['"]"#,
        title: "Node module not found: $1",
        explanation: "The Node.js module '$1' is not installed.",
        solution: "\
# In shell.nix:
nodePackages.$1

# Or use npmInstallHook:
buildInputs = [ nodejs ];
npmDeps = fetchNpmDeps { ... };",
        deep_dive: "\
WHY THIS HAPPENS:
Node.js modules are not globally available in Nix. You need to either:
1. Use nodePackages from nixpkgs (limited selection)
2. Build your own with buildNpmPackage or mkYarnPackage

OPTIONS FOR NODE IN NIX:

1. NIXPKGS nodePackages (easiest, limited):
   environment.systemPackages = [ nodePackages.typescript ];
   
2. buildNpmPackage (for your projects):
   buildNpmPackage {
     src = ./.;
     npmDepsHash = \"sha256-...\";
   }

3. mkYarnPackage (for Yarn projects):
   mkYarnPackage {
     src = ./.;
   }

4. node2nix (generates expressions):
   node2nix -i package.json -o node-packages.nix
   
5. dream2nix (modern alternative):
   Handles npm, yarn, pnpm automatically.

FOR DEVELOPMENT SHELLS:
  mkShell {
    buildInputs = [ nodejs nodePackages.npm ];
    shellHook = ''
      npm install  # Install deps locally
      export PATH=\"$PWD/node_modules/.bin:$PATH\"
    '';
  }

NATIVE MODULES (node-gyp):
Need Python and build tools:
  nativeBuildInputs = [ python3 pkg-config ];
  buildInputs = [ nodejs ];",
        tip: Some("Use node2nix or dream2nix for complex projects"),
    },

    // =========================================================================
    // PERMISSION / PATH ERRORS
    // =========================================================================
    Pattern {
        id: "permission-denied-nix-store",
        category: Category::Build,
        regex_str: r#"[Pp]ermission denied.*(/nix/store/[^\s'"]+)"#,
        title: "Permission denied in Nix store",
        explanation: "Attempted to write to the read-only Nix store.",
        solution: "\
# Use $out for outputs:
mkdir -p $out/bin
cp mybin $out/bin/

# For temp files use $TMPDIR:
export HOME=$TMPDIR",
        deep_dive: "\
WHY THIS HAPPENS:
The Nix store (/nix/store) is READ-ONLY. This is fundamental to Nix's design. 
It ensures that builds are reproducible and packages are immutable.

COMMON CAUSES:

1. HARDCODED PATHS:
   Code tries to write to /nix/store/... directly.
   -> Patch the code or use substituteInPlace

2. CACHE DIRECTORIES:
   Program tries to create ~/.cache or similar.
   -> Set: export HOME=$TMPDIR

3. RUNTIME STATE:
   Program expects to modify its install directory.
   -> Use wrapProgram to redirect to writable location

SOLUTIONS:

1. OUTPUT TO $out:
   installPhase = ''
     mkdir -p $out/bin
     cp myprogram $out/bin/
   '';

2. USE TEMP DIRECTORY:
   export HOME=$TMPDIR
   export XDG_CACHE_HOME=$TMPDIR/.cache
   export XDG_DATA_HOME=$TMPDIR/.local/share

3. WRAP PROGRAM:
   postInstall = ''
     wrapProgram $out/bin/myprogram --set HOME /tmp
   '';

4. PATCH SOURCE:
   postPatch = ''
     substituteInPlace src/config.h --replace /usr/share $out/share
   '';

NIX BUILD ENVIRONMENT:
- /nix/store/* = read-only inputs
- $out = your output (writable during build)
- $TMPDIR = temp space (writable)
- /build = working directory (writable)",
        tip: Some("Never write to /nix/store directly"),
    },

    Pattern {
        id: "path-not-in-store",
        category: Category::Eval,
        regex_str: r#"path ['"]([\w./-]+)['"] is not in the Nix store"#,
        title: "Path not in Nix store: $1",
        explanation: "The path '$1' must be copied to the Nix store first.",
        solution: "\
# Use ./path for local files:
src = ./my-source;

# Or fetch from URL:
src = fetchurl { url = \"...\"; hash = \"...\"; };",
        deep_dive: "\
WHY THIS HAPPENS:
Nix can only use paths that are either:
1. Already in /nix/store
2. Local paths (./foo) that get copied to store automatically
3. Fetched via fetchurl, fetchFromGitHub, etc.

INVALID PATHS:
  /home/user/project     # Absolute path outside store
  /tmp/foo               # Temp directory
  (a string like ./foo)  # String, not a path!

VALID PATHS:
  ./foo                  # Path literal (starts with ./ or /)
  ./src/main.rs          # Relative path
  /absolute/path         # Absolute (but avoid these)
  
  fetchFromGitHub { }    # Fetched and hashed
  builtins.fetchurl { }  # Fetched
  
PATH vs STRING:
  ./foo     # PATH - will be copied to /nix/store
  \"./foo\"   # STRING - just text, not a path!

COMMON PATTERNS:

1. LOCAL SOURCE:
   src = ./.;  # Current directory (filtered)
   
   # With filter:
   src = lib.cleanSource ./.;

2. SUBDIRECTORY:
   src = ./src;

3. SINGLE FILE:
   configFile = ./config.toml;

4. FROM FLAKE INPUT:
   src = inputs.my-repo;

IN FLAKES:
Paths are relative to flake.nix location.
Use self for the flake's own source:
  src = self;
  src = self + /subdir;",
        tip: None,
    },

    // =========================================================================
    // NIX DAEMON / STORE ERRORS
    // =========================================================================
    Pattern {
        id: "cannot-connect-daemon",
        category: Category::Build,
        regex_str: r"cannot connect to daemon|Is the daemon running|connection refused.*nix-daemon",
        title: "Cannot connect to Nix daemon",
        explanation: "The Nix daemon is not running or not accessible.",
        solution: "\
# On NixOS:
sudo systemctl start nix-daemon

# On other Linux:
sudo nix-daemon &

# Check status:
systemctl status nix-daemon",
        deep_dive: "\
WHY THIS HAPPENS:
Nix uses a daemon for multi-user installations. The daemon manages the 
/nix/store and handles builds. Without it, Nix commands fail.

COMMON CAUSES:
1. Daemon not started (after reboot)
2. Socket permissions issue
3. Daemon crashed

SINGLE-USER vs MULTI-USER:
- Single-user: No daemon needed, but only one user can use Nix
- Multi-user: Daemon required, multiple users share the store

FIXING ON DIFFERENT SYSTEMS:

NixOS:
  # Usually automatic, but:
  sudo systemctl restart nix-daemon

Other Linux (systemd):
  sudo systemctl enable nix-daemon
  sudo systemctl start nix-daemon

macOS:
  sudo launchctl start org.nixos.nix-daemon

Docker/WSL:
  May need manual daemon start or single-user mode.",
        tip: Some("On NixOS this should auto-start"),
    },

    Pattern {
        id: "experimental-features",
        category: Category::Eval,
        regex_str: r#"experimental Nix feature ['`"]([^'`"]+)['`"] is disabled"#,
        title: "Experimental feature disabled: $1",
        explanation: "The feature '$1' requires explicit opt-in.",
        solution: "\
# Temporary (one command):
nix --experimental-features '$1' <command>

# Permanent (~/.config/nix/nix.conf):
experimental-features = nix-command flakes",
        deep_dive: "\
WHY THIS HAPPENS:
Nix has 'experimental' features that aren't enabled by default. The most 
common ones are 'nix-command' (new CLI) and 'flakes'.

COMMON FEATURES:
- nix-command: New 'nix build', 'nix develop', etc.
- flakes: Flake support (flake.nix, inputs, etc.)
- repl-flake: Use flakes in nix repl

ENABLING PERMANENTLY:

Option 1 - User config (~/.config/nix/nix.conf):
  experimental-features = nix-command flakes

Option 2 - System config (/etc/nix/nix.conf):
  experimental-features = nix-command flakes

Option 3 - NixOS configuration.nix:
  nix.settings.experimental-features = [ \"nix-command\" \"flakes\" ];

CHECKING CURRENT SETTINGS:
  nix show-config | grep experimental",
        tip: Some("Most people enable 'nix-command flakes' permanently"),
    },

    Pattern {
        id: "store-path-not-valid",
        category: Category::Build,
        regex_str: r#"store path ['`"]([^'`"]+)['`"] is not valid|path.*is not valid"#,
        title: "Invalid store path",
        explanation: "A referenced store path doesn't exist or is corrupted.",
        solution: "\
# Verify and repair store:
nix-store --verify --check-contents --repair

# Or rebuild the path:
nix-store --realise <path>",
        deep_dive: "\
WHY THIS HAPPENS:
Store paths can become invalid due to:
1. Garbage collection removed needed paths
2. Interrupted builds
3. Manual deletion from /nix/store
4. Disk corruption

FIXING:

1. REPAIR THE STORE:
   nix-store --verify --check-contents --repair
   
2. REBUILD MISSING PATHS:
   nix-build '<nixpkgs>' -A <package> --repair
   
3. DELETE AND REBUILD:
   nix-store --delete <path>
   nix-build ...

4. FOR SYSTEM PATHS (NixOS):
   sudo nixos-rebuild switch --repair

PREVENTING:
- Don't manually edit /nix/store
- Keep roots to prevent unwanted GC
- Use 'nix-collect-garbage -d' carefully",
        tip: Some("Try: nix-store --verify --repair"),
    },

    Pattern {
        id: "cached-failure",
        category: Category::Build,
        regex_str: r"cached failure of attribute|cached build failure",
        title: "Cached build failure",
        explanation: "A previous build failed and the failure is cached.",
        solution: "\
# Clear the failure cache and retry:
nix build --rebuild

# Or clear all failed:
nix-store --delete $(nix-store -q --failed)",
        deep_dive: "\
WHY THIS HAPPENS:
Nix caches build failures to avoid repeating failed builds. This is 
usually helpful but can be annoying when you've fixed the issue.

CLEARING FAILURE CACHE:

1. SINGLE BUILD:
   nix build --rebuild .#package

2. ALL FAILURES:
   nix-store --delete $(nix-store -q --failed)

3. CHECK WHAT'S CACHED:
   nix-store -q --failed

WHEN THIS HELPS:
- You fixed a build issue
- Transient failure (network, disk space)
- Updated inputs

WHEN IT DOESN'T HELP:
If the build truly fails, clearing cache just wastes time rebuilding.",
        tip: Some("Use --rebuild to ignore cached failure"),
    },

    // =========================================================================
    // HOME-MANAGER ERRORS
    // =========================================================================
    Pattern {
        id: "home-manager-not-found",
        category: Category::Eval,
        regex_str: r#"attribute ['`"]home-manager['`"].*missing|home-manager.*not found|undefined variable ['`"]home-manager['`"]"#,
        title: "Home-Manager not found",
        explanation: "Home-Manager is not available. Missing input or import?",
        solution: "\
# In flake.nix inputs:
inputs.home-manager = {
  url = \"github:nix-community/home-manager\";
  inputs.nixpkgs.follows = \"nixpkgs\";
};

# In outputs:
outputs = { home-manager, ... }: { };",
        deep_dive: "\
WHY THIS HAPPENS:
Home-Manager is a separate project, not part of nixpkgs. You must 
explicitly add it as an input or import it.

FLAKE SETUP:
{
  inputs = {
    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";
    home-manager = {
      url = \"github:nix-community/home-manager\";
      inputs.nixpkgs.follows = \"nixpkgs\";
    };
  };

  outputs = { nixpkgs, home-manager, ... }: {
    # Standalone home-manager:
    homeConfigurations.myuser = home-manager.lib.homeManagerConfiguration {
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      modules = [ ./home.nix ];
    };
    
    # Or as NixOS module:
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      modules = [
        home-manager.nixosModules.home-manager
        ./configuration.nix
      ];
    };
  };
}

NON-FLAKE SETUP:
  imports = [
    (import (builtins.fetchTarball \"https://github.com/nix-community/home-manager/archive/master.tar.gz\"))
  ];",
        tip: Some("Don't forget 'inputs.nixpkgs.follows'"),
    },

    Pattern {
        id: "home-option-not-exist",
        category: Category::NixOS,
        regex_str: r#"The option ['`"](home\.[^'`"]+)['`"] does not exist"#,
        title: "Home-Manager option '$1' doesn't exist",
        explanation: "This Home-Manager option doesn't exist. Typo or missing module?",
        solution: "\
# Search options:
# https://nix-community.github.io/home-manager/options.html

# If from a program module, enable it first:
programs.git.enable = true;",
        deep_dive: "\
WHY THIS HAPPENS:
Home-Manager options must be declared by a module. Common reasons:
1. Typo in option name
2. Program module not enabled
3. Old/new option name change

COMMON MISTAKES:

1. PROGRAM NOT ENABLED:
   # WRONG - option exists but program not enabled:
   programs.git.userName = \"me\";
   
   # RIGHT:
   programs.git.enable = true;
   programs.git.userName = \"me\";

2. WRONG OPTION PATH:
   home.programs.git  # WRONG
   programs.git       # RIGHT

3. DEPRECATED OPTIONS:
   Check release notes for renamed options.

FINDING OPTIONS:
- https://nix-community.github.io/home-manager/options.html
- home-manager option <name>
- Search in home-manager source",
        tip: Some("Enable the program first: programs.X.enable = true"),
    },

    Pattern {
        id: "home-file-collision",
        category: Category::NixOS,
        regex_str: r#"Existing file ['`"]([^'`"]+)['`"].*in the way|collision.*home\.file|would clobber"#,
        title: "Home-Manager file collision: $1",
        explanation: "File '$1' already exists and Home-Manager won't overwrite it.",
        solution: "\
# Option 1 - Backup and let HM manage:
mv ~/.config/file ~/.config/file.backup

# Option 2 - Force overwrite:
home.file.\"path\".force = true;",
        deep_dive: "\
WHY THIS HAPPENS:
Home-Manager refuses to overwrite existing files that it doesn't manage.
This prevents accidental data loss.

SOLUTIONS:

1. BACKUP AND REMOVE:
   mv ~/.config/myfile ~/.config/myfile.bak
   home-manager switch

2. FORCE OVERWRITE:
   home.file.\".config/myfile\".force = true;

3. LET HM GENERATE INITIAL:
   Remove the file, let HM create it, then customize via HM.

4. USE home.activation:
   For complex cases, use activation scripts.

CHECKING WHAT HM MANAGES:
  ls -la ~/.config/  # Symlinks point to /nix/store

BEST PRACTICE:
Let Home-Manager manage dotfiles from the start. Import existing 
configs into your home.nix rather than managing them manually.",
        tip: Some("Backup the file first, then let HM manage it"),
    },

    // =========================================================================
    // FUNCTION / ARGUMENT ERRORS
    // =========================================================================
    Pattern {
        id: "function-expects-argument",
        category: Category::Eval,
        regex_str: r#"function ['`"]([^'`"]+)['`"] called without required argument ['`"]([^'`"]+)['`"]|called without required argument|argument without a value|cannot evaluate a function.*argument"#,
        title: "Function missing argument: $2",
        explanation: "Function '$1' requires argument '$2' but it wasn't provided.",
        solution: "\
# Add the missing argument:
myFunction {
  $2 = <value>;
  # ... other args
}",
        deep_dive: "\
WHY THIS HAPPENS:
Nix functions with attribute set parameters can have required arguments.
If you don't provide them, you get this error.

FUNCTION TYPES:

1. REQUIRED ARGUMENTS:
   f = { a, b }: a + b;
   f { a = 1; }  # ERROR: missing 'b'

2. OPTIONAL WITH DEFAULT:
   f = { a, b ? 0 }: a + b;
   f { a = 1; }  # OK: b defaults to 0

3. WITH ... (EXTRA ARGS ALLOWED):
   f = { a, ... }: a;
   f { a = 1; c = 2; }  # OK: 'c' ignored

FINDING REQUIRED ARGS:
1. Check function definition
2. Look at examples in nixpkgs
3. Use nix repl to explore

COMMON CASES:
- mkDerivation requires 'name' or 'pname+version'
- mkShell requires at least one of buildInputs/packages
- Functions from lib often have required args",
        tip: None,
    },

    Pattern {
        id: "unexpected-argument",
        category: Category::Eval,
        regex_str: r#"called with unexpected argument ['`"]([^'`"]+)['`"]|anonymous function.*does not accept argument"#,
        title: "Unexpected argument: $1",
        explanation: "Function doesn't accept argument '$1'.",
        solution: "\
# Remove the argument or check function signature:
# The function might not have '...' to accept extra args

# If you control the function, add ... :
myFunc = { known, args, ... }: ...",
        deep_dive: "\
WHY THIS HAPPENS:
The function's parameter set doesn't include this argument and doesn't 
have '...' to accept extra arguments.

EXAMPLE:
  # Function without ...:
  f = { a, b }: a + b;
  f { a = 1; b = 2; c = 3; }  # ERROR: unexpected 'c'
  
  # Function with ...:
  f = { a, b, ... }: a + b;
  f { a = 1; b = 2; c = 3; }  # OK: 'c' ignored

COMMON CASES:
1. Typo in argument name
2. Using old argument that was removed
3. Function signature changed in update

FINDING ACCEPTED ARGS:
- Read function definition
- Check documentation
- nix repl and use :doc or look at the function",
        tip: Some("Check for typos in argument name"),
    },

    Pattern {
        id: "not-a-function",
        category: Category::Eval,
        regex_str: r"attempt to call something which is not a function but a ([a-z]+)",
        title: "Not a function (is a $1)",
        explanation: "Tried to call a $1 as if it were a function.",
        solution: "\
# Check what type you have:
builtins.typeOf x

# Common fix - don't call an attrset:
pkgs.hello      # Derivation (correct)
pkgs.hello { }  # WRONG - hello isn't a function",
        deep_dive: "\
WHY THIS HAPPENS:
You used function call syntax (f x or f { }) on something that 
isn't a function.

COMMON MISTAKES:

1. CALLING A DERIVATION:
   # WRONG:
   pkgs.hello { }
   
   # RIGHT (it's already a derivation):
   pkgs.hello

2. CALLING AN ATTRSET:
   # WRONG:
   { foo = 1; } { }
   
   # RIGHT:
   { foo = 1; }

3. MISSING IMPORT:
   # WRONG (path isn't auto-imported):
   ./file.nix { }
   
   # RIGHT:
   (import ./file.nix) { }

4. WRONG OVERRIDE SYNTAX:
   # WRONG:
   pkgs.hello { patches = []; }
   
   # RIGHT:
   pkgs.hello.override { }
   # or
   pkgs.hello.overrideAttrs (old: { })

CHECK THE TYPE:
  nix repl
  nix-repl> builtins.typeOf pkgs.hello
  \"set\"  # It's a set (derivation), not a function!",
        tip: Some("Use builtins.typeOf to check"),
    },

    // =========================================================================
    // FLAKE ADVANCED ERRORS
    // =========================================================================
    Pattern {
        id: "flake-lock-outdated",
        category: Category::Flake,
        regex_str: r#"input ['`"]([^'`"]+)['`"].*out of date|flake\.lock.*not up to date|lock file.*outdated"#,
        title: "Flake input outdated: $1",
        explanation: "The flake.lock file is out of date with flake.nix.",
        solution: "\
# Update all inputs:
nix flake update

# Update specific input:
nix flake lock --update-input $1",
        deep_dive: "\
WHY THIS HAPPENS:
flake.lock pins exact versions of inputs. When you change flake.nix 
(add/remove/modify inputs), the lock file needs updating.

COMMANDS:

UPDATE ALL:
  nix flake update

UPDATE ONE INPUT:
  nix flake lock --update-input nixpkgs

RECREATE LOCK:
  rm flake.lock
  nix flake lock

CHECK INPUT VERSIONS:
  nix flake metadata

PINNING SPECIFIC VERSIONS:
  inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-23.11\";
  # Specific commit:
  inputs.nixpkgs.url = \"github:NixOS/nixpkgs/abc123...\";

CI/CD CONSIDERATION:
Lock files should be committed to git for reproducibility.
Update them intentionally, not automatically.",
        tip: Some("Run 'nix flake update' after changing inputs"),
    },

    Pattern {
        id: "flake-follows-not-found",
        category: Category::Flake,
        regex_str: r#"follows a non-existent input ['`"]([^'`"]+)['`"]|input ['`"]([^'`"]+)['`"].*follows.*does not exist"#,
        title: "Follows non-existent input: $1",
        explanation: "Input tries to follow '$1' but that input doesn't exist.",
        solution: "\
# Make sure the followed input exists:
inputs.nixpkgs.url = \"...\";
inputs.home-manager.inputs.nixpkgs.follows = \"nixpkgs\";",
        deep_dive: "\
WHY THIS HAPPENS:
The 'follows' directive tells an input to use another input's version 
instead of its own. The target must exist.

EXAMPLE:
{
  inputs = {
    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";
    
    home-manager = {
      url = \"github:nix-community/home-manager\";
      # Use OUR nixpkgs, not home-manager's:
      inputs.nixpkgs.follows = \"nixpkgs\";
    };
  };
}

COMMON MISTAKES:
1. Typo in followed input name
2. Forgot to declare the input being followed
3. Removed an input that others follow

CHECKING INPUTS:
  nix flake metadata
  
Shows input tree and what follows what.

WHY USE FOLLOWS:
- Reduces closure size (one nixpkgs instead of many)
- Ensures version consistency
- Faster evaluation",
        tip: Some("Check spelling of the followed input"),
    },

    // =========================================================================
    // BUILD PHASE ERRORS
    // =========================================================================
    Pattern {
        id: "substitute-in-place-failed",
        category: Category::Build,
        regex_str: r"substituteInPlace.*no match for pattern|substituteInPlace:.*pattern.*not found|substituteInPlace.*didn't change anything",
        title: "substituteInPlace pattern not found",
        explanation: "The pattern to replace wasn't found in the file.",
        solution: "\
# Check exact content in file:
cat $src/path/to/file | grep 'pattern'

# Use more flexible pattern or check file exists:
substituteInPlace file --replace-warn 'old' 'new'",
        deep_dive: "\
WHY THIS HAPPENS:
substituteInPlace requires exact pattern match. Common reasons:
1. Whitespace differences
2. Pattern changed in new version
3. File doesn't exist or wrong path

DEBUGGING:

1. CHECK FILE CONTENTS:
   postPatch = ''
     cat path/to/file
   '';

2. USE --replace-warn (doesn't fail):
   substituteInPlace file --replace-warn 'old' 'new'

3. CHECK FILE EXISTS:
   ls -la path/to/

4. USE sed FOR REGEX:
   sed -i 's/pattern/replacement/' file

COMMON ISSUES:
- Upstream changed the file
- Tabs vs spaces
- Line endings (\\r\\n vs \\n)
- Path is different in new version

ALTERNATIVE - PATCHES:
For complex changes, use patch files:
  patches = [ ./fix-something.patch ];",
        tip: Some("Use --replace-warn to not fail on missing pattern"),
    },

    Pattern {
        id: "patch-failed",
        category: Category::Build,
        regex_str: r"applying patch.*failed|patch failed|can't find file to patch",
        title: "Patch failed to apply",
        explanation: "A patch file couldn't be applied to the source.",
        solution: "\
# Regenerate patch for new version:
diff -u original modified > fix.patch

# Or use patchFlags:
patchFlags = [ \"-p0\" ];",
        deep_dive: "\
WHY THIS HAPPENS:
Patches have context lines that must match. When source changes, 
patches may no longer apply cleanly.

DEBUGGING:

1. CHECK PATCH LEVEL:
   Default is -p1 (strips first path component).
   Use patchFlags = [ \"-p0\" ]; if needed.

2. REGENERATE PATCH:
   diff -u file.orig file.new > fix.patch

3. CHECK FUZZ:
   Try: patchFlags = [ \"-F3\" ];
   Allows fuzzy matching of context.

4. MANUAL APPLY:
   Extract source, apply manually, see what fails.

CREATING GOOD PATCHES:
  # In nix-shell with source:
  cp file file.orig
  # edit file
  diff -u file.orig file > fix.patch

NIXPKGS CONVENTION:
Put patches in same dir as default.nix:
  patches = [
    ./fix-build.patch
    (fetchpatch {
      url = \"https://...\";
      hash = \"sha256-...\";
    })
  ];",
        tip: Some("Patches often break on version updates"),
    },

    Pattern {
        id: "patchshebangs-failed",
        category: Category::Build,
        regex_str: r"patchShebangs.*cannot find|cannot execute.*after patchShebangs|interpreter.*not found",
        title: "patchShebangs failed",
        explanation: "Script interpreter couldn't be found or patched.",
        solution: "\
# Add interpreter to nativeBuildInputs:
nativeBuildInputs = [ bash python3 ];

# Or skip patching for specific file:
dontPatchShebangs = true;
postFixup = ''patchShebangs --host $out/bin/script'';",
        deep_dive: "\
WHY THIS HAPPENS:
Nix automatically patches #!/usr/bin/env python (etc.) to use Nix 
store paths. If the interpreter isn't in the build environment, 
patchShebangs can't find what to patch to.

SOLUTIONS:

1. ADD INTERPRETER TO BUILD:
   nativeBuildInputs = [ python3 ];

2. DISABLE AUTO-PATCHING:
   dontPatchShebangs = true;

3. PATCH MANUALLY:
   postFixup = ''
     patchShebangs --host $out/bin/myscript
   '';

4. SUBSTITUTE DIRECTLY:
   substituteInPlace script.py \\
     --replace '#!/usr/bin/env python' '#!${python3}/bin/python'

UNDERSTANDING patchShebangs:
- --build: For scripts used during build
- --host: For scripts in final output
- Default patches everything it finds

COMMON INTERPRETERS:
- bash, sh -> stdenv.shell
- python -> python3
- perl -> perl
- ruby -> ruby",
        tip: Some("Add the interpreter to nativeBuildInputs"),
    },

    Pattern {
        id: "ifd-disabled",
        category: Category::Eval,
        regex_str: r"import from derivation.*disabled|IFD is not allowed|importfromderivation.*restricted",
        title: "Import From Derivation (IFD) disabled",
        explanation: "Trying to import Nix code from a build result, but IFD is disabled.",
        solution: "\
# Enable IFD (if you control the system):
nix.settings.allow-import-from-derivation = true;

# Or refactor to avoid IFD",
        deep_dive: "\
WHY THIS HAPPENS:
Import From Derivation (IFD) means importing .nix files that are 
produced by a build. It's disabled by default in some contexts 
because it hurts evaluation performance.

WHAT IS IFD:
  # This is IFD:
  import (pkgs.writeText \"foo.nix\" \"{ a = 1; }\")
  
  # The writeText must be BUILT before we can evaluate further.

WHY IT'S PROBLEMATIC:
- Blocks parallel evaluation
- Must build before eval can continue
- Makes caching harder
- Hydra/CI often disables it

COMMON IFD SOURCES:
- Code generators (protobuf, etc.)
- dreamlock from dream2nix
- Some language2nix tools

ALTERNATIVES:
1. Generate code at build time, not eval time
2. Pre-generate and commit to repo
3. Use fixed-output derivations

ENABLING (if needed):
  # NixOS:
  nix.settings.allow-import-from-derivation = true;
  
  # nix.conf:
  allow-import-from-derivation = true",
        tip: Some("IFD slows evaluation - avoid if possible"),
    },

    // =========================================================================
    // SYSTEM / PLATFORM ERRORS  
    // =========================================================================
    Pattern {
        id: "unsupported-system",
        category: Category::Eval,
        regex_str: r#"is not supported on ['`"]([^'`"]+)['`"]|unsupported system.*['`"]([^'`"]+)['`"]|not available on your system"#,
        title: "Unsupported system: $1",
        explanation: "This package doesn't support your system architecture.",
        solution: "\
# Check supported platforms:
nix eval nixpkgs#hello.meta.platforms

# For cross-compilation:
nix build .#packages.x86_64-linux.hello",
        deep_dive: "\
WHY THIS HAPPENS:
Not all packages work on all systems. Common reasons:
1. Binary-only packages (Steam, Spotify)
2. Platform-specific code
3. Missing cross-compilation support

SYSTEMS IN NIX:
- x86_64-linux: Most Linux PCs
- aarch64-linux: ARM Linux (Raspberry Pi)
- x86_64-darwin: Intel Macs
- aarch64-darwin: Apple Silicon Macs

CHECKING SUPPORT:
  nix eval nixpkgs#hello.meta.platforms
  nix eval nixpkgs#hello.meta.badPlatforms

WORKING AROUND:

1. CROSS-COMPILE:
   nix build --system x86_64-linux

2. EMULATION (slow):
   boot.binfmt.emulatedSystems = [ \"aarch64-linux\" ];

3. CHECK ALTERNATIVES:
   Some packages have different variants:
   - linuxPackages vs linuxPackages_latest
   - Package variants for different platforms

FLAKES AND SYSTEMS:
  outputs = { ... }: {
    packages.x86_64-linux.default = ...;
    packages.aarch64-linux.default = ...;
  };",
        tip: Some("Check meta.platforms for supported systems"),
    },

    Pattern {
        id: "unfree-not-allowed",
        category: Category::Eval,
        regex_str: r"is not free|unfree.*not allowed|has an unfree license|refusing.*unfree",
        title: "Unfree package not allowed",
        explanation: "This package has a non-free license and unfree packages aren't enabled.",
        solution: "\
# Allow all unfree (NixOS):
nixpkgs.config.allowUnfree = true;

# Or per-package:
nixpkgs.config.allowUnfreePredicate = pkg:
  builtins.elem (lib.getName pkg) [ \"steam\" ];",
        deep_dive: "\
WHY THIS HAPPENS:
Nix respects software freedom by default. Packages with proprietary 
licenses must be explicitly allowed.

ENABLING UNFREE:

1. NIXOS (configuration.nix):
   nixpkgs.config.allowUnfree = true;

2. HOME-MANAGER:
   nixpkgs.config.allowUnfree = true;

3. FLAKES (in flake.nix):
   nixpkgs.legacyPackages.x86_64-linux.override {
     config.allowUnfree = true;
   };
   # Or:
   import nixpkgs { config.allowUnfree = true; };

4. ENVIRONMENT VARIABLE:
   export NIXPKGS_ALLOW_UNFREE=1
   nix build ...

SELECTIVE UNFREE:
  nixpkgs.config.allowUnfreePredicate = pkg:
    builtins.elem (lib.getName pkg) [
      \"nvidia-x11\"
      \"nvidia-settings\"
      \"steam\"
      \"spotify\"
    ];

CHECKING LICENSE:
  nix eval nixpkgs#hello.meta.license",
        tip: Some("Use allowUnfreePredicate for selective unfree"),
    },

    Pattern {
        id: "broken-package",
        category: Category::Eval,
        regex_str: r"is marked as broken|evaluating.*broken package|is broken.*refusing",
        title: "Package is marked broken",
        explanation: "This package is known to be broken in nixpkgs.",
        solution: "\
# Allow broken (not recommended):
nixpkgs.config.allowBroken = true;

# Better: check why it's broken and find alternative",
        deep_dive: "\
WHY THIS HAPPENS:
Packages are marked broken when:
1. Build fails consistently
2. Critical bugs
3. Security issues
4. Unmaintained and outdated

CHECKING WHY:
  nix eval nixpkgs#package.meta.broken
  nix eval nixpkgs#package.meta.knownVulnerabilities

ALLOWING BROKEN (NOT RECOMMENDED):
  nixpkgs.config.allowBroken = true;

  # Or per-package:
  (pkg.overrideAttrs { meta.broken = false; })

BETTER OPTIONS:
1. Find alternative package
2. Use older nixpkgs where it worked
3. Fix and contribute to nixpkgs
4. Use upstream source directly

OVERLAYS TO FIX:
  nixpkgs.overlays = [(final: prev: {
    broken-pkg = prev.broken-pkg.overrideAttrs (old: {
      # Apply fixes
      patches = old.patches or [] ++ [ ./fix.patch ];
      meta = old.meta // { broken = false; };
    });
  })];",
        tip: Some("Check nixpkgs issues for the package"),
    },

    Pattern {
        id: "insecure-package",
        category: Category::Eval,
        regex_str: r"is marked as insecure|known security vulnerabilities|insecure.*refusing",
        title: "Package has security vulnerabilities",
        explanation: "This package has known security issues.",
        solution: "\
# Check vulnerabilities:
nix eval nixpkgs#pkg.meta.knownVulnerabilities

# If you must use it:
nixpkgs.config.permittedInsecurePackages = [
  \"openssl-1.1.1w\"
];",
        deep_dive: "\
WHY THIS HAPPENS:
Nixpkgs tracks CVEs and marks vulnerable packages. This protects you 
from accidentally using insecure software.

CHECKING VULNERABILITIES:
  nix eval nixpkgs#openssl.meta.knownVulnerabilities

ALLOWING INSECURE (USE WITH CAUTION):
  # NixOS:
  nixpkgs.config.permittedInsecurePackages = [
    \"electron-25.9.0\"
    \"openssl-1.1.1w\"
  ];

  # Flakes:
  nixpkgs.legacyPackages.x86_64-linux.override {
    config.permittedInsecurePackages = [ \"...\" ];
  };

BETTER OPTIONS:
1. Update to fixed version
2. Find secure alternative
3. Isolate in container/VM
4. Check if vulnerability applies to your use

DEPENDENCY PULLING INSECURE:
Sometimes a dependency requires insecure package:
  nix why-depends .#mypackage nixpkgs#insecure-pkg",
        tip: Some("Update to patched version if possible"),
    },

    // =========================================================================
    // GC / STORE MANAGEMENT ERRORS
    // =========================================================================
    Pattern {
        id: "gc-root-protected",
        category: Category::Build,
        regex_str: r"cannot delete.*because it is a root|refusing to delete.*gc root|is still alive",
        title: "Cannot delete: path is a GC root",
        explanation: "This path is protected by a garbage collector root.",
        solution: "\
# List GC roots:
nix-store --gc --print-roots

# Remove root:
rm /nix/var/nix/gcroots/auto/<link>

# Then garbage collect:
nix-collect-garbage",
        deep_dive: "\
WHY THIS HAPPENS:
GC roots protect store paths from garbage collection. Nix won't delete 
paths that are still referenced.

TYPES OF ROOTS:
1. User profiles (~/.nix-profile)
2. System profile (/run/current-system)
3. Result symlinks (./result)
4. Auto roots (/nix/var/nix/gcroots/auto)

LISTING ROOTS:
  nix-store --gc --print-roots
  nix-store -q --roots /nix/store/<path>

REMOVING ROOTS:

1. REMOVE RESULT LINKS:
   rm result result-*

2. REMOVE OLD PROFILES:
   nix-collect-garbage -d  # Deletes old generations

3. MANUAL ROOT REMOVAL:
   rm /nix/var/nix/gcroots/auto/<link>

4. NIXOS GENERATIONS:
   sudo nix-collect-garbage -d

KEEPING ROOTS:
To prevent GC of a path:
  nix-store --add-root /path/to/link -r /nix/store/<path>",
        tip: Some("Use 'nix-collect-garbage -d' to remove old generations"),
    },

    Pattern {
        id: "disk-full",
        category: Category::Build,
        regex_str: r"No space left on device|disk full|ENOSPC|out of disk space",
        title: "Disk full",
        explanation: "Not enough disk space for build or store operations.",
        solution: "\
# Free space with garbage collection:
nix-collect-garbage -d

# Check usage:
nix-store --gc --print-dead | wc -l
du -sh /nix/store",
        deep_dive: "\
WHY THIS HAPPENS:
/nix/store can grow large over time:
- Multiple package versions
- Old system generations
- Build artifacts

FREEING SPACE:

1. QUICK CLEANUP:
   nix-collect-garbage

2. AGGRESSIVE (removes old generations):
   nix-collect-garbage -d
   # NixOS:
   sudo nix-collect-garbage -d

3. CHECK WHAT WOULD BE FREED:
   nix-store --gc --print-dead | head

4. DELETE SPECIFIC GENERATIONS:
   nix-env --delete-generations 30d  # Older than 30 days

5. NIXOS BOOT ENTRIES:
   sudo nix-collect-garbage -d
   sudo nixos-rebuild boot

CHECKING USAGE:
  df -h /nix
  du -sh /nix/store
  nix path-info -Sh /run/current-system

REDUCING FUTURE GROWTH:
- Keep fewer generations
- Auto-gc in NixOS:
  nix.gc = {
    automatic = true;
    dates = \"weekly\";
    options = \"--delete-older-than 30d\";
  };",
        tip: Some("Run 'nix-collect-garbage -d' regularly"),
    },

    // =========================================================================
    // PACKAGE RENAMED / REMOVED
    // =========================================================================
    Pattern {
        id: "package-renamed",
        category: Category::Eval,
        regex_str: r#"has been renamed|was renamed|Use [`'"]([^'`"]+)[`'"] instead"#,
        title: "Package renamed to '$1'",
        explanation: "This package was renamed. Use the new name.",
        solution: "\
# Replace in your configuration:
# old-name  ->  $1",
        deep_dive: "\
WHY THIS HAPPENS:
Nixpkgs renames packages for:
- Consistency (python3Packages not pythonPackages)
- Clarity (package-qt5 vs package-qt6)
- Upstream changes

WHERE TO CHANGE:
- configuration.nix
- home.nix  
- flake.nix
- shell.nix

FINDING ALL OCCURRENCES:
  grep -r 'old-name' /etc/nixos/
  grep -r 'old-name' ~/.config/nixpkgs/",
        tip: Some("Just follow the rename suggestion"),
    },

    Pattern {
        id: "package-removed",
        category: Category::Eval,
        regex_str: r"has been removed|was removed from nixpkgs|is no longer available",
        title: "Package has been removed",
        explanation: "This package was removed from nixpkgs.",
        solution: "\
# Search for alternative:
nix search nixpkgs <similar-name>

# Or use older nixpkgs:
inputs.nixpkgs-old.url = \"github:NixOS/nixpkgs/nixos-23.11\";",
        deep_dive: "\
WHY THIS HAPPENS:
Packages get removed because:
- Unmaintained/abandoned upstream
- Security issues
- Replaced by better alternative
- License problems

ALTERNATIVES:

1. FIND SIMILAR PACKAGE:
   nix search nixpkgs ...
   https://search.nixos.org

2. USE OLDER NIXPKGS:
   Pin to a version where it existed.

3. PACKAGE IT YOURSELF:
   Create your own derivation if needed.",
        tip: Some("Check if there's an alternative"),
    },

    // =========================================================================
    // NETWORK ERRORS
    // =========================================================================
    Pattern {
        id: "network-timeout",
        category: Category::Fetch,
        regex_str: r"timed? ?out|Connection timed out|timeout.*connect|Operation timed out",
        title: "Network timeout",
        explanation: "Connection timed out while fetching.",
        solution: "\
# Retry (often works):
nix build

# Increase timeout:
nix build --option connect-timeout 60

# Build offline if possible:
nix build --offline",
        deep_dive: "\
COMMON CAUSES:
1. Slow/unstable internet connection
2. Server overloaded
3. Firewall blocking
4. DNS issues

SOLUTIONS:

1. INCREASE TIMEOUT:
   nix build --option connect-timeout 60

2. USE DIFFERENT CACHE:
   Use alternative binary cache or build locally.

3. BUILD OFFLINE:
   nix build --offline
   (Only works if everything is in local store)

4. JUST RETRY:
   Often transient - try again.",
        tip: Some("Often just retrying works"),
    },

    Pattern {
        id: "cannot-resolve-host",
        category: Category::Fetch,
        regex_str: r"[Cc]ould not resolve host|Name or service not known|getaddrinfo.*failed|DNS.*failed",
        title: "Cannot resolve hostname",
        explanation: "DNS resolution failed for the server.",
        solution: "\
# Check DNS:
nslookup cache.nixos.org

# Try different DNS:
echo 'nameserver 8.8.8.8' | sudo tee /etc/resolv.conf",
        deep_dive: "\
COMMON CAUSES:
1. No internet connection
2. DNS server unreachable
3. Firewall blocking DNS (port 53)
4. VPN issues

DEBUGGING:
  ping cache.nixos.org
  nslookup cache.nixos.org
  dig cache.nixos.org

SOLUTIONS:
1. Check internet connection
2. Change DNS server in /etc/resolv.conf
3. Disable/enable VPN
4. Restart router",
        tip: Some("Check your internet connection"),
    },

    Pattern {
        id: "ssl-certificate-error",
        category: Category::Fetch,
        regex_str: r"SSL certificate problem|certificate verify failed|unable to get local issuer|CERTIFICATE_VERIFY_FAILED",
        title: "SSL certificate error",
        explanation: "SSL/TLS certificate could not be verified.",
        solution: "\
# Check system time (common cause!):
date

# Update CA certificates:
sudo nixos-rebuild switch",
        deep_dive: "\
COMMON CAUSES:
1. WRONG SYSTEM TIME (very common!)
   Certificates have validity periods.

2. MISSING CA CERTIFICATES
   Especially in minimal systems.

3. MITM/PROXY
   Corporate proxy with custom certificates.

SOLUTIONS:

1. SYNC TIME:
   sudo systemctl restart systemd-timesyncd
   # Or:
   sudo ntpdate pool.ntp.org

2. CA CERTIFICATES:
   security.pki.certificateFiles = [ ./corp-ca.crt ];

3. PROXY CERTS:
   Add corporate CA to trusted certificates.",
        tip: Some("Check your system time first!"),
    },

    // =========================================================================
    // NIXOS-REBUILD SPECIFIC
    // =========================================================================
    Pattern {
        id: "nixos-config-not-found",
        category: Category::Flake,
        regex_str: r#"nixosConfigurations\.([^\s'`"]+).*does not exist|flake.*does not provide.*nixosConfigurations"#,
        title: "NixOS configuration '$1' not found",
        explanation: "nixosConfigurations.$1 doesn't exist in the flake.",
        solution: "\
# List available configurations:
nix flake show

# Use correct hostname:
sudo nixos-rebuild switch --flake .#<hostname>",
        deep_dive: "\
WHY THIS HAPPENS:
The hostname in --flake .#<name> must match a key in 
nixosConfigurations in your flake.

CHECK:
  hostname  # Current hostname
  nix flake show  # Available configs

TYPICAL STRUCTURE:
  nixosConfigurations = {
    my-pc = nixpkgs.lib.nixosSystem { ... };
    laptop = nixpkgs.lib.nixosSystem { ... };
  };

COMMON MISTAKES:
- Typo in hostname
- Changed hostname without updating flake
- Case sensitivity",
        tip: Some("Hostname must match flake.nix"),
    },

    Pattern {
        id: "activation-script-failed",
        category: Category::NixOS,
        regex_str: r"activation script.*failed|error during activation|activating.*failed",
        title: "Activation script failed",
        explanation: "A NixOS activation script failed.",
        solution: "\
# Check error in journal:
journalctl -xe

# Manually activate with debug:
sudo /nix/var/nix/profiles/system/activate",
        deep_dive: "\
WHY THIS HAPPENS:
Activation scripts run after build to configure the system.
Failures can occur due to:

1. PERMISSION ISSUES
2. SERVICE COULDN'T START
3. SYMLINK CONFLICT
4. MISSING RUNTIME DEPENDENCY

DEBUGGING:
  journalctl -xe
  systemctl status <service>
  
COMMON CAUSES:
- Old state conflicting with new
- Service needs manual migration
- Hardcoded paths in service",
        tip: Some("Check journalctl -xe for details"),
    },

    Pattern {
        id: "boot-loader-failed",
        category: Category::NixOS,
        regex_str: r"[Bb]oot ?[Ll]oader.*failed|grub-install.*failed|cannot install.*boot|EFI.*failed",
        title: "Bootloader installation failed",
        explanation: "The bootloader could not be installed.",
        solution: "\
# Reinstall GRUB:
sudo grub-install /dev/sda
sudo nixos-rebuild boot

# Or check EFI partition:
mount | grep boot",
        deep_dive: "\
COMMON CAUSES:

1. EFI PARTITION NOT MOUNTED:
   mount /dev/sda1 /boot
   
2. FULL BOOT PARTITION:
   df -h /boot
   # Remove old kernels

3. WRONG DEVICE:
   boot.loader.grub.device = \"/dev/sda\";
   # Not /dev/sda1!

4. UEFI vs BIOS MISMATCH:
   boot.loader.grub.efiSupport = true;
   boot.loader.efi.canTouchEfiVariables = true;

RECOVERY:
Boot from NixOS USB and:
  mount /dev/sda2 /mnt
  mount /dev/sda1 /mnt/boot
  nixos-enter
  nixos-rebuild boot",
        tip: Some("For EFI: Check if /boot is mounted"),
    },

    Pattern {
        id: "systemd-service-failed",
        category: Category::NixOS,
        regex_str: r"systemd.*service.*failed|Failed to start|Unit.*failed|job.*failed",
        title: "Systemd service failed",
        explanation: "A systemd service failed to start after switch.",
        solution: "\
# Check status:
systemctl status <service>
journalctl -u <service>

# Restart:
sudo systemctl restart <service>",
        deep_dive: "\
DEBUGGING:

1. STATUS AND LOGS:
   systemctl status <service>
   journalctl -u <service> -e

2. CHECK CONFIG:
   systemctl cat <service>

3. DEPENDENCIES:
   systemctl list-dependencies <service>

COMMON CAUSES:
- Port already in use
- Permission issues
- Configuration error
- Missing runtime deps

MANUAL TEST:
  sudo -u <user> /nix/store/.../bin/<program>",
        tip: Some("journalctl -u <service> shows logs"),
    },

    Pattern {
        id: "switch-to-configuration-failed",
        category: Category::NixOS,
        regex_str: r"switch-to-configuration.*failed|switching to.*failed|activation.*did not succeed",
        title: "switch-to-configuration failed",
        explanation: "The new configuration could not be activated.",
        solution: "\
# Use previous generation:
sudo nixos-rebuild switch --rollback

# Or at boot:
# Select older generation in GRUB menu",
        deep_dive: "\
WHY THIS HAPPENS:
Activating the new configuration failed.
System is still running old configuration.

ROLLBACK:
  sudo nixos-rebuild switch --rollback
  
  # Or: Boot into old generation via GRUB/systemd-boot

DEBUGGING:
  journalctl -xe
  systemctl --failed

COMMON CAUSES:
- Service start failure
- Permission issues
- Resource conflicts (ports, files)
- Incompatible state migration",
        tip: Some("Rollback is safe and fast"),
    },

    Pattern {
        id: "dependency-build-failed",
        category: Category::Build,
        regex_str: r"dependencies couldn't be built|dependency.*failed|cannot build.*dependencies",
        title: "Dependency failed to build",
        explanation: "A dependency couldn't be built.",
        solution: "\
# Check full log:
nix log <failed-derivation>

# Or with more output:
nix build -L",
        deep_dive: "\
WHY THIS HAPPENS:
A package in your dependency chain failed to build.
Your package itself is probably fine.

DEBUGGING:

1. WHICH DEPENDENCY:
   The error message names the failed derivation.

2. CHECK LOG:
   nix log /nix/store/<hash>-<name>.drv

3. FIND CAUSE:
   Is it a nixpkgs bug? Check GitHub issues.

SOLUTIONS:
- Update nixpkgs (bug might be fixed)
- Roll back to last working version
- Report bug to nixpkgs",
        tip: Some("Check nixpkgs GitHub issues"),
    },

    // =========================================================================
    // LOCK / PERMISSION
    // =========================================================================
    Pattern {
        id: "resource-locked",
        category: Category::Build,
        regex_str: r"[Rr]esource.*unavailable|acquiring.*lock|lock.*held|database is locked",
        title: "Resource is locked",
        explanation: "A Nix resource is locked by another process.",
        solution: "\
# Check for other Nix processes:
ps aux | grep nix

# Remove lock manually (careful!):
sudo rm /nix/var/nix/gc.lock",
        deep_dive: "\
WHY THIS HAPPENS:
Nix uses locks to prevent concurrent access.

COMMON CAUSES:
1. Another nix-build is running
2. nix-collect-garbage is running
3. Crashed Nix process left a lock

SOLUTIONS:

1. WAIT:
   If another build is running, wait for it.

2. FIND PROCESS:
   ps aux | grep nix
   # Kill if it's a zombie

3. REMOVE LOCK:
   sudo rm /nix/var/nix/gc.lock
   sudo rm /nix/var/nix/db/db.lock
   
   CAREFUL: Only if sure no other process is running!",
        tip: Some("Wait for other Nix processes to finish"),
    },

    Pattern {
        id: "not-authorized-daemon",
        category: Category::Build,
        regex_str: r"not.*authori[sz]ed.*daemon|permission denied.*daemon|cannot connect.*permission",
        title: "Not authorized to use Nix daemon",
        explanation: "You don't have permission to use the Nix daemon.",
        solution: "\
# Add user to nix group:
sudo usermod -aG nixbld $USER

# Then re-login or:
newgrp nixbld",
        deep_dive: "\
WHY THIS HAPPENS:
Multi-user Nix installations require group membership.

SOLUTIONS:

1. ADD TO GROUP:
   sudo usermod -aG nixbld $USER
   # Then re-login!

2. NIXOS:
   users.users.myuser.extraGroups = [ \"wheel\" ];
   # wheel can usually use sudo nix

3. TRUSTED USER:
   nix.settings.trusted-users = [ \"myuser\" ];

CHECK:
  groups  # Shows your groups
  ls -la /nix/var/nix/daemon-socket/",
        tip: Some("Re-login after group change"),
    },

    // =========================================================================
    // FLAKE SPECIFIC
    // =========================================================================
    Pattern {
        id: "flake-not-found",
        category: Category::Flake,
        regex_str: r"flake\.nix.*not found|does not contain.*flake\.nix|no flake\.nix",
        title: "flake.nix not found",
        explanation: "No flake.nix was found at the specified path/repository.",
        solution: "\
# Check if flake.nix exists:
ls -la flake.nix

# For git repos - correct branch?
git branch -a",
        deep_dive: "\
COMMON CAUSES:

1. WRONG DIRECTORY:
   cd /path/to/flake
   ls flake.nix

2. WRONG GIT BRANCH:
   git checkout main
   
3. FLAKE.NIX NOT COMMITTED:
   git add flake.nix
   git commit -m 'Add flake'

4. URL WRONG:
   github:user/repo  # Not github:user/repo.git
   
5. PRIVATE REPO:
   git+ssh://git@github.com/user/private-repo",
        tip: Some("Is flake.nix committed to git?"),
    },

    Pattern {
        id: "dirty-git-tree",
        category: Category::Flake,
        regex_str: r"dirty|uncommitted changes|Git tree.*is dirty|has a dirty input",
        title: "Git tree has uncommitted changes",
        explanation: "The git repository has uncommitted changes.",
        solution: "\
# Commit changes:
git add -A && git commit -m 'Update'

# Or allow dirty (for testing):
nix build --impure",
        deep_dive: "\
WHY THIS HAPPENS:
Flakes want a clean git state for reproducibility.
Uncommitted files are IGNORED!

IMPORTANT:
New files that aren't committed won't be seen!

SOLUTIONS:

1. COMMIT:
   git add -A
   git commit -m 'WIP'

2. ALLOW DIRTY (testing only):
   nix build .#package --impure
   
3. LOCAL PATH INSTEAD OF GIT:
   nix build path:.#package",
        tip: Some("Uncommitted files are ignored!"),
    },

    Pattern {
        id: "pure-eval-forbidden",
        category: Category::Eval,
        regex_str: r"access to (absolute )?path.*forbidden|not allowed.*pure eval|pure evaluation mode",
        title: "Absolute path forbidden in pure eval",
        explanation: "Access to absolute path is not allowed in pure evaluation mode.",
        solution: "\
# Use relative path:
./config instead of /home/user/config

# Or in flake.nix:
src = ./.;",
        deep_dive: "\
WHY THIS HAPPENS:
Flakes enforce 'pure evaluation' - no side effects, no absolute 
paths, full reproducibility.

FORBIDDEN:
  /home/user/file
  /etc/nixos/config.nix
  builtins.getEnv \"HOME\"

ALLOWED:
  ./file              # Relative to flake.nix
  self                # The flake itself
  inputs.nixpkgs      # Declared inputs

WORKAROUND (not recommended):
  nix build --impure",
        tip: Some("Use relative paths: ./foo not /absolute/foo"),
    },

    // =========================================================================
    // COMMON TYPOS / MISTAKES
    // =========================================================================
    Pattern {
        id: "not-in-nixpkgs",
        category: Category::Eval,
        regex_str: r"does not provide.*attribute|cannot find.*in nixpkgs|package.*not found",
        title: "Package not found in nixpkgs",
        explanation: "The package could not be found in nixpkgs.",
        solution: "\
# Search with correct name:
nix search nixpkgs <name>

# Search online:
# https://search.nixos.org/packages",
        deep_dive: "\
POSSIBLE CAUSES:

1. TYPO:
   pkgs.htoop  # -> pkgs.htop

2. DIFFERENT NAME:
   pkgs.openjdk  # Not pkgs.java
   pkgs.python3  # Not pkgs.python

3. IN SUBGROUP:
   pkgs.python3Packages.numpy
   pkgs.nodePackages.typescript

4. DOESN'T EXIST IN NIXPKGS:
   -> Package yourself or find alternative

SEARCH:
  nix search nixpkgs <name>
  https://search.nixos.org/packages",
        tip: Some("Check exact spelling at search.nixos.org"),
    },

    Pattern {
        id: "file-conflict-activation",
        category: Category::NixOS,
        regex_str: r"[Ff]ile.*exists|refusing to overwrite|would clobber|[Cc]onflict.*existing",
        title: "File conflict during activation",
        explanation: "A file already exists and cannot be replaced.",
        solution: "\
# Backup old file:
sudo mv /conflict/file /conflict/file.bak

# Then retry:
sudo nixos-rebuild switch",
        deep_dive: "\
WHY THIS HAPPENS:
NixOS/Home-Manager want to manage a file that already exists.

COMMON WITH:
- /etc/... configuration files
- ~/.config/... user configs
- Symlinks pointing to nothing

SOLUTIONS:

1. BACKUP AND REMOVE:
   sudo mv /etc/file /etc/file.bak
   
2. FORCE (Home-Manager):
   home.file.\"path\".force = true;

3. CHECK WHAT IT IS:
   ls -la /path/to/file
   file /path/to/file",
        tip: Some("Backup the file, then remove it"),
    },

    Pattern {
        id: "nar-hash-mismatch",
        category: Category::Fetch,
        regex_str: r"NAR hash mismatch|hash mismatch in fixed-output|expected.*got.*sha256",
        title: "NAR hash mismatch",
        explanation: "Downloaded content has unexpected hash.",
        solution: "\
# Retry (often temporary):
nix build --rebuild

# Clear cache:
nix-store --delete /nix/store/<hash>...",
        deep_dive: "\
WHY THIS HAPPENS:
Binary cache returned data with wrong hash.

CAUSES:
1. Corrupted download
2. Cache server issue
3. Man-in-the-middle (rare)
4. Local disk corruption

SOLUTIONS:

1. RETRY:
   nix build --rebuild

2. USE DIFFERENT CACHE:
   nix build --option substituters ''
   # Builds locally instead of downloading

3. DELETE PATH AND RETRY:
   nix-store --delete /nix/store/<hash>...
   nix build

4. VERIFY STORE:
   nix-store --verify --check-contents",
        tip: Some("Usually just retrying works"),
    },

    // =========================================================================
    // SUPER COMMON DAILY ERRORS
    // =========================================================================
    Pattern {
        id: "need-root",
        category: Category::NixOS,
        regex_str: r"you need to be root|requires root|[Pp]ermission denied.*nix-env|cannot.*without root|must be run as root|Run as root|needs root privileges",
        title: "Root/sudo required",
        explanation: "This operation requires root privileges.",
        solution: "\
# Run with sudo:
sudo nixos-rebuild switch
sudo nix-collect-garbage",
        deep_dive: "\
OPERATIONS THAT NEED ROOT:
- nixos-rebuild switch/boot/test
- nix-collect-garbage (system-wide)
- Installing to system profile
- Modifying /nix/var

OPERATIONS THAT DON'T:
- nix build
- nix develop  
- nix-shell
- User profile changes (nix-env for user)

IF SUDO DOESN'T WORK:
Check if you're in the wheel group:
  groups
  
Add yourself:
  sudo usermod -aG wheel $USER",
        tip: Some("Use sudo for system operations"),
    },

    Pattern {
        id: "git-not-found",
        category: Category::Build,
        regex_str: r"git.*not found|cannot find.*git|git:.*No such file|command not found.*git|Git is required",
        title: "Git not found",
        explanation: "Git is required but not installed or not in PATH.",
        solution: "\
# NixOS - add to configuration.nix:
environment.systemPackages = [ pkgs.git ];

# Or temporarily:
nix-shell -p git",
        deep_dive: "\
WHY THIS HAPPENS:
Flakes and many fetch operations require git.

SOLUTIONS:

1. NIXOS - PERMANENT:
   environment.systemPackages = [ pkgs.git ];
   sudo nixos-rebuild switch

2. TEMPORARY:
   nix-shell -p git
   # Then run your command

3. IN FLAKE devShell:
   devShells.default = mkShell {
     packages = [ git ];
   };

COMMON CASES:
- Flake operations (nix build, nix develop)
- fetchGit, fetchFromGitHub
- Any source from git repos",
        tip: Some("Add git to systemPackages"),
    },

    Pattern {
        id: "channel-not-found",
        category: Category::Eval,
        regex_str: r#"file ['`"]nixpkgs['`"] was not found|cannot find channel|channel.*not found|NIX_PATH.*nixpkgs"#,
        title: "Nixpkgs channel not found",
        explanation: "No nixpkgs channel is configured.",
        solution: "\
# Add channel:
nix-channel --add https://nixos.org/channels/nixos-unstable nixpkgs
nix-channel --update

# Or use flakes instead (recommended)",
        deep_dive: "\
WHY THIS HAPPENS:
The old Nix (pre-flakes) uses channels via NIX_PATH.
If no channel is set, <nixpkgs> can't be found.

FOR TRADITIONAL NIX:
  nix-channel --add https://nixos.org/channels/nixos-unstable nixpkgs
  nix-channel --update
  
  # Check:
  nix-channel --list
  echo $NIX_PATH

FOR FLAKES (RECOMMENDED):
Don't use channels. Use flake inputs:
  inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";

TEMPORARY FIX:
  NIX_PATH=nixpkgs=https://github.com/NixOS/nixpkgs/archive/master.tar.gz nix-build ...",
        tip: Some("Consider switching to flakes"),
    },

    Pattern {
        id: "value-is-null",
        category: Category::Eval,
        regex_str: r"value is null|null.*while.*expected|cannot coerce null|assertion.*null",
        title: "Value is null",
        explanation: "A value is null when something else was expected.",
        solution: "\
# Add default value:
myValue = config.foo.bar or \"default\";

# Or check for null:
if myValue != null then ... else ...",
        deep_dive: "\
WHY THIS HAPPENS:
An attribute access returned null, but you tried to use it.

COMMON CAUSES:
1. Optional attribute not set
2. Wrong attribute path
3. Conditional that returned null

SOLUTIONS:

1. DEFAULT VALUE:
   myValue = config.foo.bar or \"default\";
   myPkg = pkgs.optionalPackage or pkgs.fallback;

2. NULL CHECK:
   if myValue != null then myValue else \"fallback\"

3. OPTIONAL WITH lib:
   lib.optionalAttrs (myValue != null) { inherit myValue; }

4. FIND THE SOURCE:
   Use --show-trace to find where null came from.",
        tip: Some("Use 'or' for default values"),
    },

    Pattern {
        id: "attribute-already-defined",
        category: Category::Eval,
        regex_str: r#"attribute ['`"]([^'`"]+)['`"].*already defined|duplicate.*attribute|defined multiple times"#,
        title: "Attribute '$1' already defined",
        explanation: "The attribute '$1' is defined more than once.",
        solution: "\
# Merge instead of override:
{ a = 1; } // { b = 2; }

# Or use lib.mkMerge:
lib.mkMerge [ config1 config2 ]",
        deep_dive: "\
WHY THIS HAPPENS:
In Nix, you can't define the same attribute twice in a set.

WRONG:
{
  foo = 1;
  foo = 2;  # ERROR!
}

SOLUTIONS:

1. USE DIFFERENT NAMES:
   {
     foo = 1;
     foo2 = 2;
   }

2. MERGE SETS:
   { a = 1; } // { b = 2; }

3. NIXOS MODULES - USE mkMerge:
   {
     services.nginx = lib.mkMerge [
       { enable = true; }
       (lib.mkIf condition { ... })
     ];
   }

4. USE lib.recursiveUpdate:
   lib.recursiveUpdate { a.b = 1; } { a.c = 2; }

COMMON IN MODULES:
Split into separate files or use mkMerge.",
        tip: Some("Use // or lib.mkMerge to combine"),
    },

    Pattern {
        id: "out-of-memory",
        category: Category::Build,
        regex_str: r"[Oo]ut of memory|OOM|Cannot allocate memory|memory exhausted|killed.*memory|oom-killer",
        title: "Out of memory",
        explanation: "The build ran out of memory.",
        solution: "\
# Limit parallel jobs:
nix build -j 1

# Or increase swap:
sudo fallocate -l 8G /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile",
        deep_dive: "\
WHY THIS HAPPENS:
Some packages need lots of RAM to build:
- Chromium, Firefox
- LLVM, GCC
- Large Rust projects

SOLUTIONS:

1. REDUCE PARALLELISM:
   nix build -j 1 --cores 2
   
   # In nix.conf:
   max-jobs = 1
   cores = 2

2. ADD SWAP:
   sudo fallocate -l 8G /swapfile
   sudo chmod 600 /swapfile
   sudo mkswap /swapfile
   sudo swapon /swapfile

3. USE BINARY CACHE:
   Large packages usually have cached binaries.
   Check your substituters.

4. CLOSE OTHER APPS:
   Browsers, IDEs use lots of RAM.",
        tip: Some("Try: nix build -j 1 --cores 2"),
    },

    Pattern {
        id: "build-interrupted",
        category: Category::Build,
        regex_str: r"interrupted by the user|build.*interrupted|SIGINT|keyboard interrupt|Ctrl-C",
        title: "Build interrupted",
        explanation: "The build was interrupted (Ctrl+C or signal).",
        solution: "\
# Just run again:
nix build

# Partial builds are safe - Nix will continue",
        deep_dive: "\
WHY THIS HAPPENS:
You pressed Ctrl+C or the process received SIGINT/SIGTERM.

THIS IS FINE:
- Nix builds are atomic
- Partial builds don't corrupt the store
- You can resume anytime

JUST RUN AGAIN:
  nix build
  
Nix will:
- Skip already completed builds
- Resume from where it stopped
- Not redo finished work

BACKGROUND BUILDS:
If you want to run in background:
  nix build &
  # Or use screen/tmux",
        tip: Some("Just run the command again"),
    },

    Pattern {
        id: "config-not-found",
        category: Category::NixOS,
        regex_str: r"configuration\.nix.*not found|No such file.*configuration\.nix|cannot find.*configuration\.nix|/etc/nixos.*not found",
        title: "configuration.nix not found",
        explanation: "NixOS configuration file not found.",
        solution: "\
# Check location:
ls -la /etc/nixos/

# Create if missing:
sudo nixos-generate-config

# Or specify path:
sudo nixos-rebuild switch -I nixos-config=./configuration.nix",
        deep_dive: "\
WHY THIS HAPPENS:
NixOS looks for /etc/nixos/configuration.nix by default.

COMMON CAUSES:
1. Fresh install without config
2. Config moved/deleted
3. Wrong working directory
4. Using flakes but forgot --flake

SOLUTIONS:

1. GENERATE DEFAULT:
   sudo nixos-generate-config
   
2. SPECIFY PATH:
   sudo nixos-rebuild switch -I nixos-config=/path/to/config.nix

3. FOR FLAKES:
   sudo nixos-rebuild switch --flake .#hostname
   # Doesn't use /etc/nixos/configuration.nix

4. CHECK WHAT EXISTS:
   ls -la /etc/nixos/",
        tip: Some("Use --flake for flake-based configs"),
    },

    Pattern {
        id: "flake-lock-not-committed",
        category: Category::Flake,
        regex_str: r"flake\.lock.*not.*commit|lock file.*tracked|flake\.lock.*untracked|add.*flake\.lock.*git",
        title: "flake.lock not committed to git",
        explanation: "The flake.lock file needs to be tracked in git.",
        solution: "\
# Add and commit:
git add flake.lock
git commit -m 'Update flake.lock'",
        deep_dive: "\
WHY THIS HAPPENS:
Flakes require flake.lock to be committed for reproducibility.
Uncommitted lock files are ignored.

SOLUTION:
  git add flake.lock
  git commit -m 'Update flake.lock'

WHY COMMIT LOCK FILES:
- Ensures everyone uses same versions
- Reproducible builds
- CI/CD consistency

UPDATING LOCK:
  nix flake update
  git add flake.lock
  git commit -m 'Update flake inputs'

WORKFLOW:
1. Change flake.nix
2. nix flake lock (or nix flake update)
3. git add flake.nix flake.lock
4. git commit",
        tip: Some("Always commit flake.lock"),
    },

    Pattern {
        id: "evaluation-timeout",
        category: Category::Eval,
        regex_str: r"evaluation.*timed? ?out|eval.*killed|evaluation took too long|stack overflow|maximum call depth exceeded",
        title: "Evaluation timeout/overflow",
        explanation: "Nix evaluation took too long or hit recursion limit.",
        solution: "\
# Usually means infinite loop or very deep recursion
# Check for:
# - Recursive imports
# - Overlays referencing themselves
# - Circular module imports",
        deep_dive: "\
WHY THIS HAPPENS:
1. INFINITE RECURSION:
   Overlay uses final instead of prev
   Module imports itself

2. VERY LARGE EVALUATION:
   Huge package set
   Complex overlays

3. STACK OVERFLOW:
   Too deep recursion
   Circular dependencies

DEBUGGING:
  nix eval --show-trace
  
Look for repeated patterns in the trace.

COMMON FIXES:
1. Overlays: Use prev not final for the package being modified
2. Modules: Check for circular imports
3. Let bindings: Avoid self-reference

INCREASE LIMITS (temporary):
  nix eval --option max-call-depth 10000",
        tip: Some("Usually an infinite loop - check overlays"),
    },

    Pattern {
        id: "binary-cache-miss",
        category: Category::Fetch,
        regex_str: r"cannot find.*in.*cache|no binary.*available|building.*instead of downloading|this path will be fetched|will be built",
        title: "No binary cache hit",
        explanation: "Package not in binary cache - will be built locally.",
        solution: "\
# This is informational, not an error
# To avoid building, use stable nixpkgs:
inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-24.05\";",
        deep_dive: "\
WHY THIS HAPPENS:
Hydra (Nix's CI) only builds packages for stable branches.
If you use:
- nixos-unstable: Most things cached
- master: Often not cached
- Custom overlays: Never cached
- Modified packages: Never cached

SOLUTIONS:

1. USE STABLE BRANCH:
   nixos-24.05 instead of nixos-unstable

2. WAIT:
   Hydra may still be building it.
   Check: https://hydra.nixos.org

3. ACCEPT LOCAL BUILD:
   It's fine, just takes longer.

4. USE CACHIX:
   Add community caches:
   nix.settings.substituters = [
     \"https://nix-community.cachix.org\"
   ];",
        tip: Some("Use stable nixpkgs for better cache hits"),
    },

    Pattern {
        id: "derivation-output-mismatch",
        category: Category::Build,
        regex_str: r"output.*differs|hash mismatch.*output|output hash.*expected|fixed-output.*hash",
        title: "Derivation output mismatch",
        explanation: "Build output doesn't match expected hash.",
        solution: "\
# For fetchurl/fetchzip - update hash:
hash = lib.fakeHash;  # Get correct hash from error

# For packages - upstream may have changed:
# Check if tarball was re-uploaded",
        deep_dive: "\
WHY THIS HAPPENS:
Fixed-output derivations (downloads) must match their declared hash.

CAUSES:
1. Upstream changed the file
2. Wrong hash in package
3. Mirror served different content

FOR YOUR PACKAGES:
1. Use lib.fakeHash temporarily
2. Build - get the correct hash from error
3. Replace with correct hash

FOR NIXPKGS PACKAGES:
1. Update nixpkgs (may be fixed)
2. Report issue on GitHub
3. Override hash in overlay

COMMON WITH:
- GitHub release tarballs (regenerated)
- Unstable URLs
- Rolling release software",
        tip: Some("Use lib.fakeHash to get correct hash"),
    },

    Pattern {
        id: "read-only-store",
        category: Category::Build,
        regex_str: r"[Rr]ead.only file ?system|cannot.*store.*read.only|EROFS|nix store.*read.only",
        title: "Nix store is read-only",
        explanation: "Cannot write to /nix/store - filesystem is read-only.",
        solution: "\
# Check mount:
mount | grep /nix

# Remount if needed:
sudo mount -o remount,rw /nix",
        deep_dive: "\
WHY THIS HAPPENS:
/nix/store is normally read-only (that's good!).
But the daemon needs write access.

CAUSES:
1. Filesystem mounted read-only
2. Disk errors forced read-only
3. Docker/container restrictions
4. Nix daemon not running

SOLUTIONS:

1. CHECK MOUNT:
   mount | grep nix
   
2. REMOUNT:
   sudo mount -o remount,rw /nix

3. CHECK DISK:
   dmesg | tail -50
   # Look for disk errors

4. IN DOCKER:
   Need privileged mode or proper volumes

5. START DAEMON:
   sudo systemctl start nix-daemon",
        tip: Some("Check: mount | grep nix"),
    },

    Pattern {
        id: "generation-switch-failed",
        category: Category::NixOS,
        regex_str: r"failed to switch.*generation|cannot switch.*profile|profile.*switch.*failed|generation.*not found",
        title: "Failed to switch generation",
        explanation: "Could not switch to the specified generation.",
        solution: "\
# List available generations:
sudo nix-env --list-generations -p /nix/var/nix/profiles/system

# Switch to specific one:
sudo nix-env --switch-generation 42 -p /nix/var/nix/profiles/system",
        deep_dive: "\
WHY THIS HAPPENS:
1. Generation was garbage collected
2. Profile corrupted
3. Wrong generation number

SOLUTIONS:

1. LIST GENERATIONS:
   sudo nix-env --list-generations -p /nix/var/nix/profiles/system

2. SWITCH TO EXISTING:
   sudo nix-env --switch-generation <num> -p /nix/var/nix/profiles/system

3. REBUILD INSTEAD:
   sudo nixos-rebuild switch
   # Creates new generation

4. FROM BOOT MENU:
   Reboot and select working generation

PREVENTING:
Don't garbage collect too aggressively:
  nix.gc.options = \"--delete-older-than 14d\";",
        tip: Some("Check available generations first"),
    },

    Pattern {
        id: "module-import-failed",
        category: Category::Eval,
        regex_str: r"error importing.*module|module.*import.*failed|cannot import.*\.nix|while importing",
        title: "Module import failed",
        explanation: "Failed to import a NixOS/Home-Manager module.",
        solution: "\
# Check path exists:
ls -la ./module.nix

# Check syntax:
nix-instantiate --parse ./module.nix",
        deep_dive: "\
WHY THIS HAPPENS:
1. File doesn't exist
2. Syntax error in module
3. Wrong path (relative vs absolute)
4. Module has evaluation error

DEBUGGING:

1. CHECK FILE EXISTS:
   ls -la ./module.nix

2. CHECK SYNTAX:
   nix-instantiate --parse ./module.nix

3. CHECK WITH TRACE:
   nix eval --show-trace

COMMON MISTAKES:
- Relative path from wrong directory
- Forgot .nix extension
- Module has undefined variables
- Circular imports

IN FLAKES:
Paths are relative to flake.nix:
  imports = [ ./modules/mymodule.nix ];",
        tip: Some("Check file exists and has valid syntax"),
    },

    Pattern {
        id: "overlay-infinite-recursion",
        category: Category::Eval,
        regex_str: r"infinite recursion.*overlay|overlay.*infinite|overlay.*final.*final",
        title: "Overlay causes infinite recursion",
        explanation: "Overlay references final (self) instead of prev (super) for modified package.",
        solution: "\
# WRONG:
(final: prev: {
  pkg = final.pkg.override { };  # WRONG!
})

# RIGHT:
(final: prev: {
  pkg = prev.pkg.override { };   # Use prev!
})",
        deep_dive: "\
WHY THIS HAPPENS:
In overlays:
- final (or self) = the RESULT after all overlays
- prev (or super) = BEFORE this overlay

If you modify pkg using final.pkg, you reference your own result!

WRONG:
  (final: prev: {
    hello = final.hello.override { };  # Recursion!
  })

RIGHT:
  (final: prev: {
    hello = prev.hello.override { };   # Uses original
  })

WHEN TO USE final:
- For OTHER packages you're not modifying:
  (final: prev: {
    myPkg = final.callPackage ./pkg.nix { };
    # final.callPackage is fine - we're not modifying callPackage
  })

RULE OF THUMB:
- Modifying X? Use prev.X
- Using unmodified Y? final.Y is fine",
        tip: Some("Use prev for the package being modified"),
    },

    Pattern {
        id: "nix-path-empty",
        category: Category::Eval,
        regex_str: r"NIX_PATH.*empty|NIX_PATH.*not set|cannot look up.*angle brackets",
        title: "NIX_PATH not set",
        explanation: "<nixpkgs> lookup failed because NIX_PATH is empty.",
        solution: "\
# Set NIX_PATH:
export NIX_PATH=nixpkgs=channel:nixos-unstable

# Or better - use flakes:
nix build nixpkgs#hello",
        deep_dive: "\
WHY THIS HAPPENS:
<nixpkgs> is the old way to reference nixpkgs.
It requires NIX_PATH to be set.

SOLUTIONS:

1. SET NIX_PATH:
   export NIX_PATH=nixpkgs=channel:nixos-unstable
   # Or in shell config

2. USE CHANNELS:
   nix-channel --add https://nixos.org/channels/nixos-unstable nixpkgs
   nix-channel --update

3. USE FLAKES (RECOMMENDED):
   Instead of: nix-build '<nixpkgs>' -A hello
   Use: nix build nixpkgs#hello

4. EXPLICIT PATH:
   nix-build -I nixpkgs=/path/to/nixpkgs -A hello

NIXOS:
NIX_PATH is usually set automatically.
Check /etc/nix/nix.conf.",
        tip: Some("Consider using flakes instead"),
    },

    // =========================================================================
    // EXTREMELY COMMON BEGINNER/DAILY ERRORS
    // =========================================================================
    Pattern {
        id: "nix-command-not-found",
        category: Category::Build,
        regex_str: r"(bash|sh|zsh|fish):.*nix.*([Cc]ommand )?not found|nix: No such file|cannot find.*nix|nix:.*not found|nix.*command not found",
        title: "Nix command not found",
        explanation: "Nix is not installed or not in your PATH.",
        solution: "\
# Install Nix:
sh <(curl -L https://nixos.org/nix/install) --daemon

# Or add to PATH:
source ~/.nix-profile/etc/profile.d/nix.sh",
        deep_dive: "\
WHY THIS HAPPENS:
1. Nix not installed
2. Shell not configured (PATH missing)
3. New terminal after install (needs source)

SOLUTIONS:

1. INSTALL NIX:
   sh <(curl -L https://nixos.org/nix/install) --daemon

2. SOURCE PROFILE:
   # bash/zsh:
   source ~/.nix-profile/etc/profile.d/nix.sh
   
   # Or add to .bashrc/.zshrc:
   if [ -e ~/.nix-profile/etc/profile.d/nix.sh ]; then
     . ~/.nix-profile/etc/profile.d/nix.sh
   fi

3. RESTART SHELL:
   exec $SHELL

4. CHECK INSTALLATION:
   ls ~/.nix-profile/bin/nix",
        tip: Some("New terminal? Run: source ~/.nix-profile/etc/profile.d/nix.sh"),
    },

    Pattern {
        id: "not-a-derivation",
        category: Category::Eval,
        regex_str: r"is not a derivation|expected a derivation|value is.*but a derivation was expected|cannot coerce.*to a derivation",
        title: "Value is not a derivation",
        explanation: "Expected a package/derivation but got something else.",
        solution: "\
# Check what you're referencing:
nix repl
> :t pkgs.hello  # Should show 'derivation'

# Common fix - maybe it's a function:
pkgs.callPackage ./pkg.nix { }",
        deep_dive: "\
WHY THIS HAPPENS:
You used something that isn't a package where a package was expected.

COMMON CAUSES:

1. IT'S A FUNCTION (needs calling):
   # WRONG:
   environment.systemPackages = [ ./my-pkg.nix ];
   
   # RIGHT:
   environment.systemPackages = [ (pkgs.callPackage ./my-pkg.nix {}) ];

2. IT'S A SET (need to pick attribute):
   # WRONG:
   pkgs.python3Packages  # This is a set, not a package!
   
   # RIGHT:
   pkgs.python3Packages.numpy

3. IT'S NULL:
   Check if the package exists:
   nix eval nixpkgs#mypackage

4. WRONG PATH:
   pkgs.python3.pkgs.numpy  # Different from
   pkgs.python3Packages.numpy

CHECK TYPE:
  nix repl -f '<nixpkgs>'
  nix-repl> builtins.typeOf pkgs.hello
  \"set\"  # Derivations are sets with special attributes",
        tip: Some("Use 'nix repl' to explore what you have"),
    },

    Pattern {
        id: "override-not-available",
        category: Category::Eval,
        regex_str: r#"attribute ['`"]override['`"].*missing|does not have.*override|override.*not.*function|cannot override"#,
        title: "Package doesn't support .override",
        explanation: "This package doesn't have .override or .overrideAttrs.",
        solution: "\
# Use overrideAttrs instead:
pkg.overrideAttrs (old: {
  patches = old.patches or [] ++ [ ./fix.patch ];
})

# Or wrap with callPackage for override:
(pkgs.callPackage ./pkg.nix { }).override { }",
        deep_dive: "\
WHY THIS HAPPENS:
Not all derivations have .override. It's added by callPackage.

OVERRIDE METHODS:

1. overrideAttrs (almost always works):
   pkgs.hello.overrideAttrs (old: {
     version = \"2.0\";
   })

2. override (only callPackage'd):
   pkgs.hello.override {
     stdenv = pkgs.clangStdenv;
   }

3. overrideDerivation (legacy, avoid):
   pkgs.lib.overrideDerivation pkgs.hello (old: { })

WHEN TO USE WHICH:
- Change dependencies -> .override
- Change build attributes -> .overrideAttrs
- Both -> chain them

EXAMPLE:
  pkgs.hello.override {
    stdenv = pkgs.clangStdenv;
  }.overrideAttrs (old: {
    patches = [ ./fix.patch ];
  })",
        tip: Some("overrideAttrs works on almost everything"),
    },

    Pattern {
        id: "git-not-a-repository",
        category: Category::Flake,
        regex_str: r"[Nn]ot a git repository|does not appear to be a git repo|fatal:.*git.*repository|cannot find.*\.git",
        title: "Not a git repository",
        explanation: "Flakes require the directory to be a git repository.",
        solution: "\
# Initialize git:
git init
git add flake.nix flake.lock
git commit -m 'Initial commit'",
        deep_dive: "\
WHY THIS HAPPENS:
Flakes track files via git. Without git, Nix doesn't know which 
files belong to the flake.

SOLUTIONS:

1. INITIALIZE GIT:
   git init
   git add -A
   git commit -m 'Initial commit'

2. USE path: PREFIX (not recommended):
   nix build path:.#package
   # This ignores git, but loses reproducibility

3. CHECK YOU'RE IN RIGHT DIR:
   pwd
   ls -la .git

IMPORTANT:
- New files must be 'git add'ed to be seen!
- Uncommitted changes may be ignored
- .gitignore'd files are invisible to flakes

COMMON MISTAKE:
  cd /tmp  # No git repo here!
  nix build /path/to/flake#pkg  # Still needs git in /path/to/flake",
        tip: Some("Run 'git init' and commit your files"),
    },

    Pattern {
        id: "git-ref-not-found",
        category: Category::Flake,
        regex_str: r"[Rr]eference.*not found|cannot find ref|rev.*does not exist|could not resolve.*ref|branch.*not found",
        title: "Git reference not found",
        explanation: "The specified git branch, tag, or commit doesn't exist.",
        solution: "\
# Check available refs:
git ls-remote <repo>

# Use correct ref in flake.nix:
inputs.foo.url = \"github:owner/repo/main\";  # Not master!",
        deep_dive: "\
WHY THIS HAPPENS:
The branch, tag, or commit you specified doesn't exist.

COMMON MISTAKES:
1. 'master' vs 'main' - many repos switched!
2. Typo in branch name
3. Tag doesn't exist yet
4. Private repo without auth

SOLUTIONS:

1. CHECK REFS:
   git ls-remote https://github.com/owner/repo
   
2. USE CORRECT BRANCH:
   # Many repos are now 'main' not 'master':
   inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";

3. PIN TO COMMIT:
   inputs.foo.url = \"github:owner/repo/abc1234\";

4. CHECK IF PRIVATE:
   Use git+ssh:// for private repos:
   inputs.private.url = \"git+ssh://git@github.com/owner/private\";

LIST BRANCHES:
  git ls-remote --heads https://github.com/owner/repo",
        tip: Some("'master' is often 'main' now"),
    },

    Pattern {
        id: "not-a-shell-derivation",
        category: Category::Build,
        regex_str: r"is not a shell derivation|not a valid shell|expected.*shell|does not provide.*devShell|cannot be used as a shell",
        title: "Not a shell derivation",
        explanation: "You ran 'nix develop' on a package instead of a devShell.",
        solution: "\
# For packages, use nix shell instead:
nix shell nixpkgs#hello

# For development, create a devShell:
devShells.default = mkShell {
  packages = [ gcc cmake ];
};",
        deep_dive: "\
WHY THIS HAPPENS:
'nix develop' needs a devShell, not a regular package.

COMMANDS:
- nix develop -> Uses devShells.<system>.default (for development)
- nix shell -> Puts package in PATH (for running)
- nix build -> Builds package (for building)

SOLUTIONS:

1. USE nix shell FOR PACKAGES:
   nix shell nixpkgs#hello  # Run hello
   nix shell nixpkgs#python3 --command python

2. CREATE A DEVSHELL:
   # In flake.nix:
   devShells.x86_64-linux.default = pkgs.mkShell {
     packages = [ pkgs.gcc pkgs.cmake ];
   };

3. USE inputsFrom FOR PACKAGE DEV:
   devShells.default = pkgs.mkShell {
     inputsFrom = [ self.packages.default ];
   };

COMMON MISTAKE:
  nix develop nixpkgs#hello  # WRONG - hello is not a shell
  nix shell nixpkgs#hello    # RIGHT",
        tip: Some("Use 'nix shell' for packages, 'nix develop' for devShells"),
    },

    Pattern {
        id: "sqlite-database-locked",
        category: Category::Build,
        regex_str: r"database is locked|[Ss][Qq][Ll]ite.*locked|[Ss][Qq][Ll]ite.*busy|cannot.*database.*lock|database.*unavailable",
        title: "Nix database is locked",
        explanation: "Another Nix process is using the database.",
        solution: "\
# Find other Nix processes:
ps aux | grep nix

# Wait for them to finish, or kill if stuck:
sudo pkill -9 nix",
        deep_dive: "\
WHY THIS HAPPENS:
Only one process can write to the Nix database at a time.

COMMON CAUSES:
1. Another nix build running
2. nix-collect-garbage running  
3. Crashed nix process
4. nix-daemon stuck

SOLUTIONS:

1. FIND PROCESSES:
   ps aux | grep nix
   pgrep -a nix

2. WAIT:
   Just wait for the other build to finish.

3. KILL STUCK PROCESS:
   sudo pkill nix-daemon
   sudo systemctl restart nix-daemon

4. REMOVE STALE LOCKS:
   # Only if sure nothing is running!
   sudo rm /nix/var/nix/db/db.lock

5. CHECK DAEMON:
   systemctl status nix-daemon
   sudo journalctl -u nix-daemon

PREVENTION:
Don't run multiple nix-build in parallel on same store.",
        tip: Some("Wait for other Nix processes to finish"),
    },

    Pattern {
        id: "mkderivation-missing-name",
        category: Category::Eval,
        regex_str: r#"called without.*['"]?(pname|name)['"]|mkDerivation.*requires.*(name|pname)|missing.*required.*(name|pname)"#,
        title: "mkDerivation requires name/pname",
        explanation: "stdenv.mkDerivation needs either 'name' or 'pname' + 'version'.",
        solution: "\
# Option 1 - pname + version (recommended):
stdenv.mkDerivation {
  pname = \"my-package\";
  version = \"1.0.0\";
  ...
}

# Option 2 - name directly:
stdenv.mkDerivation {
  name = \"my-package-1.0.0\";
  ...
}",
        deep_dive: "\
WHY THIS HAPPENS:
Every derivation needs a name. mkDerivation requires you to specify it.

TWO OPTIONS:

1. pname + version (RECOMMENDED):
   stdenv.mkDerivation {
     pname = \"hello\";
     version = \"2.10\";
     # name becomes \"hello-2.10\"
   }

2. name directly:
   stdenv.mkDerivation {
     name = \"hello-2.10\";
   }

WHY pname + version IS BETTER:
- Enables version-based overrides
- Cleaner separation
- Nixpkgs convention

COMMON MISTAKE:
  stdenv.mkDerivation {
    # Forgot name!
    src = ./. ;
    buildInputs = [ ... ];
  }

FOR BUILDPYTHONPACKAGE:
  buildPythonPackage {
    pname = \"mypackage\";
    version = \"0.1.0\";
    # ...
  }",
        tip: Some("Use pname + version, not just name"),
    },

    Pattern {
        id: "nvidia-driver-mismatch",
        category: Category::NixOS,
        regex_str: r"NVIDIA.*version mismatch|nvidia.*driver.*mismatch|NVRM.*mismatch|kernel module.*nvidia|nvidia.*module.*version",
        title: "NVIDIA driver version mismatch",
        explanation: "Kernel module version doesn't match userspace driver.",
        solution: "\
# Reboot after nixos-rebuild:
sudo nixos-rebuild switch
sudo reboot

# Or reload modules:
sudo rmmod nvidia_uvm nvidia_drm nvidia_modeset nvidia
sudo modprobe nvidia",
        deep_dive: "\
WHY THIS HAPPENS:
After updating, the kernel module is old but userspace is new.
They must match exactly.

SOLUTIONS:

1. REBOOT (easiest):
   sudo nixos-rebuild switch
   sudo reboot

2. RELOAD MODULES (if X not running):
   sudo systemctl stop display-manager
   sudo rmmod nvidia_uvm nvidia_drm nvidia_modeset nvidia
   sudo modprobe nvidia
   sudo systemctl start display-manager

3. CHECK VERSIONS:
   cat /proc/driver/nvidia/version
   nvidia-smi

NIXOS CONFIG:
  # Use latest drivers:
  hardware.nvidia.package = config.boot.kernelPackages.nvidiaPackages.stable;
  
  # Or specific version:
  hardware.nvidia.package = config.boot.kernelPackages.nvidiaPackages.legacy_470;

COMMON ISSUES:
- Didn't reboot after rebuild
- Module in initrd but not updated
- Secure boot blocking new module",
        tip: Some("Reboot after updating NVIDIA drivers"),
    },

    Pattern {
        id: "overlays-wrong-format",
        category: Category::Eval,
        regex_str: r"overlays.*must be.*list|overlay.*not.*function|expected.*overlay|overlays.*function.*but|cannot apply overlay",
        title: "Overlays must be a list of functions",
        explanation: "Overlays must be a list of (final: prev: {...}) functions.",
        solution: "\
# WRONG:
nixpkgs.overlays = (final: prev: { });

# RIGHT:
nixpkgs.overlays = [
  (final: prev: { myPkg = ...; })
];",
        deep_dive: "\
WHY THIS HAPPENS:
Overlays must be a LIST of functions, not a single function.

CORRECT FORMAT:
  nixpkgs.overlays = [
    # Each overlay is a function:
    (final: prev: {
      myPackage = prev.hello;
    })
    
    # Can have multiple:
    (final: prev: {
      anotherPkg = prev.world;
    })
    
    # Can import from file:
    (import ./my-overlay.nix)
  ];

WRONG FORMATS:
  # Just a function (not a list):
  nixpkgs.overlays = (final: prev: { });
  
  # Set instead of function:
  nixpkgs.overlays = [{ myPkg = pkgs.hello; }];

IN OVERLAY FILE (./overlay.nix):
  # File should contain:
  final: prev: {
    myPkg = ...;
  }
  
  # Used as:
  nixpkgs.overlays = [ (import ./overlay.nix) ];",
        tip: Some("Overlays = [ (final: prev: {...}) ]"),
    },

    Pattern {
        id: "modules-wrong-format",
        category: Category::Eval,
        regex_str: r"modules.*must be.*list|expected.*module|module.*not.*valid|imports.*must be.*list|cannot import.*module",
        title: "Modules must be a list",
        explanation: "The 'modules' or 'imports' option expects a list.",
        solution: "\
# WRONG:
modules = ./module.nix;

# RIGHT:
modules = [ ./module.nix ];

# Multiple:
modules = [
  ./hardware.nix
  ./users.nix
];",
        deep_dive: "\
WHY THIS HAPPENS:
NixOS modules and imports must be lists, even for single items.

CORRECT FORMATS:

1. NIXOS SYSTEM:
   nixpkgs.lib.nixosSystem {
     modules = [
       ./configuration.nix
       ./hardware-configuration.nix
     ];
   }

2. IMPORTS IN MODULE:
   { config, pkgs, ... }: {
     imports = [
       ./other-module.nix
       inputs.home-manager.nixosModules.home-manager
     ];
   }

3. HOME-MANAGER:
   home-manager.lib.homeManagerConfiguration {
     modules = [
       ./home.nix
     ];
   }

WRONG:
  modules = ./configuration.nix;  # Not a list!
  imports = ./module.nix;         # Not a list!

MIXING TYPES IS OK:
  imports = [
    ./local.nix                    # Path
    inputs.foo.nixosModules.bar    # Attrset
    ({ pkgs, ... }: { })           # Inline function
  ];",
        tip: Some("Always use [ ] even for single module"),
    },

    Pattern {
        id: "specialisation-not-found",
        category: Category::NixOS,
        regex_str: r"specialisation.*not found|specialization.*does not exist|unknown specialisation|no.*specialisation",
        title: "Specialisation not found",
        explanation: "The specified NixOS specialisation doesn't exist.",
        solution: "\
# List available specialisations:
ls /nix/var/nix/profiles/system/specialisation/

# Define in configuration.nix:
specialisation.gaming.configuration = {
  services.xserver.enable = true;
};",
        deep_dive: "\
WHY THIS HAPPENS:
You tried to switch to a specialisation that doesn't exist.

WHAT ARE SPECIALISATIONS:
Alternative system configurations that share the base system.
Useful for: gaming mode, work mode, minimal mode.

DEFINING:
  specialisation.gaming.configuration = {
    hardware.nvidia.prime.offload.enable = false;
    hardware.nvidia.prime.sync.enable = true;
  };
  
  specialisation.work.configuration = {
    services.openvpn.enable = true;
  };

SWITCHING:
  # At boot: Select from bootloader menu
  
  # At runtime:
  sudo /run/current-system/specialisation/gaming/bin/switch-to-configuration switch

LISTING:
  ls /nix/var/nix/profiles/system/specialisation/
  
COMMON MISTAKE:
Typo in specialisation name, or didn't rebuild after adding it.",
        tip: Some("Check spelling and rebuild first"),
    },

    Pattern {
        id: "fetchgit-requires-hash",
        category: Category::Fetch,
        regex_str: r"fetchGit.*requires.*hash|hash.*required.*pure|cannot fetch.*without hash|fetchTree.*requires",
        title: "fetchGit requires hash in pure mode",
        explanation: "Pure evaluation requires hashes for fetches.",
        solution: "\
# Add hash:
src = fetchGit {
  url = \"https://...\";
  rev = \"abc123\";
  hash = \"sha256-...\";  # or sha256 = \"...\";
};

# Or use fetchFromGitHub:
src = fetchFromGitHub {
  owner = \"...\";
  repo = \"...\";
  rev = \"...\";
  hash = \"sha256-...\";
};",
        deep_dive: "\
WHY THIS HAPPENS:
In pure evaluation mode (flakes default), all fetches need hashes
for reproducibility.

SOLUTIONS:

1. ADD HASH:
   src = fetchGit {
     url = \"https://github.com/owner/repo\";
     rev = \"abc123def\";
     hash = \"sha256-AAAA...\";
   };

2. GET THE HASH:
   nix-prefetch-git https://github.com/owner/repo --rev abc123
   
   # Or use fakeHash:
   hash = lib.fakeHash;
   # Build will fail and show correct hash

3. USE fetchFromGitHub (easier):
   src = fetchFromGitHub {
     owner = \"NixOS\";
     repo = \"nixpkgs\";
     rev = \"abc123\";
     hash = \"sha256-...\";
   };

4. IMPURE MODE (not recommended):
   nix build --impure

FOR FLAKE INPUTS:
Inputs don't need hashes - flake.lock handles that.",
        tip: Some("Use lib.fakeHash to get correct hash"),
    },

    Pattern {
        id: "home-manager-version-mismatch",
        category: Category::NixOS,
        regex_str: r"home.manager.*version|home-manager.*mismatch|HM.*incompatible|home-manager.*nixpkgs.*version",
        title: "Home-Manager/nixpkgs version mismatch",
        explanation: "Home-Manager version doesn't match nixpkgs version.",
        solution: "\
# Use matching branches:
inputs = {
  nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";
  home-manager = {
    url = \"github:nix-community/home-manager\";
    inputs.nixpkgs.follows = \"nixpkgs\";  # Important!
  };
};",
        deep_dive: "\
WHY THIS HAPPENS:
Home-Manager releases track nixpkgs releases. Mixing versions causes issues.

MATCHING VERSIONS:
  nixos-24.05   -> home-manager release-24.05
  nixos-unstable -> home-manager master

CORRECT FLAKE SETUP:
  inputs = {
    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";
    
    home-manager = {
      url = \"github:nix-community/home-manager\";
      inputs.nixpkgs.follows = \"nixpkgs\";  # IMPORTANT!
    };
  };

THE 'follows' IS CRUCIAL:
Without it, home-manager uses its own nixpkgs, causing:
- Version mismatches
- Duplicate packages
- Evaluation errors

STABLE SETUP:
  inputs = {
    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-24.05\";
    home-manager.url = \"github:nix-community/home-manager/release-24.05\";
    home-manager.inputs.nixpkgs.follows = \"nixpkgs\";
  };",
        tip: Some("Always use 'inputs.nixpkgs.follows'"),
    },

    Pattern {
        id: "boot-read-only-filesystem",
        category: Category::NixOS,
        regex_str: r"/boot.*[Rr]ead.only|cannot write.*boot|boot.*partition.*full|No space.*boot|EFI.*read.only",
        title: "Cannot write to /boot",
        explanation: "/boot is read-only or full.",
        solution: "\
# Check if mounted:
mount | grep boot

# Check space:
df -h /boot

# Remove old generations to free space:
sudo nix-collect-garbage -d
sudo nixos-rebuild boot",
        deep_dive: "\
WHY THIS HAPPENS:
1. /boot not mounted
2. /boot is full (common with small EFI partition)
3. Read-only mount

SOLUTIONS:

1. MOUNT /boot:
   sudo mount /boot
   # Or check /etc/fstab

2. FREE SPACE:
   # List old kernels:
   ls /boot/
   
   # Remove old generations:
   sudo nix-collect-garbage -d
   sudo nixos-rebuild boot
   
   # Manual cleanup:
   sudo rm /boot/EFI/nixos/OLD_ENTRIES

3. CHECK FSTAB:
   cat /etc/fstab | grep boot
   
4. REMOUNT RW:
   sudo mount -o remount,rw /boot

SMALL EFI PARTITION:
If /boot is always filling up:
  boot.loader.systemd-boot.configurationLimit = 10;
  # Or use GRUB with separate /boot

CHECK SPACE:
  df -h /boot
  du -sh /boot/*",
        tip: Some("Remove old generations: nix-collect-garbage -d"),
    },

    Pattern {
        id: "environment-variable-not-set",
        category: Category::Build,
        regex_str: r"environment variable.*not set|variable.*undefined|getenv.*failed|\$[A-Z_]+.*not set|required.*env.*missing",
        title: "Required environment variable not set",
        explanation: "A required environment variable is missing.",
        solution: "\
# Set in shell:
export MY_VAR=\"value\"

# In Nix derivation:
MY_VAR = \"value\";

# Or preBuild:
preBuild = ''export MY_VAR=value'';",
        deep_dive: "\
WHY THIS HAPPENS:
Nix builds run in clean environments. Variables from your shell 
aren't available unless explicitly passed.

SOLUTIONS:

1. IN DERIVATION:
   stdenv.mkDerivation {
     MY_VAR = \"value\";
     # Available as $MY_VAR in build
   }

2. IN SHELL HOOK:
   mkShell {
     shellHook = ''
       export MY_VAR=\"value\"
     '';
   }

3. PASS FROM OUTSIDE (impure):
   nix build --impure
   # Then builtins.getEnv works

4. USE makeWrapper:
   postInstall = ''
     wrapProgram $out/bin/myapp \\
       --set MY_VAR \"value\"
   '';

COMMON VARIABLES:
- HOME: Usually set to /homeless-shelter or $TMPDIR
- USER: Set to nixbld
- PATH: Only contains build inputs

FOR API KEYS ETC:
Don't hardcode! Use runtime config or secrets management.",
        tip: Some("Nix builds have clean environments"),
    },

    Pattern {
        id: "lib-not-found-runtime",
        category: Category::Build,
        regex_str: r"error while loading shared libraries|cannot open shared object|libGL|libvulkan|ELF.*interpreter.*not found",
        title: "Shared library not found at runtime",
        explanation: "Program can't find required .so library at runtime.",
        solution: "\
# Wrap with library path:
postInstall = ''
  wrapProgram $out/bin/app \\
    --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ libGL vulkan-loader ]}
'';

# Or use autoPatchelfHook:
nativeBuildInputs = [ autoPatchelfHook ];
buildInputs = [ libGL ];",
        deep_dive: "\
WHY THIS HAPPENS:
Binary was compiled expecting libraries in standard paths (/usr/lib),
but Nix puts them in /nix/store.

SOLUTIONS:

1. autoPatchelfHook (best for binaries):
   nativeBuildInputs = [ autoPatchelfHook ];
   buildInputs = [ stdenv.cc.cc.lib libGL ];

2. makeWrapper:
   nativeBuildInputs = [ makeWrapper ];
   postInstall = ''
     wrapProgram $out/bin/app \\
       --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [
         libGL
         vulkan-loader
       ]}
   '';

3. patchelf directly:
   patchelf --set-interpreter ${glibc}/lib/ld-linux-x86-64.so.2 $out/bin/app
   patchelf --set-rpath ${lib.makeLibraryPath [ ... ]} $out/bin/app

COMMON LIBRARIES:
- libGL.so -> libGL, libglvnd
- libvulkan.so -> vulkan-loader
- libstdc++.so -> stdenv.cc.cc.lib

FOR STEAM/GAMES:
Use steam-run or buildFHSUserEnv.",
        tip: Some("Use autoPatchelfHook for prebuilt binaries"),
    },

    Pattern {
        id: "flake-private-repo",
        category: Category::Flake,
        regex_str: r"Repository not found|Authentication failed|could not read Username|HTTP.*403|HTTP.*401|access denied.*repository",
        title: "Cannot access private repository",
        explanation: "Git authentication failed for private repository.",
        solution: "\
# Use SSH URL:
inputs.private.url = \"git+ssh://git@github.com/owner/repo\";

# Make sure SSH key is loaded:
ssh-add ~/.ssh/id_ed25519",
        deep_dive: "\
WHY THIS HAPPENS:
HTTPS URLs can't authenticate. Use SSH for private repos.

SOLUTIONS:

1. USE SSH URL:
   inputs.private.url = \"git+ssh://git@github.com/owner/repo\";
   # Not: github:owner/repo (uses HTTPS)

2. LOAD SSH KEY:
   eval $(ssh-agent)
   ssh-add ~/.ssh/id_ed25519
   
   # Test:
   ssh -T git@github.com

3. FOR GITLAB:
   inputs.private.url = \"git+ssh://git@gitlab.com/owner/repo\";

4. ACCESS TOKEN (alternative):
   # In ~/.config/nix/nix.conf:
   access-tokens = github.com=ghp_xxxx

COMMON ISSUES:
- SSH key not added to agent
- Wrong key for this repo
- Key not added to GitHub/GitLab
- SSH config wrong

TEST SSH:
  ssh -T git@github.com
  # Should say 'Hi username!'",
        tip: Some("Use git+ssh:// for private repos"),
    },

    // =========================================================================
    // ADDITIONAL COMMON ERRORS
    // =========================================================================
    Pattern {
        id: "cannot-unpack-archive",
        category: Category::Fetch,
        regex_str: r"cannot unpack.*archive|failed to unpack|unpack.*failed|tar.*error|cannot extract",
        title: "Cannot unpack source archive",
        explanation: "Failed to extract the downloaded archive.",
        solution: "\
# Check if archive is corrupted:
nix-prefetch-url --unpack <url>

# Or specify unpack method:
src = fetchzip { ... };  # For zip
src = fetchurl { ... };  # For tar.gz",
        deep_dive: "\
WHY THIS HAPPENS:
1. Corrupted download
2. Wrong archive format
3. Archive type mismatch (zip vs tar)
4. URL doesn't point to archive

SOLUTIONS:

1. RE-DOWNLOAD:
   Delete and fetch again:
   nix-store --delete /nix/store/<hash>...

2. CHECK FORMAT:
   fetchurl -> expects tar.gz by default
   fetchzip -> for zip files
   
3. VERIFY URL:
   curl -L <url> | file -
   # Should show archive type

4. USE CORRECT FETCHER:
   fetchzip {
     url = \"...\";
     hash = \"sha256-...\";
   }
   
   fetchurl {
     url = \"...\";
     hash = \"sha256-...\";
   }",
        tip: Some("Use fetchzip for .zip files"),
    },

    Pattern {
        id: "file-not-found-store",
        category: Category::Build,
        regex_str: r#"file ['`"]([^'`"]+)['`"] was not found in the Nix store|path.*not found in store|store path.*missing"#,
        title: "File not found in Nix store: $1",
        explanation: "The referenced file doesn't exist in the Nix store.",
        solution: "\
# Rebuild the path:
nix-store --realise /nix/store/<path>

# Or rebuild your derivation:
nix build --rebuild",
        deep_dive: "\
WHY THIS HAPPENS:
1. Path was garbage collected
2. Build was interrupted
3. Store corruption
4. Path from another machine

SOLUTIONS:

1. REBUILD:
   nix build --rebuild

2. REALISE PATH:
   nix-store --realise <path>

3. CHECK SUBSTITUTERS:
   Path might be in binary cache:
   nix-store --realise <path> --option substituters 'https://cache.nixos.org'

4. VERIFY STORE:
   nix-store --verify --check-contents",
        tip: Some("Try: nix build --rebuild"),
    },

    Pattern {
        id: "file-not-found-stat",
        category: Category::Eval,
        regex_str: r#"getting status of ['`"]([^'`"]+)['`"].*[Nn]o such file|stat.*[Nn]o such file|[Nn]o such file.*['`"]([^'`"]+)['`"]|cannot access ['`"]([^'`"]+)['`"]"#,
        title: "File not found: $1",
        explanation: "The file or directory doesn't exist.",
        solution: "\
# Check path exists:
ls -la <path>

# For Nix files, use relative path:
./myfile.nix  # Not /absolute/path",
        deep_dive: "\
WHY THIS HAPPENS:
1. Typo in path
2. File was deleted/moved
3. Absolute path in flake (forbidden)
4. Not committed to git (for flakes)

SOLUTIONS:

1. CHECK PATH:
   ls -la <path>
   
2. USE RELATIVE PATHS:
   ./module.nix  # Good
   /home/user/module.nix  # Bad in flakes

3. COMMIT TO GIT:
   git add <file>
   # Uncommitted files invisible to flakes!

4. CHECK WORKING DIRECTORY:
   pwd
   # Make sure you're in the right place",
        tip: Some("Use relative paths in flakes"),
    },

    Pattern {
        id: "unrecognised-cli-option",
        category: Category::Build,
        regex_str: r"unrecogni[sz]ed.*option|unknown option|invalid option|unrecogni[sz]ed flag|no such option",
        title: "Unrecognised command-line option",
        explanation: "The command-line option doesn't exist.",
        solution: "\
# Check available options:
nix build --help
nix --help

# Common new vs old CLI:
# Old: nix-build -A hello
# New: nix build .#hello",
        deep_dive: "\
WHY THIS HAPPENS:
1. Typo in option name
2. Old CLI vs new CLI syntax
3. Option removed in newer version
4. Missing experimental features

OLD vs NEW NIX CLI:
  # Old:
  nix-build -A package
  nix-shell -p package
  
  # New (needs experimental features):
  nix build .#package
  nix shell nixpkgs#package

ENABLE NEW CLI:
  # In ~/.config/nix/nix.conf:
  experimental-features = nix-command flakes

COMMON MISTAKES:
  nix build -A hello  # WRONG (old syntax)
  nix build .#hello   # RIGHT (new syntax)
  
  nix-build .#hello   # WRONG (new syntax)
  nix-build -A hello  # RIGHT (old syntax)",
        tip: Some("Check: nix <command> --help"),
    },

    Pattern {
        id: "option-wrong-type",
        category: Category::NixOS,
        regex_str: r"[Tt]he option.*value.*is not of type|option.*type mismatch|expected type.*but got|value.*does not match.*type",
        title: "Option value has wrong type",
        explanation: "The NixOS option value doesn't match the expected type.",
        solution: "\
# Check expected type:
nixos-option <option>

# Common fixes:
enable = true;           # bool, not \"true\"
port = 8080;             # int, not \"8080\"
packages = [ pkg ];      # list, not single",
        deep_dive: "\
WHY THIS HAPPENS:
NixOS options have strict types. Wrong type = error.

COMMON TYPE ERRORS:

1. STRING instead of BOOL:
   # WRONG:
   services.nginx.enable = \"true\";
   # RIGHT:
   services.nginx.enable = true;

2. STRING instead of INT:
   # WRONG:
   services.nginx.port = \"80\";
   # RIGHT:
   services.nginx.port = 80;

3. SINGLE instead of LIST:
   # WRONG:
   environment.systemPackages = pkgs.git;
   # RIGHT:
   environment.systemPackages = [ pkgs.git ];

4. WRONG SUBMODULE:
   # Check the option definition for expected structure

FINDING EXPECTED TYPE:
  nixos-option services.nginx.enable
  # Or: https://search.nixos.org/options",
        tip: Some("Check option type at search.nixos.org"),
    },

    Pattern {
        id: "while-evaluating",
        category: Category::Eval,
        regex_str: r#"while evaluating the attribute ['`"]([^'`"]+)['`"]|while evaluating ['`"]([^'`"]+)['`"]|while calling ['`"]([^'`"]+)['`"]"#,
        title: "Error while evaluating '$1'",
        explanation: "An error occurred while evaluating '$1'. Check the full trace.",
        solution: "\
# Get full stack trace:
nix build --show-trace

# The actual error is usually below this line",
        deep_dive: "\
UNDERSTANDING THIS ERROR:
This line tells you WHERE the error happened, not WHAT the error is.
The actual error message comes after this.

HOW TO READ:
  error: while evaluating the attribute 'packages.x86_64-linux'
  error: while calling 'mkDerivation'
  error: attribute 'src' missing   <- THE ACTUAL ERROR

DEBUGGING:
1. Read the FULL error output
2. Use --show-trace for complete stack
3. The deepest 'while...' is closest to the problem

COMMON PATTERNS:
- while evaluating 'config' -> Module problem
- while calling 'mkDerivation' -> Package derivation issue
- while evaluating 'packages' -> Flake output problem

TIPS:
- Start from the bottom of the trace
- The first non-'while' line is usually the real error",
        tip: Some("Use --show-trace, read from bottom up"),
    },

    Pattern {
        id: "derivation-call-error",
        category: Category::Eval,
        regex_str: r#"while calling the ['`"]derivation['`"] builtin|derivation.*missing required|error in derivation"#,
        title: "Error in derivation call",
        explanation: "Something went wrong when creating the derivation.",
        solution: "\
# Check required attributes:
# mkDerivation needs: name (or pname+version), src or phases

stdenv.mkDerivation {
  pname = \"mypackage\";
  version = \"1.0\";
  src = ./. ;
}",
        deep_dive: "\
WHY THIS HAPPENS:
The derivation builtin requires specific attributes.

REQUIRED FOR mkDerivation:
- name OR (pname + version)
- src OR custom phases

COMMON MISTAKES:
1. Missing name:
   mkDerivation { src = ./.; }  # No name!

2. Invalid src:
   mkDerivation { name = \"x\"; src = \"./\"; }  # String not path!

3. Wrong type:
   mkDerivation { name = \"x\"; buildInputs = pkg; }  # Not list!

CHECKING:
  nix eval --show-trace
  # Will show which attribute is problematic

MINIMAL EXAMPLE:
  stdenv.mkDerivation {
    pname = \"hello\";
    version = \"1.0\";
    src = ./.;
    installPhase = ''mkdir -p $out'';
  }",
        tip: Some("Check: name/pname, src, buildInputs types"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linker_pattern_matches() {
        let pattern = &PATTERNS[0];
        let re = pattern.regex();
        assert!(re.is_match("ld: cannot find -lssl"));
        assert!(re.is_match("collect2: cannot find -lz"));
    }

    #[test]
    fn test_library_mapping() {
        assert_eq!(library_to_package("ssl"), Some("openssl"));
        assert_eq!(library_to_package("z"), Some("zlib"));
        assert_eq!(library_to_package("unknown"), None);
    }

    #[test]
    fn test_python_module_pattern() {
        let pattern = PATTERNS.iter().find(|p| p.id == "python-module-not-found").unwrap();
        let re = pattern.regex();
        assert!(re.is_match("ModuleNotFoundError: No module named 'numpy'"));
        assert!(re.is_match("ModuleNotFoundError: No module named \"requests\""));
    }

    #[test]
    fn test_experimental_features_pattern() {
        let pattern = PATTERNS.iter().find(|p| p.id == "experimental-features").unwrap();
        let re = pattern.regex();
        assert!(re.is_match("error: experimental Nix feature 'flakes' is disabled"));
        assert!(re.is_match("experimental Nix feature 'nix-command' is disabled"));
    }

    #[test]
    fn test_home_manager_pattern() {
        let pattern = PATTERNS.iter().find(|p| p.id == "home-manager-not-found").unwrap();
        let re = pattern.regex();
        assert!(re.is_match("error: attribute 'home-manager' missing"));
        assert!(re.is_match("undefined variable 'home-manager'"));
    }

    #[test]
    fn test_unfree_pattern() {
        let pattern = PATTERNS.iter().find(|p| p.id == "unfree-not-allowed").unwrap();
        let re = pattern.regex();
        assert!(re.is_match("Package 'steam' is not free and refusing to evaluate"));
        assert!(re.is_match("has an unfree license"));
    }
}
