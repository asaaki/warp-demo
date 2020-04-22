use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

// align with competitor length
const REQUEST_ID_DATA_LENGTH: usize = 63;
type InnerRequestIdData = [u8; REQUEST_ID_DATA_LENGTH];
type RequestIdDataArrayVecString = arrayvec::ArrayString<InnerRequestIdData>;

// 63 bytes content, 64 bytes total, cache line optimized
type RequestIdDataCacheString = arraystring::CacheString;

// both should have the same size:
// eprintln!(
//     "size RequestIdDataArrayVecString = {}",
//     std::mem::size_of::<RequestIdDataArrayVecString>()
// );
// eprintln!(
//     "size RequestIdDataCacheString = {}",
//     std::mem::size_of::<RequestIdDataCacheString>()
// );

fn array_vec_string(input: &str) -> RequestIdDataArrayVecString {
    let mut min_length = core::cmp::min(input.len(), REQUEST_ID_DATA_LENGTH);
    // cheap but also potentially exploitable; equal performance with arraystring::CacheString,
    // so this less safe check should not be taken:
    // while !input.is_char_boundary(min_length) { min_length -= 1 }
    // very costly but much safer (CacheString does this internally, too):
    while !input.is_char_boundary(min_length) { min_length = min_length.saturating_sub(1) }
    let (truncated, _) = input.split_at(min_length);
    RequestIdDataArrayVecString::from(truncated).unwrap()
}

fn cache_string(input: &str) -> RequestIdDataCacheString {
    RequestIdDataCacheString::from_str_truncate(input)
}

/*
=============================================================63
This is a very long string and should get truncated at some point, because we have a fixed length.
*/
const TEST_STR: &'static str = "This is a very long string and should get truncated at some point, because we have a fixed length.";
const EXPECTED: &'static str = "This is a very long string and should get truncated at some poi";

fn truncate_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("truncate &str");
    for i in 1..=10 {
        group.bench_function(BenchmarkId::new("arrayvec_str+t", i), |b| {
            b.iter(|| assert_eq!(EXPECTED, array_vec_string(black_box(TEST_STR)).as_str()))
        });
        group.bench_function(BenchmarkId::new("arraystring_ct", i), |b| {
            b.iter(|| assert_eq!(EXPECTED, cache_string(black_box(TEST_STR)).as_str()))
        });
    }
    group.finish()
}

/*
=============================================================63
Let's run test strings with some special chars like emojis 👨‍👨‍👦‍👦.
*/
const EMOJI_STR: &'static str =
    "Let's run test strings with some special chars like emojis 👨‍👨‍👦‍👦.";
const EXPECTED2: &'static str = "Let's run test strings with some special chars like emojis 👨";

fn truncate_emoji_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("truncate emoji &str");

    for i in 1..=10 {
        group.bench_function(BenchmarkId::new("arrayvec_str+t", i), |b| {
            b.iter(|| assert_eq!(EXPECTED2, array_vec_string(black_box(EMOJI_STR)).as_str()))
        });

        group.bench_function(BenchmarkId::new("arraystring_ct", i), |b| {
            b.iter(|| assert_eq!(EXPECTED2, cache_string(black_box(EMOJI_STR)).as_str()))
        });
    }
    group.finish()
}

const SHORT_STR: &'static str = "a31eaf0c-a573-44c9-9e47-e26a1d6c53b1";

fn short_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("short &str (UUID)");

    for i in 1..=10 {
        group.bench_function(BenchmarkId::new("arrayvec_str+t", i), |b| {
            b.iter(|| assert_eq!(SHORT_STR, array_vec_string(black_box(SHORT_STR)).as_str()))
        });

        group.bench_function(BenchmarkId::new("arraystring_ct", i), |b| {
            b.iter(|| assert_eq!(SHORT_STR, cache_string(black_box(SHORT_STR)).as_str()))
        });
    }
    group.finish()
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets = truncate_str, truncate_emoji_str, short_str
);
criterion_main!(benches);
