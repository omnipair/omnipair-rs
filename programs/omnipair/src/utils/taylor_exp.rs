pub fn exp_fn_scaled(x: i64, scale: u64, terms: u64 ) -> u64 {
    let mut result = scale; // Initialize with first term (1 * scale)
    let mut term = scale as i64; // This will store (x^n / n!) * scale

    for n in 1..terms { 
       term = (term as i64 * x) / (n as i64 * scale as i64);
       result = result.saturating_add(term as u64)
    }

    return result;
}