use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;
use rmesg::entry::{Entry, LogFacility, LogLevel};
use std::time::Duration;

fn random_entry() -> Entry {
    Entry {
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
        message: "Some very long string with no purpose. Lorem. Ipsum. Something Something."
            .to_owned(),
    }
}

fn display_entry() {
    let displayed = format!("{}", random_entry());
    black_box(displayed);
}

fn entry_to_kmsg_str() {
    let displayed = random_entry().to_kmsg_str();
    black_box(displayed);
}

fn entry_to_klog_str() {
    let displayed = random_entry().to_klog_str();
    black_box(displayed);
}

pub fn benchmark(c: &mut Criterion) {
    c.bench_function("display_entry", |b| b.iter(|| black_box(display_entry())));
    c.bench_function("entry_to_kmsg_str", |b| {
        b.iter(|| black_box(entry_to_kmsg_str()))
    });
    c.bench_function("entry_to_klog_str", |b| {
        b.iter(|| black_box(entry_to_klog_str()))
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
