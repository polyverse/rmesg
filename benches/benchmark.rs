use criterion::{black_box, criterion_group, criterion_main, Criterion};
use futures::stream::StreamExt;
use rand::Rng;
use rmesg::{
    entry::{Entry, LogFacility, LogLevel},
    klogctl::{klog, KLogEntries},
    kmsgfile::{kmsg, KMsgEntriesIter, KMsgEntriesStream},
};
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
    let displayed = random_entry().to_kmsg_str().unwrap();
    black_box(displayed);
}

fn entry_to_klog_str() {
    let displayed = random_entry().to_klog_str().unwrap();
    black_box(displayed);
}

fn kmsg_read() {
    let file = match rand::thread_rng().gen_bool(0.5) {
        true => Some("/dev/kmsg".to_owned()),
        false => None,
    };
    let entries = kmsg(file).unwrap();
    black_box(entries);
}

fn kmsg_iter_read() {
    let file = match rand::thread_rng().gen_bool(0.5) {
        true => Some("/dev/kmsg".to_owned()),
        false => None,
    };
    let entries = KMsgEntriesIter::with_options(file, rand::thread_rng().gen_bool(0.5)).unwrap();
    let mut count = 0;
    for entry in entries {
        black_box(entry).unwrap();
        count += 1;
        if count > 25 {
            break;
        }
    }
}

async fn kmsg_stream_read() {
    let file = match rand::thread_rng().gen_bool(0.5) {
        true => Some("/dev/kmsg".to_owned()),
        false => None,
    };
    let mut entries = KMsgEntriesStream::with_options(file, rand::thread_rng().gen_bool(0.5))
        .await
        .unwrap();
    let mut count = 0;
    while let Some(entry) = entries.next().await {
        black_box(entry).unwrap();
        count += 1;
        if count > 25 {
            break;
        }
    }
}

fn klog_read() {
    let entries = klog(false).unwrap();
    black_box(entries);
}

fn klog_iter_read() {
    let entries = KLogEntries::with_options(false, Duration::from_secs(1)).unwrap();
    let mut count = 0;
    for entry in entries {
        black_box(entry).unwrap();
        count += 1;
        if count > 25 {
            break;
        }
    }
}

async fn klog_stream_read() {
    let mut entries = KLogEntries::with_options(false, Duration::from_secs(1)).unwrap();
    let mut count = 0;
    while let Some(entry) = StreamExt::next(&mut entries).await {
        black_box(entry).unwrap();
        count += 1;
        if count > 25 {
            break;
        }
    }
}

pub fn benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    c.bench_function("display_entry", |b| b.iter(|| {black_box(display_entry());}));
    c.bench_function("entry_to_kmsg_str", |b| {
        b.iter(|| {black_box(entry_to_kmsg_str());})
    });
    c.bench_function("entry_to_klog_str", |b| {
        b.iter(|| {black_box(entry_to_klog_str());})
    });

    c.bench_function("kmsg_read", |b| b.iter(|| {black_box(kmsg_read());}));
    c.bench_function("kmsg_iter_read", |b| b.iter(|| {black_box(kmsg_iter_read());}));
    c.bench_function("kmsg_stream_read", |b| {
        b.to_async(&rt)
            .iter(|| async { black_box(kmsg_stream_read().await); });
    });

    c.bench_function("klog_read", |b| b.iter(|| {black_box(klog_read());}));
    c.bench_function("klog_iter_read", |b| b.iter(|| {black_box(klog_iter_read());}));
    c.bench_function("klog_stream_read", |b| {
        b.to_async(&rt)
            .iter(|| async { black_box(klog_stream_read().await); });
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
