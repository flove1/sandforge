#[cfg(feature = "bench")] 
fn main() {
    let mut world = sandforge::bench_init();

    sandforge::bench_fill(&mut world);

    sandforge::bench_until_empty(&mut world);
}

#[cfg(not(feature = "bench"))] 
fn main() {
    env_logger::init();

    std::thread::spawn(|| { sandforge::deadlock_checker() });

    sandforge::run();
}