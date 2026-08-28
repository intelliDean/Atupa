//! Low-level EVM stack and memory decoders for call target addresses and function selectors.

/// Extracts the target contract address from an EVM call/create stack.
///
/// In standard EVM `CALL`, `STATICCALL`, `DELEGATECALL`, and `CALLCODE` opcodes,
/// the target address is the second item from the top of the stack (`stack[len - 2]`).
pub fn extract_target_address(stack: &[String]) -> Option<String> {
    if stack.len() < 2 {
        return None;
    }
    let hex_addr = &stack[stack.len() - 2];
    let clean_hex = hex_addr.trim_start_matches("0x");
    let padded = format!("{:0>40}", clean_hex);
    let extracted = &padded[padded.len().saturating_sub(40)..];
    Some(format!("0x{extracted}"))
}

/// Attempts to extract the 4-byte function selector from EVM memory based on
/// call opcode argument offsets and lengths on the stack.
///
/// Handles both single 32-byte word extraction and selectors spanning across
/// adjacent 32-byte memory word boundaries.
pub fn extract_memory_selector(
    op: &str,
    stack: &[String],
    memory: &[String],
) -> Option<String> {
    let (args_offset_idx, args_length_idx) = match op {
        "CALL" | "CALLCODE" if stack.len() >= 5 => (stack.len() - 4, stack.len() - 5),
        "DELEGATECALL" | "STATICCALL" if stack.len() >= 4 => (stack.len() - 3, stack.len() - 4),
        _ => return None,
    };

    let offset_str = stack[args_offset_idx].trim_start_matches("0x");
    let len_str = stack[args_length_idx].trim_start_matches("0x");

    let offset = usize::from_str_radix(offset_str, 16).ok()?;
    let length = usize::from_str_radix(len_str, 16).ok()?;

    if length < 4 {
        return None;
    }

    let word_idx = offset / 32;
    let byte_offset = offset % 32;
    let hex_offset = byte_offset * 2; // Each byte is 2 hex characters

    let word = memory.get(word_idx)?;
    let clean_word = word.trim_start_matches("0x");

    if clean_word.len() >= hex_offset + 8 {
        let selector = &clean_word[hex_offset..hex_offset + 8];
        Some(format!("0x{selector}"))
    } else if word_idx + 1 < memory.len() {
        // The 4-byte selector spans across two memory boundary words
        let p1 = &clean_word[hex_offset..];
        let needed = 8 - p1.len();
        let next_word = memory[word_idx + 1].trim_start_matches("0x");
        if next_word.len() >= needed {
            let p2 = &next_word[..needed];
            Some(format!("0x{p1}{p2}"))
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_target_address_from_call_stack() {
        let stack = vec![
            "0x0".to_string(), // retLength
            "0x0".to_string(), // retOffset
            "0x4".to_string(), // argsLength
            "0x20".to_string(), // argsOffset
            "0x0".to_string(), // value
            "0x000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(), // target address
            "0x1000".to_string(), // gas
        ];
        let addr = extract_target_address(&stack);
        assert_eq!(
            addr,
            Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string())
        );
    }

    #[test]
    fn extract_target_address_returns_none_on_small_stack() {
        assert_eq!(extract_target_address(&[]), None);
        assert_eq!(extract_target_address(&["0x1".to_string()]), None);
    }

    #[test]
    fn extracts_memory_selector_from_first_word() {
        let stack = vec![
            "0x0".to_string(),
            "0x0".to_string(),
            "0x4".to_string(),  // length = 4 bytes
            "0x0".to_string(),  // offset = 0 bytes
            "0x0".to_string(),
            "0x1111".to_string(),
            "0x1000".to_string(),
        ];
        let memory = vec![
            "a9059cbb00000000000000000000000000000000000000000000000000000000".to_string(),
        ];

        let sel = extract_memory_selector("CALL", &stack, &memory);
        assert_eq!(sel, Some("0xa9059cbb".to_string()));
    }

    #[test]
    fn extracts_memory_selector_with_offset() {
        let stack = vec![
            "0x0".to_string(),
            "0x0".to_string(),
            "0x4".to_string(),   // length = 4 bytes
            "0x20".to_string(),  // offset = 32 bytes (word 1)
            "0x0".to_string(),
            "0x1111".to_string(),
            "0x1000".to_string(),
        ];
        let memory = vec![
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            "617ba03700000000000000000000000000000000000000000000000000000000".to_string(),
        ];

        let sel = extract_memory_selector("CALL", &stack, &memory);
        assert_eq!(sel, Some("0x617ba037".to_string()));
    }

    #[test]
    fn extracts_memory_selector_staticcall() {
        let stack = vec![
            "0x0".to_string(),
            "0x0".to_string(),
            "0x4".to_string(),  // length = 4 bytes (len - 4)
            "0x0".to_string(),  // offset = 0 bytes (len - 3)
            "0x1111".to_string(),
            "0x1000".to_string(),
        ];
        let memory = vec![
            "70a0823100000000000000000000000000000000000000000000000000000000".to_string(),
        ];

        let sel = extract_memory_selector("STATICCALL", &stack, &memory);
        assert_eq!(sel, Some("0x70a08231".to_string()));
    }

    #[test]
    fn returns_none_when_length_less_than_4() {
        let stack = vec![
            "0x0".to_string(),
            "0x0".to_string(),
            "0x3".to_string(),  // length < 4
            "0x0".to_string(),
            "0x0".to_string(),
            "0x1111".to_string(),
            "0x1000".to_string(),
        ];
        let memory = vec![
            "a9059cbb00000000000000000000000000000000000000000000000000000000".to_string(),
        ];

        assert_eq!(extract_memory_selector("CALL", &stack, &memory), None);
    }
}
