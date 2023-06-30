use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

pub fn start_parametric_test<F>(nbiter: usize, regressions: Vec<u64>, function: F)
where
    F: Fn(SmallRng),
{
    let tsleep = std::time::Duration::from_millis(100);
    for seed in regressions.iter() {
        println!("Test regression seed {}", seed);
        function(SmallRng::seed_from_u64(*seed));
        std::thread::sleep(tsleep);
    }
    let mut seeder = SmallRng::from_entropy();
    for _ in 0..nbiter {
        let new_seed: u64 = seeder.gen();
        println!("Test seed: {}", new_seed);
        function(SmallRng::seed_from_u64(new_seed));
        std::thread::sleep(tsleep);
    }
}
