//! [`super::coeff_columns`]: the deferred coordinates of a secure value times its columns sum to
//! the secure product.

#[test]
fn coeff_columns_give_the_secure_product() {
    use stwo::core::fields::m31::BaseField;
    use stwo::core::fields::qm31::SecureField;

    use super::coeff_columns;

    let secure = |f: &dyn Fn(u32) -> u32| SecureField::from_m31_array(std::array::from_fn(|k| BaseField::from(f(k as u32))));
    let coeffs: Vec<SecureField> = (0..5u32).map(|i| secure(&|k| i * 7919 + k * 104729 + 3)).collect();
    let columns = coeff_columns(&coeffs);
    let value = secure(&|k| 2147483000 - k * 31);
    let v = value.to_m31_array();
    for (i, &coeff) in coeffs.iter().enumerate() {
        let got: [BaseField; 4] = std::array::from_fn(|j| (0..4).map(|k| v[k] * columns[4 * i + k][j]).sum());
        assert_eq!(SecureField::from_m31_array(got), coeff * value);
    }
}
