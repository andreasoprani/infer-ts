# Standard Format Validation Refactoring Plan

**Goal**: Simplify Standard format validation by accepting all compositional combinations of date/separator/time/timezone and using lazy domain initialization for CSP inference.

**Status**: Not started

---

## Overview

This refactoring moves from an explicit enumeration approach (generating ~500 format combinations and manually restricting which are valid) to a compositional parsing approach (parsing on-demand and accepting all structurally valid combinations).

### Key Changes

- Replace explicit validation with compositional parsing
- Remove `Format::all()` - use lazy domain initialization instead
- Use hybrid CSP: parse first value to construct domain, validate subsequent values
- Accept all compositional combinations (remove artificial restrictions)
- Update docs to reflect lazy domain initialization

---

## Steps

### ✅ Step 1: Restructure `StandardFormat` to group time components

**File**: `src/formats/standard.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardFormat {
    DateOnly {
        date: DateFmt
    },
    DateTime {
        date: DateFmt,
        time: TimeComponent,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeComponent {
    separator: Separator,
    format: TimeFmt,
    timezone: Option<Timezone>,
}
```

**Changes:**
- Keep `DateFmt` directly (no wrapper needed)
- Group separator/time/timezone into `TimeComponent` struct
- Maintains the `DateOnly` vs `DateTime` distinction

**Acceptance Criteria:**
- [ ] Code compiles
- [ ] All existing tests pass
- [ ] `TimeComponent` struct created with proper derives

---

### ✅ Step 2: Replace `validates()` with compositional `parse()`

**File**: `src/formats/standard.rs`

Remove the massive 200+ line `validates()` method. Replace with:

```rust
impl StandardFormat {
    /// Parse a value and return all standard formats that match it.
    /// Uses compositional parsing: date → separator → time → timezone.
    pub fn parse(value: &str) -> Vec<StandardFormat> {
        let mut matches = Vec::new();

        // Try each date format
        for date_fmt in DateFmt::all() {
            if let Some(remaining) = parse_date(value, *date_fmt) {
                if remaining.is_empty() {
                    // Date-only match
                    matches.push(StandardFormat::DateOnly { date: *date_fmt });
                } else {
                    // Try to parse time component (sep + time + optional tz)
                    matches.extend(
                        parse_time_component(remaining)
                            .into_iter()
                            .map(|time| StandardFormat::DateTime {
                                date: *date_fmt,
                                time
                            })
                    );
                }
            }
        }

        matches
    }

    /// Validate that a value matches this specific format.
    /// Used for CSP filtering after initial domain construction.
    pub(super) fn validates(&self, value: &str) -> bool {
        // Simplified validation that just checks if this format parses the value
        Self::parse(value).contains(self)
    }
}
```

**Acceptance Criteria:**
- [ ] `parse()` method implemented
- [ ] `validates()` simplified to use `parse()`
- [ ] All existing tests pass

---

### ✅ Step 3: Create component-specific parsers

**File**: `src/formats/standard.rs` or `src/formats/date.rs`

```rust
/// Parse date component. Returns remaining string if successful.
fn parse_date(s: &str, fmt: DateFmt) -> Option<&str> {
    match fmt {
        DateFmt::Iso => {
            if s.len() >= 10 && parse_iso_date(&s[..10]).is_some() {
                Some(&s[10..])
            } else {
                None
            }
        }
        DateFmt::SlashUS => {
            if s.len() >= 10 && validate_slash_date(&s[..10], true) {
                Some(&s[10..])
            } else {
                None
            }
        }
        // ... similar for each DateFmt variant
    }
}

/// Parse time component (separator + time format + optional timezone).
/// Returns all matching TimeComponent instances.
fn parse_time_component(s: &str) -> Vec<TimeComponent> {
    let mut matches = Vec::new();

    for sep in [Separator::T, Separator::Space] {
        if let Some(remaining) = parse_separator(s, sep) {
            for time_fmt in [TimeFmt::Hms, TimeFmt::HmsFrac, TimeFmt::HmsCompact,
                            TimeFmt::Hms12, TimeFmt::Hms12Compact] {
                if let Some(remaining) = parse_time_format(remaining, time_fmt) {
                    // No timezone
                    if remaining.is_empty() {
                        matches.push(TimeComponent {
                            separator: sep,
                            format: time_fmt,
                            timezone: None
                        });
                    }
                    // Try timezones
                    for tz in [Timezone::Utc, Timezone::Offset, Timezone::OffsetCompact] {
                        if parse_timezone(remaining, tz).is_some_and(|r| r.is_empty()) {
                            matches.push(TimeComponent {
                                separator: sep,
                                format: time_fmt,
                                timezone: Some(tz)
                            });
                        }
                    }
                }
            }
        }
    }

    matches
}

fn parse_separator(s: &str, sep: Separator) -> Option<&str> { /* ... */ }
fn parse_time_format(s: &str, fmt: TimeFmt) -> Option<&str> { /* ... */ }
fn parse_timezone(s: &str, tz: Timezone) -> Option<&str> { /* ... */ }
```

**Key points:**
- Each parser returns `Option<&str>` (remaining unparsed string)
- Parsers are composable: date → separator → time → timezone
- Natural acceptance/rejection based on structural validity
- No artificial restrictions on which combinations are allowed

**Acceptance Criteria:**
- [ ] `parse_date()` implemented for all `DateFmt` variants
- [ ] `parse_time_component()` implemented
- [ ] Helper parsers (`parse_separator`, `parse_time_format`, `parse_timezone`) implemented
- [ ] All existing tests pass
- [ ] New tests added for compositional parsing

---

### ✅ Step 4: Remove `Format::all()` and `StandardFormat::all_valid()`

**Files**:
- `src/formats/mod.rs`
- `src/formats/standard.rs`

Delete these methods entirely:
- `Format::all()` in `formats/mod.rs`
- `StandardFormat::all_valid()` in `formats/standard.rs`
- The `OnceLock<Vec<Format>>` static

**Rationale:** With lazy domain initialization, we don't need to pre-generate all formats.

**Acceptance Criteria:**
- [ ] `Format::all()` removed
- [ ] `StandardFormat::all_valid()` removed
- [ ] `OnceLock` static removed
- [ ] Code compiles (may need to update inference code first)

---

### ✅ Step 5: Add `Format::parse()` for lazy domain construction

**File**: `src/formats/mod.rs`

```rust
impl Format {
    /// Parse a value and return all formats that match it.
    /// This is used for lazy domain initialization in CSP inference.
    pub fn parse(value: &str) -> Vec<Format> {
        if value.is_empty() || !value.is_ascii() {
            return vec![];
        }

        let mut matches = Vec::new();

        // Try standard formats (compositional parsing)
        matches.extend(
            StandardFormat::parse(value)
                .into_iter()
                .map(Format::Standard)
        );

        // Try Unix formats
        for precision in UnixPrecision::all() {
            let unix_fmt = UnixFormat { precision: *precision };
            if unix_fmt.validates(value) {
                matches.push(Format::Unix(unix_fmt));
            }
        }

        matches
    }
}
```

**Acceptance Criteria:**
- [ ] `Format::parse()` implemented
- [ ] Returns all matching formats for a given value
- [ ] Handles empty/non-ASCII input correctly
- [ ] Tests added for `Format::parse()`

---

### ✅ Step 6: Update CSP inference to use hybrid approach

**File**: `src/inference.rs` (or wherever the CSP logic lives)

```rust
/// Infer timestamp format from a series of values using CSP with lazy domain initialization.
///
/// Algorithm:
/// 1. Parse first value to construct initial domain (lazy initialization)
/// 2. For subsequent values, filter domain using validation (constraint propagation)
/// 3. Return remaining candidates after processing all values
pub fn infer_format(values: &[&str]) -> Vec<Format> {
    if values.is_empty() {
        return vec![];
    }

    // Trim whitespace from values
    let trimmed: Vec<String> = values.iter()
        .map(|v| v.trim().to_string())
        .collect();

    // First value: Parse to construct initial domain (lazy initialization)
    let mut candidates: HashSet<Format> = Format::parse(&trimmed[0])
        .into_iter()
        .collect();

    // Subsequent values: Traditional CSP domain reduction
    for value in &trimmed[1..] {
        candidates.retain(|fmt| fmt.validates(value));

        // Early termination if domain is empty
        if candidates.is_empty() {
            break;
        }
    }

    candidates.into_iter().collect()
}
```

**Key points:**
- First value uses `Format::parse()` (lazy domain construction)
- Subsequent values use `fmt.validates()` (efficient filtering)
- Early termination optimization when domain becomes empty

**Acceptance Criteria:**
- [ ] Inference function updated to use hybrid approach
- [ ] First value parsed for domain construction
- [ ] Subsequent values use validation
- [ ] Early termination implemented
- [ ] All existing inference tests pass
- [ ] Performance is same or better than before

---

### ✅ Step 7: Update documentation and comments

Update the following documentation to reflect the new approach:

**`src/formats/mod.rs` module docs:**
```rust
//! Timestamp format definitions and per-value validation.
//!
//! Each [`Format`] variant represents an exact timestamp layout. Validation is
//! strict: a value only passes if its structure matches the format precisely, with
//! no extra or missing components.
//!
//! # Architecture
//!
//! Formats are organized compositionally:
//!
//! - **Standard formats** are parsed by composing date, separator, time, and timezone
//!   components. All structural combinations are accepted—the parser naturally rejects
//!   invalid ones. Format discovery uses lazy domain initialization: the first value
//!   determines which formats are possible, then subsequent values filter the domain.
//!
//! - **Unix formats** are bare integers distinguished by digit count.
//!
//! # CSP Inference with Lazy Domain Initialization
//!
//! The inference engine uses Constraint Satisfaction Problem (CSP) techniques:
//! 1. **Lazy domain construction**: `Format::parse()` on the first value constructs
//!    the initial set of possible formats (typically 2-10 formats, not 500+).
//! 2. **Constraint propagation**: Subsequent values filter the domain via `validates()`.
//! 3. **Early termination**: Stop if the domain becomes empty.
//!
//! This avoids pre-generating hundreds of format combinations that may never match.
```

**`src/formats/standard.rs` top comment:**
```rust
/// A standard datetime format composed of date, separator, time, and optional timezone.
///
/// Formats are discovered via compositional parsing rather than enumeration.
/// All structural combinations of components are accepted; the parser naturally
/// rejects invalid structures during parsing.
```

**Remove outdated comments:**
- Delete comment on lines 5-7 about "All combinations are constructible; the validator enforces..."
- Delete comment on lines 23-25 about "Many combinations are impossible...CSP inference eliminates..."

**Acceptance Criteria:**
- [ ] Module-level docs updated in `mod.rs`
- [ ] Struct/enum docs updated in `standard.rs`
- [ ] Outdated comments removed
- [ ] New CSP section added explaining lazy domain initialization
- [ ] Docs build without warnings

---

### ✅ Step 8: Ensure Polars compatibility

**File**: New test file or add to existing tests

Add validation that all generated format combinations produce valid Polars format strings:

```rust
#[cfg(test)]
mod polars_compat_tests {
    use super::*;

    #[test]
    fn all_parsed_formats_generate_valid_polars_strings() {
        // Sample values covering different format families
        let test_values = vec![
            "2024-01-15T10:30:00",
            "01/15/2024 10:30:00",
            "15.01.2024 10:30:00",
            "2024-01-15 10:30:00+05:30",
            "Jan 15, 2024 10:30:00",
            "Mon, 15 Jan 2024 10:30:00 +0530",
            // ... more test cases
        ];

        for value in test_values {
            let formats = Format::parse(value);
            for fmt in formats {
                let polars_fmt = fmt.polars_format();
                // Verify format string is non-empty and well-formed
                assert!(!polars_fmt.is_empty());
                // TODO: Actually test with Polars if integration tests are available
            }
        }
    }
}
```

**Acceptance Criteria:**
- [ ] Polars compatibility tests added
- [ ] All format combinations generate non-empty format strings
- [ ] Format strings are well-formed (no obvious syntax errors)
- [ ] (Optional) Integration test with actual Polars parsing

---

### ✅ Step 9: Preserve existing test coverage

**Files**: `src/formats/mod.rs` tests (starting at line 100)

Keep all existing tests to ensure:
- All previously valid formats are still accepted
- Format disjointness is preserved where needed
- Edge cases still work correctly

Update test helpers if needed:
```rust
/// Assert that exactly the listed formats validate `value`.
fn assert_set(value: &str, expected: &[Format]) {
    let parsed = Format::parse(value);
    assert_eq!(
        parsed.iter().collect::<HashSet<_>>(),
        expected.iter().collect::<HashSet<_>>(),
        "Parsed formats don't match expected for {:?}",
        value
    );
}
```

**Acceptance Criteria:**
- [ ] All existing tests still pass
- [ ] Test helpers updated to use `Format::parse()` if needed
- [ ] No regressions in format detection
- [ ] Edge cases still handled correctly

---

### ✅ Step 10: Remove artificial restrictions

The new compositional parser will naturally accept or reject combinations based on structural validity. For example:

- ✅ **Accept**: `01/15/2024T10:30:00` (slash date + T separator) - unusual but structurally valid
- ✅ **Accept**: `01/15/2024 10:30:00+05:00` (slash date + timezone) - structurally valid
- ❌ **Reject**: Invalid date values (month > 12, etc.) - parser catches these
- ❌ **Reject**: Malformed structure - parser returns `None`

If certain combinations should remain restricted for semantic reasons (e.g., Polars can't parse them), document this explicitly and add checks in `polars_format()`.

**Acceptance Criteria:**
- [ ] Previously restricted combinations now accepted (if structurally valid)
- [ ] Semantically invalid combinations still rejected
- [ ] Any remaining restrictions are documented
- [ ] Polars compatibility verified for newly accepted combinations

---

### ✅ Step 11: Performance validation

**File**: Add benchmarks (e.g., `benches/inference.rs`)

Add benchmarks to verify the new approach is faster:

```rust
#[bench]
fn bench_lazy_domain_init(b: &mut Bencher) {
    let values = vec!["2024-01-15T10:30:00"; 1000];
    b.iter(|| {
        let mut candidates = Format::parse(values[0]).into_iter().collect::<HashSet<_>>();
        for v in &values[1..] {
            candidates.retain(|f| f.validates(v));
        }
        candidates
    });
}

#[bench]
fn bench_mixed_formats(b: &mut Bencher) {
    let values = vec![
        "2024-01-15T10:30:00",
        "01/15/2024 10:30:00",
        "15.01.2024 10:30:00",
        // ... etc
    ];
    b.iter(|| {
        for value in &values {
            let _ = Format::parse(value);
        }
    });
}
```

**Acceptance Criteria:**
- [ ] Benchmarks added
- [ ] Performance is same or better than old approach
- [ ] No performance regressions on large datasets
- [ ] Memory usage is reasonable

---

## Summary of Changes

| Aspect | Old Approach | New Approach |
|--------|-------------|--------------|
| Format generation | Pre-generate all ~500 combinations | Parse on-demand from first value |
| Validation | Match against all combinations explicitly | Compositional parsing |
| CSP domain | Static (`Format::all()`) | Lazy initialization (`Format::parse()`) |
| Code size | 200+ line `validates()` | Smaller compositional parsers |
| Restrictions | Hard-coded combination rules | Natural structural validity |
| Extensibility | Add format → update match arms | Add component → automatic composition |
| Performance | O(500) per value | O(10) per value (after first) |

---

## Testing Checklist

Before considering this refactoring complete:

- [ ] All existing unit tests pass
- [ ] All existing integration tests pass
- [ ] New tests added for compositional parsing
- [ ] Polars compatibility verified
- [ ] Performance benchmarks show improvement or no regression
- [ ] Documentation is complete and accurate
- [ ] No compiler warnings
- [ ] Code is formatted (`cargo fmt`)
- [ ] Clippy is happy (`cargo clippy`)

---

## Rollback Plan

If the refactoring causes issues:

1. This is on a feature branch - can simply abandon and revert
2. Git history preserves old approach
3. Can cherry-pick specific improvements (e.g., `TimeComponent` struct) without full refactoring

---

## Notes

- This refactoring improves extensibility, performance, and maintainability
- The CSP semantics are preserved through lazy domain initialization
- All structural combinations are now accepted unless Polars can't handle them
- The hybrid approach (parse first, validate rest) is optimal for performance
