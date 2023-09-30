#[cfg(feature = "bench")] 
fn main() {
    let mut world = sandforge::bench_init();

    sandforge::bench_fill(&mut world);

    sandforge::bench_until_empty(&mut world);
}

#[cfg(not(feature = "bench"))] 
fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Error)
        .format_target(false)
        .format_timestamp(None)
        .init();


    std::thread::spawn(|| { sandforge::deadlock_checker() });

    pollster::block_on(sandforge::run());
}