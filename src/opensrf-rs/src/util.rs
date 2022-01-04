use rand::Rng;

/// Returns a random 12-char numeric string
pub fn random_12() -> String {
    let mut rng = rand::thread_rng();
    let num: u64 = rng.gen_range(100_000_000_000..1_000_000_000_000);
    format!("{:0width$}", num, width = 12)
}

pub fn random_16() -> String {
    let mut rng = rand::thread_rng();
    let num: u64 = rng.gen_range(100_000_000_000..1_000_000_000_000_000);
    format!("{:0width$}", num, width = 16)
}
