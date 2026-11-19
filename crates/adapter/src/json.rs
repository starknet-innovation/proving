//! The prover input's JSON, parsed on rayon's workers (one thread outlasted the setup): the large
//! arrays (casm states, memory tables) by serde_json, each into its field; the rest, those arrays
//! as `[]`, by the derived `Deserialize`, which reads each field from its own value: the one-piece
//! parse's result. Inputs declined here (a duplicate or escaped key, any error) parse in one piece.

use std::ops::Range;

use rayon::prelude::*;
use stwo_cairo_common::prover_types::cpu::CasmState;

use crate::ProverInput;
use crate::memory::{EncodedMemoryValueId, Memory};
use crate::opcodes::CasmStatesByOpcode;

/// The prover input in `json`: the value of `serde_json::from_slice::<ProverInput>(json)`.
pub fn prover_input_from_slice(json: &[u8]) -> serde_json::Result<ProverInput> {
    match split_parse(json) {
        Some(input) => Ok(input),
        None => serde_json::from_slice(json),
    }
}

/// The field that one large array is parsed into.
enum Slot<'a> {
    Casm(&'a mut Vec<CasmState>),
    Ids(&'a mut Vec<EncodedMemoryValueId>),
    F252(&'a mut Vec<[u32; 8]>),
    Small(&'a mut Vec<u128>),
}

impl Slot<'_> {
    fn fill(self, piece: &[u8]) -> serde_json::Result<()> {
        match self {
            Slot::Casm(v) => *v = serde_json::from_slice(piece)?,
            Slot::Ids(v) => *v = serde_json::from_slice(piece)?,
            Slot::F252(v) => *v = serde_json::from_slice(piece)?,
            Slot::Small(v) => *v = serde_json::from_slice(piece)?,
        }
        Ok(())
    }
}

// The casm-state fields of `CasmStatesByOpcode`, by their JSON keys.
macro_rules! casm_fields {
    ($($field:ident),* $(,)?) => {
        const CASM_KEYS: &[&[u8]] = &[$(stringify!($field).as_bytes()),*];

        fn casm_slots(
            states: &mut CasmStatesByOpcode,
        ) -> Vec<(&'static [u8], &mut Vec<CasmState>)> {
            let CasmStatesByOpcode { $($field,)* .. } = states;
            vec![$((stringify!($field).as_bytes(), $field)),*]
        }
    };
}

casm_fields!(
    generic_opcode, add_ap_opcode, add_opcode, add_opcode_small, assert_eq_opcode,
    assert_eq_opcode_double_deref, assert_eq_opcode_imm, call_opcode_abs, call_opcode_rel_imm,
    jnz_opcode_non_taken, jnz_opcode_taken, jump_opcode_rel_imm, jump_opcode_rel,
    jump_opcode_double_deref, jump_opcode_abs, mul_opcode_small, mul_opcode, ret_opcode,
    blake_compress_opcode, qm_31_add_mul_opcode,
);

/// The objects whose large arrays are parsed apart, by their keys from the document's root.
const GROUPS: [&[&[u8]]; 2] = [&[b"state_transitions", b"casm_states_by_opcode"], &[b"memory"]];

/// A large array: (is a memory table, key, value range).
type Cut<'a> = (bool, &'a [u8], Range<usize>);

/// The arrays of `found` that are parsed apart, in document order.
fn large_arrays<'a>(json: &[u8], mut found: Vec<Cut<'a>>) -> Vec<Cut<'a>> {
    // A key the struct does not have stays in the document, as in the one-piece parse.
    found.retain(|(is_memory, key, value)| {
        let known = if *is_memory {
            matches!(*key, b"address_to_id" | b"f252_values" | b"small_values")
        } else {
            CASM_KEYS.contains(key)
        };
        known && json[value.start] == b'['
    });
    found.sort_unstable_by_key(|(_, _, value)| value.start);
    found
}

/// The split parse, or `None` when the document is not one it takes.
fn split_parse(json: &[u8]) -> Option<ProverInput> {
    // Find the arrays, with a fast search for the ends of flat ones (see `array_end`).
    let text = std::str::from_utf8(json).ok()?;
    let mut found = vec![];
    let end = walk(json, Some(text), skip_ws(json, 0), &mut vec![], &mut found)?;
    if skip_ws(json, end) != json.len() {
        return None;
    }
    let cuts = large_arrays(json, found);

    // The rest of the document, each array written as `[]`.
    let cut_bytes: usize = cuts.iter().map(|(_, _, value)| value.len()).sum();
    let mut skeleton = Vec::with_capacity(json.len() - cut_bytes + 2 * cuts.len());
    let mut placeholders = Vec::with_capacity(cuts.len());
    let mut at = 0;
    for (is_memory, key, value) in &cuts {
        skeleton.extend_from_slice(&json[at..value.start]);
        placeholders.push((*is_memory, *key, skeleton.len()..skeleton.len() + 2));
        skeleton.extend_from_slice(b"[]");
        at = value.end;
    }
    skeleton.extend_from_slice(&json[at..]);
    // Walked again (brackets matched), each `[]` must be its key's value: each cut a whole value.
    let mut found = vec![];
    let end = walk(&skeleton, None, skip_ws(&skeleton, 0), &mut vec![], &mut found)?;
    if skip_ws(&skeleton, end) != skeleton.len()
        || large_arrays(&skeleton, found) != placeholders
    {
        return None;
    }
    let mut input: ProverInput = serde_json::from_slice(&skeleton).ok()?;

    let mut casm = casm_slots(&mut input.state_transitions.casm_states_by_opcode);
    let Memory { address_to_id, f252_values, small_values, .. } = &mut input.memory;
    let (mut ids, mut f252, mut small) =
        (Some(address_to_id), Some(f252_values), Some(small_values));
    let mut tasks: Vec<(Slot<'_>, &[u8])> = vec![];
    for (is_memory, key, value) in cuts {
        let slot = match (is_memory, key) {
            (true, b"address_to_id") => Slot::Ids(ids.take()?),
            (true, b"f252_values") => Slot::F252(f252.take()?),
            (true, _) => Slot::Small(small.take()?),
            (false, _) => {
                let i = casm.iter().position(|(name, _)| *name == key)?;
                Slot::Casm(casm.swap_remove(i).1)
            }
        };
        tasks.push((slot, &json[value]));
    }
    // A piece parses only when it is one whole array.
    tasks.sort_unstable_by_key(|(_, piece)| std::cmp::Reverse(piece.len()));
    tasks.into_par_iter().with_max_len(1).try_for_each(|(slot, piece)| slot.fill(piece)).ok()?;
    Some(input)
}

/// Walks the object at `b[i] == '{'` (at `path` from the root) to its end, descending on the way
/// to a group and recording its members in `found`; with `text` (`b`), see `array_end`.
fn walk<'a>(
    b: &'a [u8],
    text: Option<&str>,
    i: usize,
    path: &mut Vec<&'a [u8]>,
    found: &mut Vec<Cut<'a>>,
) -> Option<usize> {
    if *b.get(i)? != b'{' {
        return None;
    }
    let group = GROUPS.iter().position(|g| *g == path.as_slice());
    let mut j = skip_ws(b, i + 1);
    if *b.get(j)? == b'}' {
        return Some(j + 1);
    }
    loop {
        if *b.get(j)? != b'"' {
            return None;
        }
        let key_end = string_end(b, j)?;
        let key = &b[j + 1..key_end - 1];
        if key.contains(&b'\\') {
            return None;
        }
        j = skip_ws(b, key_end);
        if *b.get(j)? != b':' {
            return None;
        }
        let start = skip_ws(b, j + 1);
        path.push(key);
        let end = match (*b.get(start)?, text) {
            (b'{', _) if GROUPS.iter().any(|g| g.starts_with(path)) => {
                walk(b, text, start, path, found)?
            }
            (b'[', Some(text)) => array_end(b, text, start)?,
            _ => value_end(b, start)?,
        };
        path.pop();
        if let Some(group) = group {
            found.push((group == 1, key, start..end));
        }
        j = skip_ws(b, end);
        match *b.get(j)? {
            b',' => j = skip_ws(b, j + 1),
            b'}' => return Some(j + 1),
            _ => return None,
        }
    }
}

/// The end of the member value at `b[i] == '['`: a flat array's first `]` (fast) if a member or the
/// object's end follows, else matched brackets. A wrong end leaves a piece that fails to parse.
fn array_end(b: &[u8], text: &str, i: usize) -> Option<usize> {
    if b.get(skip_ws(b, i + 1)) != Some(&b'[') {
        if let Some(k) = text[i + 1..].find(']') {
            let end = i + 2 + k;
            let j = skip_ws(b, end);
            let follows = match b.get(j) {
                Some(b'}') => true,
                Some(b',') => b.get(skip_ws(b, j + 1)) == Some(&b'"'),
                _ => false,
            };
            if follows {
                return Some(end);
            }
        }
    }
    value_end(b, i)
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

/// The end of the string that starts at `b[i] == '"'`.
fn string_end(b: &[u8], mut i: usize) -> Option<usize> {
    i += 1;
    loop {
        match *b.get(i)? {
            b'"' => return Some(i + 1),
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
}

/// The end of the value at `b[i]` (brackets matched outside strings), checked by its parse.
fn value_end(b: &[u8], i: usize) -> Option<usize> {
    match *b.get(i)? {
        b'{' | b'[' => {
            let mut depth = 0_usize;
            let mut j = i;
            loop {
                match *b.get(j)? {
                    b'"' => {
                        j = string_end(b, j)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
        }
        b'"' => string_end(b, i),
        _ => {
            let mut j = i;
            while j < b.len() && !matches!(b[j], b',' | b'}' | b']') && skip_ws(b, j) == j {
                j += 1;
            }
            (j > i).then_some(j)
        }
    }
}

/// A small document with every field of `ProverInput` (compact, pretty, reordered, with members
/// the structs do not have) parses as in one piece; one this path declines parses in one piece.
#[test]
fn split_parse_matches_the_one_piece_parse() {
    let states = |n: usize| {
        let state = |k: usize| format!(r#"{{"pc":{},"ap":{},"fp":100}}"#, k + 1, 100 + k);
        (0..n).map(state).collect::<Vec<_>>().join(",")
    };
    let by_opcode = CASM_KEYS
        .iter()
        .enumerate()
        .map(|(i, key)| format!(r#""{}":[{}]"#, std::str::from_utf8(key).unwrap(), states(i % 4)))
        .collect::<Vec<_>>()
        .join(",");
    let transitions = format!(
        r#""state_transitions":{{"initial_state":{{"pc":1,"ap":100,"fp":100}},"final_state":{{"pc":5,"ap":120,"fp":100}},"casm_states_by_opcode":{{{by_opcode}}}}}"#
    );
    let memory = r#""memory":{"config":{"small_max":4722366482869645213695,"log_small_value_capacity":24},"address_to_id":[1073741823,0,1,2],"f252_values":[[0,0,0,0,0,0,17,134217728],[1,2,3,4,5,6,7,8]],"small_values":[290341444919459839,11,340282366920938463463374607431768211455]}"#;
    let rest = r#""pc_count":4,"public_memory_addresses":[1,2,3],"builtin_segments":{"add_mod_builtin":null,"bitwise_builtin":{"begin_addr":10,"stop_ptr":20},"output":null,"mul_mod_builtin":null,"pedersen_builtin":null,"poseidon_builtin":null,"range_check96_builtin":null,"range_check_builtin":null,"ec_op_builtin":null},"public_segment_context":{"present":[true,false,false,false,false,false,false,false,false,false,false]}"#;
    let compact = format!("{{{transitions},{memory},{rest}}}");
    let one: ProverInput = serde_json::from_str(&compact).unwrap();
    let expected = serde_json::to_vec(&one).unwrap();
    let same = |doc: &str| {
        let split = split_parse(doc.as_bytes()).expect("the split parse takes this document");
        assert_eq!(serde_json::to_vec(&split).unwrap(), expected);
    };
    same(&compact);
    same(&serde_json::to_string_pretty(&one).unwrap());
    same(&format!(" {{ {rest} , {memory},\n{transitions} }}\n"));
    // Members the structs do not have (strings with a `]`, arrays): the one-piece parse skips them.
    same(&compact.replacen(r#""add_opcode":["#, r#""unknown_opcode":[[1],"]"],"add_opcode":["#, 1));
    same(&compact.replacen(r#""fp":100}"#, r#""fp":100,"note":"],\"x\"","list":[1,[2]]}"#, 3));

    // Documents the split parse declines: each gives the one-piece parse's result.
    let declined = |doc: String| {
        assert!(split_parse(doc.as_bytes()).is_none());
        let via = prover_input_from_slice(doc.as_bytes()).map(|v| serde_json::to_vec(&v).unwrap());
        let one =
            serde_json::from_str::<ProverInput>(&doc).map(|v| serde_json::to_vec(&v).unwrap());
        assert_eq!(via.ok(), one.ok());
    };
    declined(compact.replacen(r#""add_opcode":"#, r#""add\u005fopcode":"#, 1));
    declined(compact.replacen(r#""add_opcode":"#, r#""add_opcode":[],"add_opcode":"#, 1));
    declined(compact.replacen(r#""memory":{"#, r#""memory":{"small_values":[],"#, 1));
    declined(compact.replacen(r#""small_values":["#, r#""small_values":[-1,"#, 1));
    declined(compact.replacen(r#""add_opcode":[{"pc":1"#, r#""add_opcode":[{"pc":1.5"#, 1));
    declined(format!("{compact},"));
}
