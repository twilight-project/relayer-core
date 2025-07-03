# Run all benchmarks

cargo bench

# Run specific benchmark

cargo bench relayer_benchmarks

# Run with specific features

cargo bench --features "your_feature"

# Generate detailed HTML reports

cargo bench -- --output-format html

// In your main.rs or lib.rs #[cfg(feature = "profiling")]
use std::time::Instant;

#[cfg(feature = "profiling")]
macro_rules! time_it {
($name:expr, $code:block) => {
let start = Instant::now();
let result = $code;
let duration = start.elapsed();
tracing::info!("⏱️ {} took: {:?}", $name, duration);
result
};
}
