#[cfg(feature = "bench")] 
fn main() {
    sandforge::bench();
}

#[cfg(not(feature = "bench"))] 
fn main() {
    env_logger::init();

    std::thread::spawn(|| { sandforge::deadlock_checker() });

    sandforge::run();
}