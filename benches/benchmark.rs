use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;
use rmesg::entry::{Entry, LogFacility, LogLevel};
use std::time::Duration;

fn display_entry() {
    let entry_struct = Entry {
        timestamp_from_system_start: match rand::thread_rng().gen_bool(0.5) {
            true => Some(Duration::from_secs_f64(rand::thread_rng().gen::<f64>())),
            false => None,
        },
        facility: match rand::thread_rng().gen_bool(0.5) {
            true => Some(LogFacility::Kern),
            false => None,
        },
        level: match rand::thread_rng().gen_bool(0.5) {
            true => Some(LogLevel::Info),
            false => None,
        },
        sequence_num: match rand::thread_rng().gen_bool(0.5) {
            true => Some(rand::thread_rng().gen::<usize>()),
            false => None,
        },
        message: "Some very long string with no purpose. Lorem. Ipsum. Something Something.".to_owned(),
    };

    let displayed = format!("{}", entry_struct);
    black_box(displayed);
}

pub fn entry_display_benchmark(c: &mut Criterion) {
    c.bench_function("display_entry", |b| b.iter(|| black_box(display_entry())));
}

criterion_group!(benches, entry_display_benchmark);
criterion_main!(benches);

