use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

#[cfg(test)]
#[allow(dead_code)]
pub fn start_parametric_test<F>(nbiter: usize, regressions: Vec<u64>, function: F)
where
    F: Fn(SmallRng),
{
    use std::time::Duration;

    #[cfg(feature = "heavy_testing")]
    let nbiter = nbiter * 10000;

    for seed in regressions.iter() {
        println!("\nTest regression seed {}", seed);
        function(SmallRng::seed_from_u64(*seed));
        std::thread::sleep(Duration::from_millis(50));
    }

    let mut seeder = SmallRng::from_entropy();
    let nspace = nbiter.to_string().len();
    for n in 0..nbiter {
        let new_seed: u64 = seeder.gen();
        print!("\n{:1$}", n + 1, nspace);
        println!("/{}| Seed {:20}", nbiter, new_seed);
        function(SmallRng::seed_from_u64(new_seed));
        std::thread::sleep(Duration::from_millis(50));
    }
}
