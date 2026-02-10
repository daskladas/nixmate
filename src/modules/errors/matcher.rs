//! Pattern matching logic for Nix errors.
//!
//! Takes raw error text and finds matching patterns,
//! extracting captured groups for templating.

use super::patterns::{library_to_package, Category, Pattern, PATTERNS};

/// Result of matching an error against patterns.
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub pattern_id: String,
    pub category: Category,
    pub title: String,
    pub explanation: String,
    pub solution: String,
    pub deep_dive: String,
    pub tip: Option<String>,
    /// Captured regex groups for translation
    pub captures: Vec<String>,
}

/// Analyzes error text and returns the best matching pattern.
///
/// Returns None if no pattern matches.
pub fn analyze(error_text: &str) -> Option<MatchResult> {
    for pattern in PATTERNS {
        let re = pattern.regex();
        if let Some(captures) = re.captures(error_text) {
            return Some(build_result(pattern, &captures));
        }
    }
    None
}

/// Builds a MatchResult by substituting captured groups into templates.
fn build_result(pattern: &Pattern, captures: &regex::Captures) -> MatchResult {
    // Extract capture groups (skip group 0 which is the full match)
    let groups: Vec<&str> = captures
        .iter()
        .skip(1)
        .filter_map(|m| m.map(|m| m.as_str()))
        .collect();

    // Substitute $1, $2, etc. in templates
    let title = substitute_captures(pattern.title, &groups);
    let explanation = substitute_captures(pattern.explanation, &groups);
    let mut solution = substitute_captures(pattern.solution, &groups);
    let deep_dive = substitute_captures(pattern.deep_dive, &groups);
    let tip = pattern.tip.map(|t| substitute_captures(t, &groups));

    // Special handling for linker errors - map library names to packages
    if pattern.id == "linker-missing-lib" {
        if let Some(lib_name) = groups.first() {
            if let Some(pkg_name) = library_to_package(lib_name) {
                solution =
                    solution.replace(&format!("[ {} ]", lib_name), &format!("[ {} ]", pkg_name));
            }
        }
    }

    MatchResult {
        pattern_id: pattern.id.to_string(),
        category: pattern.category,
        title,
        explanation,
        solution,
        deep_dive,
        tip,
        captures: groups.iter().map(|s| s.to_string()).collect(),
    }
}

/// Replaces $1, $2, etc. with captured values.
fn substitute_captures(template: &str, captures: &[&str]) -> String {
    let mut result = template.to_string();
    for (i, cap) in captures.iter().enumerate() {
        result = result.replace(&format!("${}", i + 1), cap);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_linker_error() {
        let error = r#"
/nix/store/abc-binutils/bin/ld: cannot find -lssl
collect2: error: ld returned 1 exit status
        "#;

        let result = analyze(error).expect("Should match");
        assert_eq!(result.pattern_id, "linker-missing-lib");
        assert!(result.title.contains("ssl"));
        assert!(result.solution.contains("openssl")); // Library mapped
    }

    #[test]
    fn test_analyze_missing_header() {
        let error = "fatal error: openssl/ssl.h: No such file or directory";

        let result = analyze(error).expect("Should match");
        assert_eq!(result.pattern_id, "missing-header");
        assert!(result.title.contains("openssl/ssl.h"));
    }

    #[test]
    fn test_analyze_infinite_recursion() {
        let error = "error: infinite recursion encountered\n   at /nix/store/...";

        let result = analyze(error).expect("Should match");
        assert_eq!(result.pattern_id, "infinite-recursion");
    }

    #[test]
    fn test_analyze_no_match() {
        let error = "some random text that is not a nix error";
        assert!(analyze(error).is_none());
    }

    #[test]
    fn test_substitute_captures() {
        let template = "Error: $1 and $2";
        let captures = vec!["foo", "bar"];
        assert_eq!(
            substitute_captures(template, &captures),
            "Error: foo and bar"
        );
    }

    #[test]
    fn test_library_mapping_in_solution() {
        let error = "ld: cannot find -lz";
        let result = analyze(error).expect("Should match");
        assert!(result.solution.contains("zlib"));
    }
}
