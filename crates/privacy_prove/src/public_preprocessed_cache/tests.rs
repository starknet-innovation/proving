#[test]
fn cached_boundary_reconstructs_the_fixed_root_and_opening_blocks() {
    use super::*;

    let tree = cached_commitment().expect("checked public boundary");
    assert_eq!(tree.root().0, data::ROOT);
    assert_eq!(tree.layers.len(), data::LIFTING as usize + 1);
    for (depth, layer) in tree.layers.iter().enumerate() {
        assert_eq!(layer.len(), if depth <= data::BOUNDARY_DEPTH as usize { 1 << depth } else { 0 });
    }
    let block = 1usize << (data::LIFTING - data::BOUNDARY_DEPTH);
    let size = 1usize << data::LIFTING;
    let leaves = tree.omitted_leaf_positions(&[1, block - 1, block, size - 1]);
    let expected: Vec<_> = (0..2 * block).chain((size - block)..size).collect();
    assert_eq!(leaves, expected);
}

#[test]
fn source_fingerprint_uses_all_rows_and_little_endian_words() {
    use super::*;

    struct SequenceSource {
        log_size: u32,
        changed: Option<usize>,
    }

    impl TraceSource for SequenceSource {
        fn log_size(&self) -> u32 {
            self.log_size
        }

        fn write_values(&self, dst: &mut [BaseField]) {
            self.write_rows(0, dst);
        }

        fn write_rows(&self, first: usize, dst: &mut [BaseField]) {
            for (i, value) in dst.iter_mut().enumerate() {
                let row = first + i;
                *value = BaseField::from_u32_unchecked(
                    row as u32 + u32::from(self.changed == Some(row)),
                );
            }
        }
    }

    let source = SequenceSource { log_size: 13, changed: None };
    let mut canonical = 13_u32.to_le_bytes().to_vec();
    for row in 0..1_u32 << 13 {
        canonical.extend_from_slice(&row.to_le_bytes());
    }
    assert_eq!(fingerprint(&source), Blake2sHasher::hash(&canonical));
    for row in [0, 4095, 4096, 8191] {
        assert_ne!(
            fingerprint(&source),
            fingerprint(&SequenceSource { log_size: 13, changed: Some(row) }),
        );
    }
}

#[test]
fn metadata_rejects_other_shapes_and_column_orders() {
    use std::sync::Arc;

    use super::*;

    struct ShapeSource(u32);

    impl TraceSource for ShapeSource {
        fn log_size(&self) -> u32 {
            self.0
        }

        fn write_values(&self, _dst: &mut [BaseField]) {
            panic!("metadata checks must not read source values");
        }
    }

    let sources: Vec<SharedTraceSource> = data::LOG_SIZES
        .iter()
        .map(|&log_size| Arc::new(ShapeSource(log_size)) as SharedTraceSource)
        .collect();
    let ids: Vec<_> = data::IDS.iter().map(|id| (*id).to_owned()).collect();
    assert!(matches_metadata(&sources, &ids, 2, 23));
    assert!(!matches_metadata(&sources, &ids, 3, 23));
    assert!(!matches_metadata(&sources, &ids, 2, 22));
    assert!(!matches_metadata(&sources[..44], &ids, 2, 23));
    assert!(!matches_metadata(&sources, &ids[..44], 2, 23));
    let mut reordered = ids.clone();
    reordered.swap(0, 1);
    assert!(!matches_metadata(&sources, &reordered, 2, 23));
    let mut wrong_size = sources;
    wrong_size[0] = Arc::new(ShapeSource(9));
    assert!(!matches_metadata(&wrong_size, &ids, 2, 23));
}

#[test]
fn malformed_cache_data_is_rejected() {
    use super::*;

    assert!(decode_boundary(b"00").is_none());
    let mut encoded = data::BOUNDARY_BASE85.as_bytes().to_vec();
    let middle = encoded.len() / 2;
    encoded[middle] = b'"';
    assert!(decode_boundary(&encoded).is_none());
}

#[test]
fn base85_words_preserve_bytes_and_reject_overflow() {
    use super::*;

    let encode = |mut value: u32| {
        let mut digits = [b'0'; 5];
        for digit in digits.iter_mut().rev() {
            *digit = BASE85_ALPHABET[(value % 85) as usize];
            value /= 85;
        }
        digits
    };
    for byte in 0..=u8::MAX {
        let word = u32::from_le_bytes([byte; 4]);
        assert_eq!(decode_word(&encode(word)), Some(word.to_le_bytes()));
    }
    for word in [0, 1, 84, 85, 86, 7224, 7225, 7226, 0x7fff_ffff, 0x8000_0000, u32::MAX] {
        assert_eq!(decode_word(&encode(word)), Some(word.to_le_bytes()));
    }
    assert_eq!(decode_word(b"00001"), Some([1, 0, 0, 0]));
    assert_eq!(decode_word(b"|NsC0"), Some(u32::MAX.to_le_bytes()));
    for invalid in [b"|NsC1", b"~~~~~"] {
        assert!(decode_word(invalid).is_none());
    }
    for byte in 0..=u8::MAX {
        if !BASE85_ALPHABET.contains(&byte) {
            for position in 0..5 {
                let mut encoded = *b"00000";
                encoded[position] = byte;
                assert!(decode_word(&encoded).is_none());
            }
        }
    }
    for count in 0..10 {
        if count != 5 {
            assert!(decode_word(&vec![b'0'; count]).is_none());
        }
    }
}
