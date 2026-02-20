pub mod discover;
pub mod display;
pub mod errors;
pub mod parse;
pub mod shell;
pub mod types;

#[cfg(test)]
mod uuid_cross_reference_tests {
    // Verify that the two `is_valid_uuid` implementations in `parse.rs` and `shell.rs`
    // agree on all test inputs. Both are intentionally duplicated (see CLAUDE.md),
    // but they must produce identical results.

    const TEST_INPUTS: &[(&str, bool)] = &[
        // Valid UUIDs
        ("eb53d999-8692-42ce-a376-4f82206a086d", true),
        ("00000000-0000-0000-0000-000000000000", true),
        ("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", true),
        ("12345678-1234-1234-1234-123456789abc", true),
        // Invalid: uppercase
        ("EB53D999-8692-42CE-A376-4F82206A086D", false),
        // Invalid: too short
        ("eb53d999-8692-42ce-a376", false),
        // Invalid: too long
        ("eb53d999-8692-42ce-a376-4f82206a086da", false),
        // Invalid: bad chars
        ("gb53d999-8692-42ce-a376-4f82206a086d", false),
        // Invalid: wrong separators
        ("eb53d999_8692_42ce_a376_4f82206a086d", false),
        // Invalid: wrong length (34 chars, rejected before hyphen check)
        ("eb53d99986924f82206a086da376a376aa", false),
        // Invalid: empty
        ("", false),
        // Invalid: not a UUID at all
        ("not-a-uuid", false),
        // Invalid: mixed case
        ("eb53d999-8692-42CE-a376-4f82206a086d", false),
    ];

    #[test]
    fn parse_and_shell_uuid_validators_agree() {
        for (input, expected) in TEST_INPUTS {
            let parse_result = crate::parse::is_valid_uuid(input);
            let shell_result = crate::shell::is_valid_uuid(input);
            assert_eq!(
                parse_result, *expected,
                "parse::is_valid_uuid({:?}) = {}, expected {}",
                input, parse_result, expected
            );
            assert_eq!(
                shell_result, *expected,
                "shell::is_valid_uuid({:?}) = {}, expected {}",
                input, shell_result, expected
            );
            assert_eq!(
                parse_result, shell_result,
                "Implementations disagree on {:?}: parse={}, shell={}",
                input, parse_result, shell_result
            );
        }
    }
}
